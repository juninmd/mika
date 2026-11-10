//! Criptografia dos caminhos sensíveis do config — port de `packages/sdk/src/services/encryption.ts`.
//!
//! **Este port tem de ser byte-compatível com o formato já gravado**, não apenas "seguro". Todo
//! usuário que atualiza tem `llm.apiKeys.*` no disco no formato `enc:aes256:…`, cifrado pelo Node
//! com a `master.key` dele. Uma implementação que apenas cifrasse bem, com outros parâmetros, faria
//! o app subir normalmente e as chaves de API simplesmente sumirem — sem erro, sem log, sem nada
//! que um teste de comportamento pegasse.
//!
//! Os parâmetros que precisam bater, e de onde vêm:
//!
//! - **scrypt com N=16384, r=8, p=1, 32 bytes.** São os defaults do `scryptSync` do Node, que o
//!   TypeScript usa sem passar opções. `log_n = 14` porque 2^14 = 16384.
//! - **AES-256-GCM com IV de 16 bytes.** Não é o tamanho canônico do GCM (12); o Node aceita
//!   qualquer tamanho e o código original sorteia 16. Usar 12 aqui geraria tag diferente e
//!   nenhum valor antigo abriria.
//! - **Formato `enc:aes256:<salt>:<iv>:<tag>:<texto>`**, tudo em hex minúsculo, salt de 32 bytes.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::aes::Aes256;
use aes_gcm::{AesGcm, Nonce};
use rand::Rng;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use typenum::U16;
use zeroize::Zeroizing;

/// AES-256-GCM com nonce de 16 bytes, para casar com o `randomBytes(16)` do TypeScript.
type Aes256GcmWide = AesGcm<Aes256, U16>;

const PREFIX: &str = "enc:aes256:";
const SALT_LEN: usize = 32;
const IV_LEN: usize = 16;
const KEY_LEN: usize = 32;

/// Parâmetros do `scryptSync` do Node, que o TypeScript usa sem passar opções.
const SCRYPT_LOG_N: u8 = 14;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;

/// Teto do cache de texto claro: bem acima do número de credenciais do app (~15). Estourou, esvazia.
const PLAIN_CACHE_MAX: usize = 64;

pub struct Encryption {
    /// `Zeroizing` apaga a chave da memória no drop. Não impede um despejo de memória com o
    /// processo vivo, mas tira o segredo de páginas liberadas — barato e vale a pena.
    master_key: Zeroizing<[u8; KEY_LEN]>,
    /// Texto claro por texto cifrado. O scrypt (N=2^14, dezenas de ms) rodava a CADA `get` de
    /// credencial — todo turno de chat/STT/TTS. A chave do cache é o próprio `enc:aes256:…`: valor
    /// novo (salt novo) é entrada nova, e `forget` tira do cache o texto claro de um valor
    /// substituído. Mesmo nível de exposição da `master_key` em memória, e igualmente zerado no drop.
    plain: Mutex<HashMap<String, Zeroizing<String>>>,
    #[cfg(test)]
    derivations: std::sync::atomic::AtomicUsize,
}

impl Encryption {
    fn new(master_key: Zeroizing<[u8; KEY_LEN]>) -> Self {
        Self {
            master_key,
            plain: Mutex::new(HashMap::new()),
            #[cfg(test)]
            derivations: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Carrega `MIKA_MASTER_KEY`, senão `data_dir/master.key`, senão gera uma nova.
    ///
    /// A ordem importa e é a mesma do TypeScript: quem define a variável de ambiente está
    /// apontando para um segredo gerenciado fora do app, e o arquivo no disco não pode vencê-la.
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("master.key");

        if let Ok(from_env) = std::env::var("MIKA_MASTER_KEY") {
            let trimmed = from_env.trim();
            if !trimmed.is_empty() {
                return Ok(Self::new(parse_master_key(trimmed, "MIKA_MASTER_KEY")?));
            }
        }

        match read_key_file(&path) {
            Ok(key) => Ok(Self::new(key)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::generate(&path),
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => Self::recover_unusable_key(data_dir, &path, &error.to_string()),
            Err(error) => Err(format!("não leu a master.key: {error}")),
        }
    }

    /// `master.key` existe mas não é uma chave. VAZIA (resíduo de um boot que morreu no meio da escrita,
    /// versões antigas) e sem NENHUM segredo cifrado para perder: a chave nunca serviu para nada, gera
    /// outra. Qualquer outra coisa — conteúdo truncado, ou segredos que só a chave antiga abre — é erro
    /// que ensina a saída; nada é sobrescrito.
    fn recover_unusable_key(data_dir: &Path, path: &Path, reason: &str) -> Result<Self, String> {
        let empty = fs::read_to_string(path).is_ok_and(|text| text.trim().is_empty());
        if empty && !has_encrypted_secrets(data_dir) {
            log::warn!("master.key vazia e sem segredos cifrados: gerando uma nova");
            fs::remove_file(path).map_err(|e| format!("não removeu a master.key vazia: {e}"))?;
            return Self::generate(path);
        }
        Err(format!(
            "a master.key ({}) está {} — {reason}. Restaure a cópia (backup) dela: sem a chave original as credenciais cifradas \
             (config.json, plugins/mika.mcp/mcp.json) não abrem. Sem backup, apague a master.key e as credenciais cifradas \
             desses arquivos e informe-as de novo.",
            path.display(),
            if empty { "vazia" } else { "corrompida" },
        ))
    }

    /// Publica a `master.key` de forma ATÔMICA e já com 0o600: a chave inteira vai para
    /// `master.key.<pid>.tmp` (0600, `sync_all`) e só então `hard_link` a coloca no nome final — que
    /// falha se já existir, então a chave de outra instância que chegou antes vence (e lemos a dela).
    /// Antes, `create_new` + `write_all` deixava um arquivo VAZIO se a escrita falhasse: todo boot
    /// seguinte morria, e uma segunda instância lia o arquivo ainda vazio. No Windows não há bit POSIX:
    /// a proteção é a ACL herdada da pasta de dados do app (`%APPDATA%`, por usuário); aplicar ACL
    /// própria exigiria `unsafe` do `windows` sem como validar aqui, então fica DITO em vez de simulado.
    fn generate(path: &Path) -> Result<Self, String> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        rand::rng().fill_bytes(key.as_mut_slice());
        let encoded = Zeroizing::new(hex::encode(key.as_slice()));

        let temp = path.with_file_name(format!("master.key.{}.tmp", std::process::id()));
        let written = write_private(&temp, encoded.as_bytes());
        let published = written.and_then(|()| fs::hard_link(&temp, path));
        let _ = fs::remove_file(&temp);

        match published {
            Ok(()) => Ok(Self::new(key)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_existing_key(path).map_err(|e| format!("não leu a master.key: {e}"))?;
                Ok(Self::new(existing))
            }
            Err(error) => Err(format!("não gravou a master.key: {error}")),
        }
    }

    /// Tira do cache o texto claro de um valor cifrado que foi substituído.
    pub fn forget(&self, ciphertext: &str) {
        if let Ok(mut plain) = self.plain.lock() {
            plain.remove(ciphertext);
        }
    }

    fn remember(&self, ciphertext: &str, plaintext: &str) {
        let Ok(mut plain) = self.plain.lock() else { return };
        if plain.len() >= PLAIN_CACHE_MAX {
            plain.clear();
        }
        plain.insert(ciphertext.to_string(), Zeroizing::new(plaintext.to_string()));
    }

    #[cfg(test)]
    fn derivations(&self) -> usize {
        self.derivations.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_encrypted(value: &str) -> bool {
        value.starts_with(PREFIX)
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        let mut salt = [0u8; SALT_LEN];
        let mut iv = [0u8; IV_LEN];
        rand::rng().fill_bytes(&mut salt);
        rand::rng().fill_bytes(&mut iv);

        let derived = self.derive(&salt)?;
        let cipher = Aes256GcmWide::new_from_slice(derived.as_slice()).map_err(|e| e.to_string())?;

        let sealed = cipher
            .encrypt(
                &nonce(&iv)?,
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: &[],
                },
            )
            .map_err(|_| "falha ao cifrar".to_string())?;

        // O `aes-gcm` devolve texto cifrado com a tag concatenada no fim; o formato do Node guarda
        // as duas em campos separados. Dividir aqui é o que mantém os dois lados compatíveis.
        let split = sealed
            .len()
            .checked_sub(16)
            .ok_or("saída cifrada menor que a tag")?;
        let (body, tag) = sealed.split_at(split);

        let sealed_text = format!(
            "{PREFIX}{}:{}:{}:{}",
            hex::encode(salt),
            hex::encode(iv),
            hex::encode(tag),
            hex::encode(body)
        );
        self.remember(&sealed_text, plaintext);
        Ok(sealed_text)
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String, String> {
        if let Some(hit) = self.plain.lock().ok().and_then(|plain| plain.get(ciphertext).map(|p| p.as_str().to_string())) {
            return Ok(hit);
        }
        let plaintext = self.decrypt_uncached(ciphertext)?;
        self.remember(ciphertext, &plaintext);
        Ok(plaintext)
    }

    fn decrypt_uncached(&self, ciphertext: &str) -> Result<String, String> {
        let body = ciphertext
            .strip_prefix(PREFIX)
            .ok_or("formato de texto cifrado inválido")?;

        let parts: Vec<&str> = body.split(':').collect();
        let [salt_hex, iv_hex, tag_hex, encrypted_hex] = parts.as_slice() else {
            return Err(format!(
                "formato inválido: esperava 4 partes, veio {}",
                parts.len()
            ));
        };

        let salt = hex::decode(salt_hex).map_err(|e| format!("salt inválido: {e}"))?;
        let iv = hex::decode(iv_hex).map_err(|e| format!("iv inválido: {e}"))?;
        let tag = hex::decode(tag_hex).map_err(|e| format!("tag inválida: {e}"))?;
        let encrypted = hex::decode(encrypted_hex).map_err(|e| format!("texto inválido: {e}"))?;

        if iv.len() != IV_LEN {
            return Err(format!("iv tem {} bytes, esperava {IV_LEN}", iv.len()));
        }

        let derived = self.derive(&salt)?;
        let cipher = Aes256GcmWide::new_from_slice(derived.as_slice()).map_err(|e| e.to_string())?;

        let mut sealed = encrypted;
        sealed.extend_from_slice(&tag);

        let plaintext = cipher
            .decrypt(
                &nonce(&iv)?,
                Payload {
                    msg: &sealed,
                    aad: &[],
                },
            )
            // A mensagem NÃO diz o que falhou na autenticação, de propósito: detalhe de falha de
            // GCM é material de oráculo. O diagnóstico útil para o usuário é o de cima — a
            // master.key não corresponde a este config.
            .map_err(|_| {
                "credencial ilegível — a master.key não corresponde ao config cifrado".to_string()
            })?;

        String::from_utf8(plaintext).map_err(|_| "texto decifrado não é UTF-8".to_string())
    }

    fn derive(&self, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
        // `Params::new` recebe 3 argumentos nesta versão; o tamanho da chave derivada vem do
        // buffer de saída passado a `scrypt()`, e é ele que tem de valer KEY_LEN.
        #[cfg(test)]
        self.derivations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let params = scrypt::Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P)
            .map_err(|e| format!("parâmetros de scrypt inválidos: {e}"))?;

        let mut derived = Zeroizing::new([0u8; KEY_LEN]);
        scrypt::scrypt(self.master_key.as_slice(), salt, &params, derived.as_mut_slice())
            .map_err(|e| format!("derivação falhou: {e}"))?;

        Ok(derived)
    }
}

/// `Nonce::from_slice` está depreciado no `aes-gcm` 0.11 — ele entrava em pânico com tamanho
/// errado, e a conversão falível devolve erro em vez de derrubar o app com config corrompido.
fn nonce(iv: &[u8]) -> Result<Nonce<U16>, String> {
    Nonce::<U16>::try_from(iv).map_err(|_| format!("iv tem {} bytes, esperava {IV_LEN}", iv.len()))
}

fn parse_master_key(raw: &str, origin: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    let bytes = hex::decode(raw).map_err(|_| {
        format!("{origin} precisa ser {} caracteres hex ({KEY_LEN} bytes)", KEY_LEN * 2)
    })?;

    let key: [u8; KEY_LEN] = bytes.try_into().map_err(|_| {
        format!("{origin} precisa ser {} caracteres hex ({KEY_LEN} bytes)", KEY_LEN * 2)
    })?;

    Ok(Zeroizing::new(key))
}

/// Cria `path` (truncando resíduo de um boot anterior) já 0600 e grava `bytes` até o disco.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Lê a chave que outra instância publicou. Algumas tentativas curtas: um arquivo ainda vazio é de uma
/// versão antiga que escrevia no lugar (a atual só publica a chave inteira).
fn read_existing_key(path: &Path) -> Result<Zeroizing<[u8; KEY_LEN]>, std::io::Error> {
    let mut last = None;
    for _ in 0..5 {
        match read_key_file(path) {
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => last = Some(error),
            other => return other,
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(last.expect("o laço só termina por aqui depois de um InvalidData"))
}

/// Há `enc:aes256:` em algum arquivo que só a `master.key` original abre? (config e `mcp.json`.)
fn has_encrypted_secrets(data_dir: &Path) -> bool {
    ["config.json", "plugins/mika.mcp/mcp.json"]
        .iter()
        .any(|file| fs::read_to_string(data_dir.join(file)).is_ok_and(|text| text.contains(PREFIX)))
}

/// Lê e valida a `master.key`; o texto lido (a chave em hex) é zerado ao sair.
fn read_key_file(path: &Path) -> Result<Zeroizing<[u8; KEY_LEN]>, std::io::Error> {
    let body = Zeroizing::new(fs::read_to_string(path)?);
    parse_master_key(body.trim(), &path.display().to_string()).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_key(hex_key: &str) -> Encryption {
        Encryption::new(parse_master_key(hex_key, "teste").unwrap())
    }

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[test]
    fn round_trips() {
        let encryption = with_key(KEY);
        let sealed = encryption.encrypt("sk-um-segredo").unwrap();

        assert!(Encryption::is_encrypted(&sealed));
        assert_eq!(encryption.decrypt(&sealed).unwrap(), "sk-um-segredo");
    }

    /// O formato é contrato com os dados já gravados em disco: prefixo, quatro campos, hex, e os
    /// tamanhos exatos de salt e IV. Qualquer desvio aqui torna ilegível o config de quem atualiza.
    #[test]
    fn format_matches_the_typescript_layout() {
        let sealed = with_key(KEY).encrypt("x").unwrap();
        let body = sealed.strip_prefix("enc:aes256:").expect("prefixo errado");
        let parts: Vec<&str> = body.split(':').collect();

        assert_eq!(parts.len(), 4, "esperava salt:iv:tag:texto");
        assert_eq!(parts[0].len(), SALT_LEN * 2, "salt não tem 32 bytes");
        assert_eq!(parts[1].len(), IV_LEN * 2, "iv não tem 16 bytes");
        assert_eq!(parts[2].len(), 32, "tag do GCM não tem 16 bytes");
        assert!(parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit())));
    }

    /// Cada escrita sorteia salt e IV novos — dois valores iguais não podem produzir o mesmo
    /// texto cifrado, senão dá para comparar segredos sem decifrar nenhum.
    #[test]
    fn same_plaintext_never_repeats_the_ciphertext() {
        let encryption = with_key(KEY);
        assert_ne!(
            encryption.encrypt("igual").unwrap(),
            encryption.encrypt("igual").unwrap()
        );
    }

    /// Chave errada tem de FALHAR, não devolver lixo. E a mensagem aponta a master.key, que é o
    /// diagnóstico acionável, sem detalhar a falha de autenticação do GCM.
    #[test]
    fn wrong_key_fails_loudly() {
        let sealed = with_key(KEY).encrypt("segredo").unwrap();
        let other = with_key("ff00000000000000000000000000000000000000000000000000000000000000");

        let error = other.decrypt(&sealed).unwrap_err();
        assert!(error.contains("master.key"), "erro pouco acionável: {error}");
        assert!(!error.contains("tag"), "vaza detalhe de autenticação: {error}");
    }

    /// GCM é autenticado: um byte trocado no texto cifrado tem de recusar, não decifrar torto.
    #[test]
    fn tampered_ciphertext_is_rejected() {
        let encryption = with_key(KEY);
        let sealed = encryption.encrypt("segredo").unwrap();

        let mut tampered: Vec<char> = sealed.chars().collect();
        let last = tampered.len() - 1;
        tampered[last] = if tampered[last] == 'a' { 'b' } else { 'a' };

        assert!(encryption.decrypt(&tampered.into_iter().collect::<String>()).is_err());
    }

    #[test]
    fn malformed_input_is_rejected() {
        let encryption = with_key(KEY);

        assert!(encryption.decrypt("texto puro").is_err());
        assert!(encryption.decrypt("enc:aes256:só:tres:partes").is_err());
        assert!(encryption.decrypt("enc:aes256:zz:zz:zz:zz").is_err());
    }

    /// scrypt N=2^14 custa dezenas de ms: lido a cada `get` de credencial (todo turno de chat/STT/TTS),
    /// o texto claro tem de vir do cache depois da primeira decifragem.
    #[test]
    fn repeated_decrypt_of_the_same_ciphertext_derives_once() {
        let encryption = with_key(KEY);
        let sealed = encryption.encrypt("sk-um-segredo").unwrap();
        let after_encrypt = encryption.derivations();

        assert_eq!(encryption.decrypt(&sealed).unwrap(), "sk-um-segredo");
        assert_eq!(encryption.decrypt(&sealed).unwrap(), "sk-um-segredo");
        // cifrar já semeia o cache com o texto claro: ler logo depois do `set` não deriva de novo
        assert_eq!(encryption.derivations(), after_encrypt, "decifrar derivou scrypt de novo");

        let fresh = with_key(KEY);
        assert_eq!(fresh.decrypt(&sealed).unwrap(), "sk-um-segredo");
        assert_eq!(fresh.decrypt(&sealed).unwrap(), "sk-um-segredo");
        assert_eq!(fresh.derivations(), 1, "duas leituras do mesmo valor derivam uma vez só");
    }

    #[test]
    fn forgetting_a_ciphertext_drops_its_cached_plaintext() {
        let fresh = with_key(KEY);
        let sealed = with_key(KEY).encrypt("antigo").unwrap();
        fresh.decrypt(&sealed).unwrap();
        fresh.forget(&sealed);
        fresh.decrypt(&sealed).unwrap();
        assert_eq!(fresh.derivations(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn generated_master_key_is_owner_only_and_never_overwrites_an_existing_one() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("mika-crypto-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("master.key");
        let _ = fs::remove_file(&path);

        let first = Encryption::generate(&path).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);

        // outro processo criou o arquivo entre o `NotFound` e o `generate`: a chave dele vence
        let second = Encryption::generate(&path).unwrap();
        assert_eq!(first.master_key.as_slice(), second.master_key.as_slice());
        assert_eq!(fs::read_to_string(&path).unwrap(), hex::encode(first.master_key.as_slice()));
        let _ = fs::remove_dir_all(&dir);
    }

    fn scratch_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mika-crypto-open-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Crash no meio de um boot anterior deixa o temporário para trás: o próximo boot não tropeça nele
    /// e a `master.key` publicada está inteira (64 hex), sem sobra do temporário deste processo.
    #[test]
    fn a_temp_file_left_by_a_crashed_boot_does_not_break_the_next_one() {
        let dir = scratch_dir();
        fs::write(dir.join(format!("master.key.{}.tmp", std::process::id())), b"").unwrap();
        fs::write(dir.join("master.key.99999999.tmp"), b"12").unwrap();

        let opened = Encryption::open(&dir).unwrap();

        let on_disk = fs::read_to_string(dir.join("master.key")).unwrap();
        assert_eq!(on_disk, hex::encode(opened.master_key.as_slice()));
        assert!(!dir.join(format!("master.key.{}.tmp", std::process::id())).exists());
        assert_eq!(Encryption::open(&dir).unwrap().master_key.as_slice(), opened.master_key.as_slice());
        let _ = fs::remove_dir_all(&dir);
    }

    /// O defeito: `create_new` + falha do `write_all` deixava um `master.key` VAZIO e todo boot seguinte
    /// morria em "não leu a master.key". Vazio sem nenhum segredo cifrado para perder: a chave nunca
    /// serviu para nada, gera uma nova.
    #[test]
    fn an_empty_master_key_with_no_encrypted_secrets_is_regenerated() {
        let dir = scratch_dir();
        fs::write(dir.join("master.key"), b"").unwrap();
        fs::write(dir.join("config.json"), br#"{"llm":{"provider":"ollama"}}"#).unwrap();

        let opened = Encryption::open(&dir).unwrap();

        assert_eq!(fs::read_to_string(dir.join("master.key")).unwrap(), hex::encode(opened.master_key.as_slice()));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Com credencial cifrada no config (ou no `mcp.json`), regenerar apagaria o que só a chave antiga
    /// abre: o boot falha NOMEANDO o arquivo e a saída, e não toca em nada.
    #[test]
    fn an_empty_master_key_next_to_encrypted_secrets_is_an_error_that_teaches_the_recovery() {
        for secret_file in ["config.json", "plugins/mika.mcp/mcp.json"] {
            let dir = scratch_dir();
            fs::write(dir.join("master.key"), b"  \n").unwrap();
            let path = dir.join(secret_file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, br#"{"k":"enc:aes256:aa:bb:cc:dd"}"#).unwrap();

            let error = Encryption::open(&dir).err().expect("não pode regenerar por cima de segredos cifrados");

            assert!(error.contains("master.key") && error.contains("backup"), "{error}");
            assert_eq!(fs::read(dir.join("master.key")).unwrap(), b"  \n", "o arquivo não pode ser tocado");
            let _ = fs::remove_dir_all(&dir);
        }
    }

    /// Conteúdo que não é uma chave (não só vazio) nunca é sobrescrito: pode ser a chave de alguém, truncada.
    #[test]
    fn a_corrupt_non_empty_master_key_is_never_overwritten() {
        let dir = scratch_dir();
        fs::write(dir.join("master.key"), b"abc123").unwrap();

        assert!(Encryption::open(&dir).is_err());
        assert_eq!(fs::read(dir.join("master.key")).unwrap(), b"abc123");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn master_key_must_be_32_bytes_of_hex() {
        assert!(parse_master_key("abc", "teste").is_err());
        assert!(parse_master_key("xyz", "teste").is_err());
        assert!(parse_master_key(KEY, "teste").is_ok());
    }
}
