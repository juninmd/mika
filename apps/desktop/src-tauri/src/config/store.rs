//! Leitura, escrita e persistência da configuração.
//!
//! Equivale a `packages/sdk/src/services/store.ts`. As diferenças de comportamento em relação ao
//! original são deliberadas e estão comentadas onde aparecem; o resto é paridade.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Map, Value};

use super::atomic::write_atomic;
use super::crypto::Encryption;
use super::registry::SchemaRegistry;

/// Prefixo das credenciais de provedor. É `apiKeys` na RAIZ, não `llm.apiKeys` — vem de
/// `ConfigKey.ApiKeys = 'apiKeys'`.
///
/// Em MINÚSCULAS porque a comparação é feita sobre o caminho já rebaixado. Escrito com o K
/// maiúsculo do original, o `starts_with` nunca casava e toda credencial de provedor ia para o
/// disco em texto claro — sem erro, com o app funcionando. Foi o teste que lê o arquivo gravado
/// que pegou; nenhum teste de `get`/`set` conseguiria.
const API_KEYS_PREFIX: &str = "apikeys.";

/// Sufixos que marcam credencial, de `CREDENTIAL_SUFFIX_RE` em
/// `packages/sdk/src/contracts/schemas/config-keys.enum.ts`.
const CREDENTIAL_SUFFIXES: [&str; 4] = ["token", "secret", "password", "key"];

/// Um caminho guarda segredo?
///
/// **A primeira versão deste port listava dois prefixos fixos e protegia de MENOS.** A regra real do
/// TypeScript é por sufixo, e alcança 11 caminhos, não 2: `voice.groqApiKey`,
/// `voice.azureTtsSubscriptionKey`, `voice.googleTtsApiKey` e os demais ficariam em texto claro no
/// disco — com o app funcionando perfeitamente, que é exatamente o que torna esta classe de erro
/// cara de achar.
///
/// A exceção do `url` importa: `apiKeys.openaiCompatibleBaseUrl` é endereço, não credencial.
/// Cifrá-lo esconderia do usuário o endpoint que ele mesmo configurou.
fn is_sensitive(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();

    if lower.starts_with(API_KEYS_PREFIX) {
        return !lower.ends_with("url");
    }

    CREDENTIAL_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix))
}

/// Chaves que envenenariam o protótipo do objeto do lado JavaScript. O config vai inteiro para o
/// renderer, então a defesa continua valendo mesmo com o store em Rust.
const DANGEROUS_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// Observador de mudança de config — porte de `ctx.config.onChange(ConfigKey.X, …)` do host de
/// plugin do TS. Recebe o caminho que mudou e o valor **como ficou gravado**.
///
/// Recebe o valor MASCARADO quando o caminho é sensível, pela mesma razão que `get_all` mascara:
/// observador é código arbitrário de módulo, e vazar credencial para todos eles anularia o cuidado
/// que `get` toma de só decifrar no caminho específico que alguém pediu.
pub type ConfigWatcher = Box<dyn Fn(&str, &Value) + Send + Sync>;

pub struct ConfigStore {
    schema: SchemaRegistry,
    path: PathBuf,
    cache: Mutex<Value>,
    encryption: Encryption,
    watchers: Mutex<Vec<ConfigWatcher>>,
}

impl ConfigStore {
    /// Abre (ou cria) o config em `data_dir/config.json`, mesclado sobre os defaults do schema.
    pub fn open(data_dir: &Path, schema: SchemaRegistry) -> Result<Self, String> {
        fs::create_dir_all(data_dir).map_err(|e| format!("não criou {}: {e}", data_dir.display()))?;

        let path = data_dir.join("config.json");
        let mut stored = match read_json(&path) {
            Ok(stored) => stored,
            // Só config ILEGÍVEL como JSON cai no `.bak`; erro de IO sobe sem tocar em nada.
            Err(ReadError::Parse(error)) => recover_from_backup(&path, &error)?,
            Err(ReadError::Io(error)) => return Err(error),
        };
        let migrated = migrate_config(&mut stored)?;
        let cache = merge_over_defaults(schema.defaults(), stored);

        let store = Self {
            schema,
            path,
            cache: Mutex::new(cache.clone()),
            encryption: Encryption::open(data_dir)?,
            watchers: Mutex::new(Vec::new()),
        };
        // Grava o carimbo na hora: sem isto um config legado só ganharia `schemaVersion` no primeiro
        // `set`, e um app mais antigo aberto antes disso não teria como ver que o arquivo foi migrado.
        if migrated {
            if let Err(error) = store.save(&cache) {
                log::warn!("config: migração para v{CONFIG_SCHEMA_VERSION} não gravada: {error}");
            }
        }
        Ok(store)
    }

    /// Lê um valor, decifrando se o caminho for sensível.
    ///
    /// Credencial ilegível vira `None`, não o texto cifrado. Devolver o `enc:aes256:…` faria o
    /// provedor tentar autenticar com ele e falhar com "chave inválida" — mandando o usuário
    /// procurar o problema na conta dele em vez de na `master.key`.
    pub fn get(&self, path: &str) -> Option<Value> {
        let raw = {
            let cache = self.cache.lock().ok()?;
            read_path(&cache, path).cloned()?
        };

        let Some(text) = raw.as_str() else {
            return Some(raw);
        };

        if !is_sensitive(path) || !Encryption::is_encrypted(text) {
            return Some(raw);
        }

        match self.encryption.decrypt(text) {
            Ok(plain) => Some(Value::String(plain)),
            Err(error) => {
                log::error!("🔐 {path}: {error}");
                None
            }
        }
    }

    /// O config inteiro, com os segredos MASCARADOS.
    ///
    /// Isto vai para o renderer e para o painel de Logs. O TypeScript decifrava aqui; manter isso
    /// faria a chave de API trafegar por IPC e aparecer em qualquer despejo de estado da UI. Quem
    /// precisa do valor real chama `get` no caminho específico, do lado do backend.
    pub fn get_all(&self) -> Value {
        let Ok(cache) = self.cache.lock() else {
            return Value::Null;
        };
        let mut snapshot = cache.clone();
        drop(cache);

        mask_sensitive(&mut snapshot, "");
        snapshot
    }

    /// Cifra um segredo que NÃO mora no `config.json` (hoje: o `env` dos servidores de `mcp.json`)
    /// com a MESMA `master.key` — um segundo esquema de cifra seria uma segunda chave para guardar.
    pub fn seal_secret(&self, plain: &str) -> Result<String, String> {
        if Encryption::is_encrypted(plain) {
            return Ok(plain.to_string());
        }
        self.encryption.encrypt(plain)
    }

    /// Inverso de [`Self::seal_secret`]. Texto sem o prefixo `enc:aes256:` (arquivo escrito à mão,
    /// anterior à cifra) volta como está — quem migra regrava cifrado.
    pub fn open_secret(&self, text: &str) -> Result<String, String> {
        if Encryption::is_encrypted(text) {
            self.encryption.decrypt(text)
        } else {
            Ok(text.to_string())
        }
    }

    /// Grava um valor validado. Devolve erro legível — ele sobe até o renderer, que sem isso teria
    /// só um controle que não obedece.
    pub fn set(&self, path: &str, value: Value) -> Result<(), String> {
        if path.split('.').any(|key| DANGEROUS_KEYS.contains(&key)) {
            return Err(format!("{path}: caminho recusado"));
        }

        self.schema.validate(path, &value)?;

        // Cifra DEPOIS de validar: validar o texto cifrado compararia a credencial contra as
        // restrições do schema e recusaria qualquer chave válida por tamanho.
        let sensitive = is_sensitive(path);
        let stored = match (sensitive, value.as_str()) {
            (true, Some(plain)) if !Encryption::is_encrypted(plain) => {
                Value::String(self.encryption.encrypt(plain)?)
            }
            _ => value,
        };

        // O lock fica preso durante a gravação em disco (não só durante a mutação em memória): o
        // wizard dispara vários `saveSetting` em paralelo (`Promise.all`), e duas gravações
        // concorrentes no MESMO `config.json.tmp` competiam pelo arquivo — no Windows isso é
        // "os error 32" (sharing violation), `save` devolvia `Err` e o renderer via
        // "Falha ao salvar as configurações iniciais" mesmo com o valor validado e correto. Serializar
        // aqui também evita perder a gravação mais recente por ordem de término em vez de ordem causal.
        // Custo conhecido: o `fs::copy` do `.bak` + `sync_all` rodam sob o lock, então um `get` concorrente
        // espera o disco. Soltar o lock exigiria número de geração + descarte de snapshot velho + rollback
        // do cache sob escrita concorrente — mais código e mais corrida que a latência que se compraria.
        let mut cache = self.cache.lock().map_err(|_| "config store envenenado")?;
        let previous = cache.clone();
        write_path(&mut cache, path, stored);
        let snapshot = cache.clone();
        if let Err(error) = self.save(&snapshot) {
            // Falhou no disco: a memória volta ao que o disco tem, senão o `get` seguinte mostraria um
            // valor que o próximo boot não vê (e o renderer já recebeu o erro).
            *cache = previous;
            return Err(error);
        }
        let replaced = read_path(&previous, path).and_then(Value::as_str).filter(|old| sensitive && Encryption::is_encrypted(old));
        if let Some(old) = replaced {
            self.encryption.forget(old);
        }
        drop(cache);

        // Observadores DEPOIS do save, e só se ele deu certo: notificar antes faria um módulo
        // reagir a uma mudança que o próximo boot não veria — MCP derrubaria uma conexão viva por
        // causa de um `offlineMode` que nunca chegou ao disco.
        self.notify(path);
        Ok(())
    }

    /// Registra um observador. Sem `unsubscribe`: o `ConfigStore` é singleton de processo e vive o
    /// app inteiro, então quem registra no boot nunca precisa cancelar — e um handle de cancelamento
    /// que ninguém usa é indireção que só existe para ser testada.
    pub fn watch(&self, watcher: ConfigWatcher) {
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.push(watcher);
        }
    }

    /// Lê o valor já mascarado e entrega a cada observador.
    ///
    /// O lock de `watchers` é solto ANTES de chamar qualquer um: um observador que chame `set` (ou
    /// `watch`) de dentro do callback travaria contra o próprio mutex, e o deadlock apareceria como
    /// app congelado sem uma linha de log.
    fn notify(&self, path: &str) {
        let masked = {
            let Ok(cache) = self.cache.lock() else { return };
            let mut snapshot = cache.clone();
            drop(cache);
            mask_sensitive(&mut snapshot, "");
            read_path(&snapshot, path).cloned().unwrap_or(Value::Null)
        };

        let taken = {
            let Ok(mut watchers) = self.watchers.lock() else { return };
            std::mem::take(&mut *watchers)
        };

        for watcher in &taken {
            watcher(path, &masked);
        }

        if let Ok(mut watchers) = self.watchers.lock() {
            // Devolve os observadores na frente de quem tenha se registrado durante a notificação,
            // para preservar a ordem de registro.
            let registered_meanwhile = std::mem::take(&mut *watchers);
            *watchers = taken;
            watchers.extend(registered_meanwhile);
        }
    }

    fn save(&self, snapshot: &Value) -> Result<(), String> {
        let body = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;

        // Cópia da última versão boa ANTES de substituí-la: `open` cai nela se o config.json vier
        // corrompido (disco cheio, queda de energia, edição à mão). Sem arquivo ainda, não há o que copiar.
        if self.path.exists() {
            if let Err(error) = fs::copy(&self.path, backup_path(&self.path)) {
                log::warn!("config: backup {} não gravado: {error}", backup_path(&self.path).display());
            }
        }
        // Escrita atômica com `sync_all`: um encerramento no meio de um `write` direto deixaria o
        // config truncado, e o próximo boot cairia inteiro para os defaults.
        write_atomic(&self.path, body.as_bytes()).map_err(|e| format!("não gravou o config: {e}"))
    }

    pub fn schema(&self) -> &SchemaRegistry {
        &self.schema
    }

    /// Pasta de dados do perfil (onde fica `config.json`) — para estado local pequeno que não é
    /// config (`body-state.json`, `usage-habits.json`).
    pub fn data_dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }
}

/// Passos de migração do `config.json`: `CONFIG_MIGRATIONS[n]` leva o arquivo de v`n` para v`n+1`.
/// v0 é o arquivo gravado antes do campo `schemaVersion` existir; v1 é só o carimbo (a forma do
/// arquivo não mudou). Mudança incompatível de chave/forma = um passo novo aqui, nunca editar um
/// passo já distribuído.
const CONFIG_MIGRATIONS: &[fn(&mut Map<String, Value>)] = &[|_| {}, drop_untouched_capture_blocklist, keep_previous_defaults, drop_retired_piper_voice];

/// v3→v4: vozes Piper que saíram do catálogo (`piper/catalog.rs`: licença do dataset não verificada
/// ou só-pesquisa). Quem as tinha salva perde a chave e ganha o default; voz do catálogo fica.
const RETIRED_PIPER_VOICES: [&str; 2] = ["en_US-lessac-high", "en_US-amy-medium"];

fn drop_retired_piper_voice(root: &mut Map<String, Value>) {
    let Some(voice) = root.get_mut("voice").and_then(Value::as_object_mut) else { return };
    if voice.get("piperVoice").and_then(Value::as_str).is_some_and(|saved| RETIRED_PIPER_VOICES.contains(&saved)) {
        voice.remove("piperVoice");
    }
}

/// v2→v3: `browser.enabled` e `computer-use.enabled` passaram a nascer `false` (PRIVACY.md §3), mas
/// um config antigo PARCIAL — sem a chave — tinha os recursos LIGADOS pelo default de então, e o
/// `open` regrava o snapshot: sem este passo o valor efetivo se perdia para sempre. Config vazio
/// (perfil novo) fica de fora e nasce desligado.
///
/// `person-catalog.webEnrichment` NÃO é preservado (decisão de privacidade: manda nomes de pessoas à
/// web); o perfil existente ganha só o aviso pendente que o painel mostra uma vez.
fn keep_previous_defaults(root: &mut Map<String, Value>) {
    if root.is_empty() {
        return;
    }
    for namespace in ["browser", "computer-use"] {
        let section = root.entry(namespace).or_insert_with(|| Value::Object(Map::new()));
        if let Some(section) = section.as_object_mut() {
            section.entry("enabled").or_insert(Value::Bool(true));
        }
    }
    if let Some(catalog) = root.entry("person-catalog").or_insert_with(|| Value::Object(Map::new())).as_object_mut() {
        if !catalog.contains_key("webEnrichment") {
            catalog.insert("webEnrichmentNoticePending".into(), Value::Bool(true));
        }
    }
}

/// A lista de bloqueio de captura que TODO perfil antigo tem gravada (o config grava o snapshot
/// inteiro, então "nunca editou" e "gravou o default" são o mesmo byte). v1→v2: quem ainda tem
/// exatamente ela perde a chave e ganha o default novo (mais ampla: e-mail, mensageiros, navegação
/// privada, carteiras, saúde, autenticadores); lista editada pelo usuário fica como está.
const LEGACY_CAPTURE_BLOCKLIST: &str = "bitwarden, 1password, keepass, lastpass, dashlane, proton pass, keeper, nordpass, banco, bank, itau, nubank, bradesco, santander, caixa, inter, internet banking";

fn drop_untouched_capture_blocklist(root: &mut Map<String, Value>) {
    let Some(vision) = root.get_mut("vision").and_then(Value::as_object_mut) else { return };
    if vision.get("captureBlocklist").and_then(Value::as_str) == Some(LEGACY_CAPTURE_BLOCKLIST) {
        vision.remove("captureBlocklist");
    }
}
const CONFIG_SCHEMA_VERSION: u64 = CONFIG_MIGRATIONS.len() as u64;
const SCHEMA_VERSION_KEY: &str = "schemaVersion";

/// Leva o config lido do disco à versão atual e o carimba. `Ok(true)` quando mudou algo (arquivo
/// existente em versão antiga); config vazio (primeiro boot) é carimbado mas não conta como migração,
/// porque não há arquivo a regravar. Recusa um arquivo de versão MAIS NOVA que a suportada: seguir
/// adiante regravaria o config perdendo campos que este app não conhece.
fn migrate_config(stored: &mut Value) -> Result<bool, String> {
    let Some(root) = stored.as_object_mut() else {
        return Ok(false);
    };
    let existing = !root.is_empty();
    let version = root.get(SCHEMA_VERSION_KEY).and_then(Value::as_u64).unwrap_or(0);
    if version > CONFIG_SCHEMA_VERSION {
        return Err(format!(
            "config.json é do schema v{version}, mas esta versão do Mika só entende até v{CONFIG_SCHEMA_VERSION} — atualize o Mika"
        ));
    }
    for step in &CONFIG_MIGRATIONS[version as usize..] {
        step(root);
    }
    root.insert(SCHEMA_VERSION_KEY.into(), Value::from(CONFIG_SCHEMA_VERSION));
    Ok(existing && version < CONFIG_SCHEMA_VERSION)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

/// `config.json` ilegível: usa o `.bak` (a versão anterior ao último save) e restaura o arquivo. Sem
/// `.bak` utilizável devolve o erro ORIGINAL — nunca um reset silencioso para os defaults.
fn recover_from_backup(path: &Path, original: &str) -> Result<Value, String> {
    let backup = backup_path(path);
    let recovered = match read_json(&backup) {
        Ok(recovered) if backup.exists() => recovered,
        _ => return Err(original.to_string()),
    };
    log::warn!("config: {original}; recuperado de {}", backup.display());
    if let Err(error) = fs::copy(&backup, path) {
        log::warn!("config: {} não restaurado a partir do backup: {error}", path.display());
    }
    Ok(recovered)
}

/// Por que o arquivo não virou JSON: `Io` (permissão, antivírus segurando o arquivo) não diz nada
/// sobre o CONTEÚDO — só `Parse` justifica trocar o config pelo `.bak`.
enum ReadError {
    Io(String),
    Parse(String),
}

fn read_json(path: &Path) -> Result<Value, ReadError> {
    match fs::read_to_string(path) {
        Ok(body) => serde_json::from_str(&body).map_err(|e| ReadError::Parse(format!("config.json inválido: {e}"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(error) => Err(ReadError::Io(format!("não leu o config: {error}"))),
    }
}

/// Mescla o que está em disco por cima dos defaults, em profundidade.
///
/// Campo novo que o usuário nunca viu nasce com o default, e campo que ele configurou é preservado.
/// Substituir o objeto inteiro (em vez de mesclar) faria toda versão nova zerar a configuração dos
/// namespaces que ganharam campos.
fn merge_over_defaults(defaults: Value, stored: Value) -> Value {
    match (defaults, stored) {
        (Value::Object(mut base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                let merged = match base.remove(&key) {
                    Some(existing) => merge_over_defaults(existing, value),
                    None => value,
                };
                base.insert(key, merged);
            }
            Value::Object(base)
        }
        (_, stored) => stored,
    }
}

fn read_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(root, |node, key| node.get(key))
}

fn write_path(root: &mut Value, path: &str, value: Value) {
    let keys: Vec<&str> = path.split('.').collect();
    let Some((last, parents)) = keys.split_last() else {
        return;
    };

    let mut node = root;
    for key in parents {
        if !node.is_object() {
            *node = Value::Object(Map::new());
        }
        node = node
            .as_object_mut()
            .expect("acabou de virar objeto")
            .entry(key.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    node.as_object_mut()
        .expect("acabou de virar objeto")
        .insert(last.to_string(), value);
}

/// Substitui por `[REDACTED]` todo valor cujo caminho seja sensível.
///
/// Percorre a árvore INTEIRA em vez de consultar uma lista de caminhos conhecidos. Sob `apiKeys.` as
/// chaves são descobertas em runtime (um provedor novo aparece sem ninguém declarar nada), e a regra
/// de sufixo alcança qualquer `…ApiKey` que um módulo futuro venha a criar. Uma lista fixa deixaria
/// passar exatamente o caso que ninguém lembrou de acrescentar.
fn mask_sensitive(node: &mut Value, path: &str) {
    let Some(object) = node.as_object_mut() else {
        return;
    };

    for (key, child) in object.iter_mut() {
        let full = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };

        if is_sensitive(&full) && !child.is_object() {
            *child = Value::String("[REDACTED]".into());
        } else {
            mask_sensitive(child, &full);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry::Namespace;
    use super::super::schema::{boolean, choice, int_range, text};
    use super::*;
    use serde_json::json;

    fn schema() -> SchemaRegistry {
        SchemaRegistry::new(vec![
            Namespace {
                name: "memory",
                fields: vec![
                    boolean("enabled", true),
                    choice("backend", "embedded", &["embedded", "redis"]),
                    int_range("episodicListLimit", 500, 1, 5000),
                ],
            },
            Namespace {
                name: "llm",
                fields: vec![text("provider", "ollama")],
            },
            Namespace {
                name: "voice",
                fields: vec![text("groqApiKey", "")],
            },
            Namespace {
                name: "apiKeys",
                fields: vec![text("openai", ""), text("openaiCompatibleBaseUrl", "")],
            },
        ])
    }

    fn store() -> (ConfigStore, tempdir::Dir) {
        let dir = tempdir::Dir::new();
        let store = ConfigStore::open(dir.path(), schema()).unwrap();
        (store, dir)
    }

    #[test]
    fn starts_from_defaults() {
        let (store, _dir) = store();

        assert_eq!(store.get("memory.enabled"), Some(json!(true)));
        assert_eq!(store.get("memory.backend"), Some(json!("embedded")));
        assert_eq!(store.get("llm.provider"), Some(json!("ollama")));
    }

    #[test]
    fn set_then_get_round_trips() {
        let (store, _dir) = store();

        store.set("memory.backend", json!("redis")).unwrap();
        assert_eq!(store.get("memory.backend"), Some(json!("redis")));
    }

    #[test]
    fn set_validates_against_the_schema() {
        let (store, _dir) = store();

        assert!(store.set("memory.backend", json!("postgres")).is_err());
        assert!(store.set("memory.episodicListLimit", json!(99999)).is_err());
        // e o valor anterior continua de pé
        assert_eq!(store.get("memory.backend"), Some(json!("embedded")));
    }

    /// O segredo NUNCA pode aparecer legível no arquivo. É a asserção que se lê no disco, não na
    /// API: um `get` que devolve o valor certo não prova nada sobre o que foi gravado.
    #[test]
    fn secret_is_never_written_in_plaintext() {
        let dir = tempdir::Dir::new();
        let store = ConfigStore::open(dir.path(), schema()).unwrap();

        store.set("apiKeys.openai", json!("sk-um-segredo-real")).unwrap();

        let on_disk = fs::read_to_string(dir.path().join("config.json")).unwrap();
        assert!(
            !on_disk.contains("sk-um-segredo-real"),
            "a credencial foi para o disco em texto claro"
        );
        assert!(on_disk.contains("enc:aes256:"), "não cifrou: {on_disk}");
    }

    /// E quem lê pelo caminho certo recebe o valor de volta — cifrar sem conseguir decifrar seria
    /// só uma forma elaborada de perder a chave do usuário.
    #[test]
    fn secret_round_trips_through_the_store() {
        let (store, _dir) = store();

        store.set("apiKeys.openai", json!("sk-um-segredo-real")).unwrap();
        assert_eq!(store.get("apiKeys.openai"), Some(json!("sk-um-segredo-real")));
    }

    /// `get_all` alimenta o renderer e o painel de Logs. O TypeScript decifrava aqui; manter isso
    /// faria a credencial trafegar por IPC e aparecer em qualquer despejo de estado da UI.
    #[test]
    fn get_all_masks_secrets() {
        let (store, _dir) = store();

        store.set("apiKeys.openai", json!("sk-um-segredo-real")).unwrap();

        let everything = serde_json::to_string(&store.get_all()).unwrap();
        assert!(!everything.contains("sk-um-segredo-real"), "vazou em get_all");
        assert!(!everything.contains("enc:aes256:"), "vazou o texto cifrado em get_all");
        assert!(everything.contains("[REDACTED]"), "não mascarou: {everything}");
    }

    /// A regra de sensibilidade é por SUFIXO, não uma lista de caminhos. Foi errar isto que deixou
    /// `voice.groqApiKey` e mais nove caminhos em texto claro na primeira versão do port.
    #[test]
    fn suffix_rule_reaches_credentials_outside_api_keys() {
        let dir = tempdir::Dir::new();
        let store = ConfigStore::open(dir.path(), schema()).unwrap();

        store.set("voice.groqApiKey", json!("gsk-segredo")).unwrap();

        let on_disk = fs::read_to_string(dir.path().join("config.json")).unwrap();
        assert!(!on_disk.contains("gsk-segredo"), "credencial fora de apiKeys ficou legível");
        assert_eq!(store.get("voice.groqApiKey"), Some(json!("gsk-segredo")));
    }

    /// Endereço não é credencial. Cifrar um `…BaseUrl` esconderia do usuário o endpoint que ele
    /// mesmo configurou, e o painel mostraria `[REDACTED]` no lugar de um dado que é dele.
    #[test]
    fn urls_under_api_keys_stay_readable() {
        let dir = tempdir::Dir::new();
        let store = ConfigStore::open(dir.path(), schema()).unwrap();

        store
            .set("apiKeys.openaiCompatibleBaseUrl", json!("http://localhost:1234"))
            .unwrap();

        let on_disk = fs::read_to_string(dir.path().join("config.json")).unwrap();
        assert!(on_disk.contains("http://localhost:1234"), "cifrou um endereço");

        let everything = serde_json::to_string(&store.get_all()).unwrap();
        assert!(everything.contains("http://localhost:1234"), "mascarou um endereço");
    }

    /// Reabrir tem de continuar lendo a credencial: é o caso do usuário que fecha e abre o app.
    #[test]
    fn secret_survives_reopen() {
        let dir = tempdir::Dir::new();

        let first = ConfigStore::open(dir.path(), schema()).unwrap();
        first.set("apiKeys.openai", json!("sk-persistente")).unwrap();
        drop(first);

        let second = ConfigStore::open(dir.path(), schema()).unwrap();
        assert_eq!(second.get("apiKeys.openai"), Some(json!("sk-persistente")));
    }

    /// v1→v2: o default antigo da blocklist de captura vira o default novo; lista editada fica.
    #[test]
    fn an_untouched_capture_blocklist_gets_the_new_default_and_an_edited_one_is_kept() {
        let defaults = crate::config::schemas::registry().defaults();
        let new_default = defaults["vision"]["captureBlocklist"].clone();
        assert_ne!(new_default, json!(LEGACY_CAPTURE_BLOCKLIST), "o default novo tem de ser mais amplo que o antigo");

        for (saved, expected) in [(LEGACY_CAPTURE_BLOCKLIST.to_string(), new_default.clone()), ("meu banco, minha lista".to_string(), json!("meu banco, minha lista"))] {
            let dir = std::env::temp_dir().join(format!("mika-config-blocklist-migration-{}-{}", std::process::id(), saved.len()));
            std::fs::create_dir_all(&dir).unwrap();
            let body = json!({ "schemaVersion": 1, "vision": { "captureBlocklist": saved } });
            std::fs::write(dir.join("config.json"), body.to_string()).unwrap();
            let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
            assert_eq!(store.get("vision.captureBlocklist"), Some(expected));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn legacy_config_without_schema_version_is_migrated_keeping_user_values() {
        let dir = tempdir::Dir::new();
        fs::write(dir.path().join("config.json"), r#"{"memory":{"backend":"redis"}}"#).unwrap();

        let store = ConfigStore::open(dir.path(), schema()).unwrap();

        assert_eq!(store.get("memory.backend"), Some(json!("redis")));
        assert_eq!(store.get("schemaVersion"), Some(json!(CONFIG_SCHEMA_VERSION)));
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(dir.path().join("config.json")).unwrap()).unwrap();
        assert_eq!(on_disk["schemaVersion"], json!(CONFIG_SCHEMA_VERSION), "o carimbo tem de ir ao disco na migração");
        assert_eq!(on_disk["memory"]["backend"], json!("redis"));
    }

    /// Mudar um default só vale para PERFIL NOVO: quem já tem `config.json` carimbado na versão atual
    /// tem o valor gravado por cima do default, então um recurso que ele ligou (ou deixou ligado)
    /// continua como estava depois de uma atualização que desliga o default.
    #[test]
    fn a_changed_default_never_overrides_what_an_existing_config_already_stores() {
        let dir = tempdir::Dir::new();
        let body = format!(r#"{{"schemaVersion":{CONFIG_SCHEMA_VERSION},"browser":{{"enabled":true}},"computer-use":{{"enabled":true}}}}"#);
        fs::write(dir.path().join("config.json"), body).unwrap();

        let store = ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap();

        assert_eq!(store.get("browser.enabled"), Some(json!(true)));
        assert_eq!(store.get("computer-use.enabled"), Some(json!(true)));
        // Chave que ele nunca gravou nasce no default novo (desligado).
        assert_eq!(store.get("person-catalog.webEnrichment"), Some(json!(false)));

        let fresh = tempdir::Dir::new();
        let new_profile = ConfigStore::open(fresh.path(), crate::config::schemas::registry()).unwrap();
        assert_eq!(new_profile.get("browser.enabled"), Some(json!(false)));
        assert_eq!(new_profile.get("computer-use.enabled"), Some(json!(false)));
    }

    /// v2→v3: o default de `browser`/`computer-use` virou `false`, mas um config ANTIGO PARCIAL (sem a
    /// chave) tinha o recurso LIGADO pelo default de então. `open` regrava o snapshot, então sem a
    /// migração o valor efetivo se perdia para sempre. Perfil novo (diretório vazio) nasce desligado;
    /// o que não é "recurso que sai da máquina" (`legal.ageConfirmed`) não é preservado, e o
    /// enriquecimento web do catálogo NÃO é preservado (privacidade) — fica um aviso pendente.
    #[test]
    fn a_partial_legacy_config_keeps_the_features_it_had_on_by_default_and_a_new_profile_starts_off() {
        let dir = tempdir::Dir::new();
        fs::write(dir.path().join("config.json"), r#"{"schemaVersion":1,"app":{}}"#).unwrap();

        let store = ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap();

        assert_eq!(store.get("browser.enabled"), Some(json!(true)));
        assert_eq!(store.get("computer-use.enabled"), Some(json!(true)));
        assert_eq!(store.get("legal.ageConfirmed"), Some(json!(false)));
        assert_eq!(store.get("person-catalog.webEnrichment"), Some(json!(false)));
        assert_eq!(store.get("person-catalog.webEnrichmentNoticePending"), Some(json!(true)));
        // o valor preservado chegou ao DISCO: é a regravação do `open` que o perdia
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(dir.path().join("config.json")).unwrap()).unwrap();
        assert_eq!(on_disk["browser"]["enabled"], json!(true));

        let fresh = tempdir::Dir::new();
        let new_profile = ConfigStore::open(fresh.path(), crate::config::schemas::registry()).unwrap();
        assert_eq!(new_profile.get("browser.enabled"), Some(json!(false)));
        assert_eq!(new_profile.get("computer-use.enabled"), Some(json!(false)));
        assert_eq!(new_profile.get("person-catalog.webEnrichmentNoticePending"), Some(json!(false)));
    }

    /// Quem já escolheu `false` explicitamente continua com `false` (a migração só preenche ausência),
    /// e quem já gravou `webEnrichment` não ganha aviso nenhum.
    #[test]
    fn the_v3_migration_only_fills_absent_keys() {
        let dir = tempdir::Dir::new();
        let body = r#"{"schemaVersion":2,"browser":{"enabled":false},"person-catalog":{"webEnrichment":true}}"#;
        fs::write(dir.path().join("config.json"), body).unwrap();

        let store = ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap();

        assert_eq!(store.get("browser.enabled"), Some(json!(false)));
        assert_eq!(store.get("computer-use.enabled"), Some(json!(true)));
        assert_eq!(store.get("person-catalog.webEnrichment"), Some(json!(true)));
        assert_eq!(store.get("person-catalog.webEnrichmentNoticePending"), Some(json!(false)));
    }

    /// v3→v4: `en_US-lessac-high` (já foi opção da UI; treinada direto no dataset Lessac, só pesquisa)
    /// e `en_US-amy-medium` (licença do dataset não verificada) saíram do catálogo do Piper. Quem as
    /// tinha salvas voltaria a "voz fora do catálogo" a cada fala: perdem a chave e ganham o default
    /// (`pt_BR-faber-medium`); voz do catálogo e valor de outro campo ficam como estão.
    #[test]
    fn a_retired_piper_voice_falls_back_to_the_default_and_a_catalog_voice_is_kept() {
        for (saved, expected) in [
            ("en_US-lessac-high", "pt_BR-faber-medium"),
            ("en_US-amy-medium", "pt_BR-faber-medium"),
            ("de_DE-thorsten-medium", "de_DE-thorsten-medium"),
        ] {
            let dir = tempdir::Dir::new();
            let body = json!({ "schemaVersion": 3, "voice": { "piperVoice": saved } });
            fs::write(dir.path().join("config.json"), body.to_string()).unwrap();
            let store = ConfigStore::open(dir.path(), crate::config::schemas::registry()).unwrap();
            assert_eq!(store.get("voice.piperVoice"), Some(json!(expected)), "{saved}");
        }
    }

    /// Erro de IO (permissão, antivírus segurando o arquivo) NÃO é config corrompido: restaurar o
    /// `.bak` por cima trocaria um config íntegro pela versão anterior. Só erro de PARSE recupera.
    #[test]
    fn an_io_error_reading_the_config_never_falls_back_to_the_backup() {
        let dir = tempdir::Dir::new();
        // diretório no lugar do arquivo: `read_to_string` falha com erro de IO, determinístico
        fs::create_dir(dir.path().join("config.json")).unwrap();
        fs::write(dir.path().join("config.json.bak"), r#"{"memory":{"backend":"redis"}}"#).unwrap();

        let Err(error) = ConfigStore::open(dir.path(), schema()) else {
            panic!("erro de IO foi tratado como config corrompido e abriu pelo .bak");
        };

        assert!(error.contains("não leu"), "{error}");
        assert_eq!(fs::read_to_string(dir.path().join("config.json.bak")).unwrap(), r#"{"memory":{"backend":"redis"}}"#);
    }

    #[test]
    fn config_from_a_newer_schema_is_refused_without_touching_the_file() {
        let dir = tempdir::Dir::new();
        let body = r#"{"schemaVersion":99,"campoDoFuturo":true}"#;
        fs::write(dir.path().join("config.json"), body).unwrap();

        let err = ConfigStore::open(dir.path(), schema()).err().expect("deveria recusar");

        assert!(err.contains("v99") && err.contains("atualize"), "{err}");
        assert_eq!(fs::read_to_string(dir.path().join("config.json")).unwrap(), body);
    }

    #[test]
    fn refuses_prototype_pollution_paths() {
        let (store, _dir) = store();

        assert!(store.set("memory.__proto__", json!("x")).is_err());
        assert!(store.set("__proto__.polluted", json!(true)).is_err());
        assert!(store.set("memory.constructor", json!("x")).is_err());
    }

    /// O que o usuário configurou sobrevive ao reinício, e campo novo de uma versão nova nasce com
    /// o default em vez de zerar o namespace inteiro.
    #[test]
    fn persists_across_reopen_and_keeps_new_defaults() {
        let dir = tempdir::Dir::new();

        let first = ConfigStore::open(dir.path(), schema()).unwrap();
        first.set("memory.backend", json!("redis")).unwrap();
        drop(first);

        let second = ConfigStore::open(dir.path(), schema()).unwrap();
        assert_eq!(second.get("memory.backend"), Some(json!("redis")));
        assert_eq!(second.get("memory.episodicListLimit"), Some(json!(500)));
    }

    /// O wizard grava vários caminhos em paralelo (`Promise.all` do lado renderer, ex.:
    /// `savePrivacyChoices`/`saveLlmSelection`). ANTES desta correção, `set` soltava o lock do
    /// `cache` antes de escrever em disco, então duas chamadas concorrentes gravavam o MESMO
    /// `config.json.tmp` ao mesmo tempo — no Windows isso falha com "os error 32" (sharing
    /// violation), `save` devolvia `Err`, e o handler de IPC respondia `success: false` mesmo com
    /// valores válidos. O sintoma no E2E era "Falha ao salvar as configurações iniciais" travando o
    /// wizard num passo aleatório. Repro determinística sem depender de timing de disco real:
    /// dispara N `set`s concorrentes contra o MESMO store e exige que todos tenham sucesso e que o
    /// estado final contenha os N valores.
    #[test]
    fn concurrent_sets_all_succeed_and_land_on_disk() {
        let (store, dir) = store();

        for _round in 0..5 {
            let paths_and_values: [(&str, Value); 8] = [
                ("llm.provider", json!("ollama")),
                ("voice.groqApiKey", json!("g-key")),
                ("apiKeys.openai", json!("sk-1")),
                ("apiKeys.openaiCompatibleBaseUrl", json!("http://localhost:1234")),
                ("llm.provider", json!("ollama")),
                ("voice.groqApiKey", json!("g-key")),
                ("apiKeys.openai", json!("sk-1")),
                ("apiKeys.openaiCompatibleBaseUrl", json!("http://localhost:1234")),
            ];

            let barrier = std::sync::Barrier::new(paths_and_values.len());
            std::thread::scope(|scope| {
                let handles: Vec<_> = paths_and_values
                    .iter()
                    .map(|(path, value)| {
                        let barrier = &barrier;
                        let store = &store;
                        scope.spawn(move || {
                            barrier.wait();
                            store.set(path, value.clone())
                        })
                    })
                    .collect();

                for handle in handles {
                    assert!(handle.join().unwrap().is_ok(), "gravação concorrente falhou");
                }
            });
        }

        assert_eq!(store.get("llm.provider"), Some(json!("ollama")));
        assert_eq!(store.get("voice.groqApiKey"), Some(json!("g-key")));
        assert_eq!(store.get("apiKeys.openai"), Some(json!("sk-1")));
        assert_eq!(
            store.get("apiKeys.openaiCompatibleBaseUrl"),
            Some(json!("http://localhost:1234"))
        );

        // Reabrir prova que o último `save` em disco carregava os quatro valores, não só o do
        // thread que "ganhou" a corrida.
        drop(store);
        let reopened = ConfigStore::open(dir.path(), schema()).unwrap();
        assert_eq!(reopened.get("llm.provider"), Some(json!("ollama")));
        assert_eq!(reopened.get("voice.groqApiKey"), Some(json!("g-key")));
    }

    /// Um config truncado ou corrompido não pode virar "começa tudo do zero" em silêncio — o
    /// usuário perderia a configuração inteira sem uma linha de aviso.
    #[test]
    fn corrupt_config_is_an_error_not_a_silent_reset() {
        let dir = tempdir::Dir::new();
        fs::write(dir.path().join("config.json"), "{ isto não é json").unwrap();

        // `unwrap_err` exigiria `Debug` em `ConfigStore`, que carrega um `Mutex` e não ganha
        // derive só por causa de um teste.
        let Err(error) = ConfigStore::open(dir.path(), schema()) else {
            panic!("config corrompido abriu como se estivesse bom — o usuário perderia tudo em silêncio");
        };
        assert!(error.contains("inválido"), "erro pouco claro: {error}");
    }

    /// Config truncado com um `.bak` bom: o app sobe com a ÚLTIMA versão boa em vez de recusar abrir
    /// (ou, pior, resetar). O `.bak` é a cópia feita antes do último `save`.
    #[test]
    fn a_truncated_config_recovers_from_the_backup() {
        let dir = tempdir::Dir::new();
        let first = ConfigStore::open(dir.path(), schema()).unwrap();
        first.set("memory.backend", json!("redis")).unwrap();
        first.set("llm.provider", json!("google")).unwrap();
        drop(first);
        let config = dir.path().join("config.json");
        let body = fs::read_to_string(&config).unwrap();
        fs::write(&config, &body[..body.len() / 2]).unwrap();

        let recovered = ConfigStore::open(dir.path(), schema()).expect("deveria recuperar do .bak");

        assert_eq!(recovered.get("memory.backend"), Some(json!("redis")), "o .bak guarda o estado antes do último set");
        assert!(serde_json::from_str::<Value>(&fs::read_to_string(&config).unwrap()).is_ok(), "config.json restaurado");
    }

    /// `save` que falha não pode deixar a memória divergente do disco: o `get` seguinte mentiria sobre
    /// um valor que o próximo boot não vê, e o observador nem é avisado.
    #[test]
    fn a_failed_save_leaves_the_cache_untouched_and_notifies_nobody() {
        use std::sync::mpsc;

        let (store, dir) = store();
        store.set("memory.backend", json!("redis")).unwrap();
        let (tx, rx) = mpsc::channel();
        store.watch(Box::new(move |path, _| {
            let _ = tx.send(path.to_string());
        }));
        // diretório no lugar do temporário: `File::create` falha de forma determinística
        fs::create_dir(dir.path().join("config.json.tmp")).unwrap();

        assert!(store.set("memory.backend", json!("embedded")).is_err());
        assert!(store.set("voice.groqApiKey", json!("gsk-novo")).is_err());

        assert_eq!(store.get("memory.backend"), Some(json!("redis")));
        assert_eq!(store.get("voice.groqApiKey"), Some(json!("")), "o default, não a chave que não foi gravada");
        assert!(rx.try_recv().is_err(), "observador avisado de escrita que falhou");
    }

    #[test]
    fn unknown_path_is_refused() {
        let (store, _dir) = store();
        assert!(store.set("memory.naoExiste", json!(1)).is_err());
    }

    /// O observador existe para ser CHAMADO. Sem este teste ele é indistinguível de um `watch` que
    /// guarda a closure e nunca a invoca — que é exatamente o estado em que `mcp` ficou enquanto
    /// `on_mcp_enabled_changed` estava escrita, testada e sem chamador nenhum.
    #[test]
    fn a_registered_watcher_sees_the_path_and_the_value_that_was_written() {
        use std::sync::mpsc;

        let (store, _dir) = store();
        let (tx, rx) = mpsc::channel();
        store.watch(Box::new(move |path, value| {
            let _ = tx.send((path.to_string(), value.clone()));
        }));

        store.set("memory.enabled", json!(false)).unwrap();

        let (path, value) = rx.try_recv().expect("observador não foi chamado");
        assert_eq!(path, "memory.enabled");
        assert_eq!(value, json!(false));
    }

    /// Escrita RECUSADA não notifica. Um observador que reagisse a um `set` inválido agiria sobre um
    /// valor que o próximo boot não veria — MCP derrubaria conexão viva por uma mudança que nunca
    /// chegou ao disco.
    #[test]
    fn a_refused_write_notifies_nobody() {
        use std::sync::mpsc;

        let (store, _dir) = store();
        let (tx, rx) = mpsc::channel();
        store.watch(Box::new(move |path, _| {
            let _ = tx.send(path.to_string());
        }));

        assert!(store.set("memory.naoExiste", json!(1)).is_err());
        assert!(rx.try_recv().is_err(), "observador foi chamado para uma escrita recusada");
    }

    /// Credencial NÃO vaza para o observador. `watch` entrega o valor mascarado pela mesma razão que
    /// `get_all` mascara: observador é código arbitrário de módulo, e a chave em claro só deve sair
    /// por um `get` no caminho específico.
    #[test]
    fn a_watcher_never_sees_a_credential_in_the_clear() {
        use std::sync::mpsc;

        let (store, _dir) = store();
        let (tx, rx) = mpsc::channel();
        store.watch(Box::new(move |_, value| {
            let _ = tx.send(value.clone());
        }));

        store.set("apiKeys.openai", json!("sk-segredo-do-usuario")).unwrap();

        let seen = rx.try_recv().expect("observador não foi chamado");
        let seen = seen.as_str().unwrap_or_default();
        assert!(!seen.contains("sk-segredo-do-usuario"), "a chave em claro chegou ao observador: {seen}");
    }

    /// Diretório temporário mínimo. Não vale uma dependência só para os testes do store.
    mod tempdir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        pub struct Dir(PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "mika-config-test-{}-{unique}",
                    std::process::id()
                ));
                std::fs::create_dir_all(&path).unwrap();
                Self(path)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
