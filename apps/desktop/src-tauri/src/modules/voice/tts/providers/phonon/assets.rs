//! Download/cache dos checkpoints Phonon, sob demanda, em `<cache>/phonon/<modelo>/`.
//!
//! **Nada é embarcado.** O instalador não carrega peso nenhum: os quatro arquivos vêm do Hugging
//! Face na primeira síntese, na revisão fixa do [`catalog`] e com a integridade conferida no
//! streaming (SHA-256 para LFS, SHA-1 do blob git para os JSON pequenos). O padrão de arquivo
//! `.part` único + `rename` atômico é o mesmo de `providers/supertonic/assets.rs` e
//! `providers/piper.rs`: download interrompido nunca deixa um cache que parece pronto.
//!
//! **Gate do Hugging Face.** Os modelos 40m são CC BY-NC e vivem atrás do gate do repositório.
//! O token fica em `apiKeys.huggingFace` (namespace de credenciais do store, cifrado como qualquer
//! chave) e vai como `Authorization: Bearer` só quando existe. 401/403 viram erro NOMEADO que manda
//! aceitar os termos na página do modelo e informar um token — nunca silêncio, nunca áudio falso.

use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::config::locale::app_locale;
use crate::config::store::ConfigStore;
use crate::modules::privacy::assert_network_allowed;
use crate::modules::settings::tf;

use super::catalog::{self, FileSpec, Integrity, PhononModel};

const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const CACHE_SUBDIR: &str = "phonon";
/// Caminho da credencial no store. `apiKeys.` é o prefixo cifrado (ver `config/store.rs`).
pub const HF_TOKEN_PATH: &str = "apiKeys.huggingFace";

fn msg(locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
    tf(key, locale, vars)
}

/// Pasta do checkpoint. `cache_dir` chega como `<app_data>/supertonic` (mesmo argumento que
/// Supertonic/Piper recebem de `speak_tool`); a subpasta própria evita dividir diretório com outro
/// provedor.
pub fn model_dir(cache_dir: &Path, model: &PhononModel) -> PathBuf {
    cache_dir.join(CACHE_SUBDIR).join(model.key)
}

/// Token do Hugging Face — só o usuário o fornece; vazio significa "sem token" (modelo público
/// segue, modelo com gate falha no 401 com instrução).
pub fn hf_token(store: &ConfigStore) -> String {
    store.get(HF_TOKEN_PATH).and_then(|value| value.as_str().map(|text| text.trim().to_string())).unwrap_or_default()
}

/// SHA-1 do blob git: `sha1("blob {len}\0" + bytes)`. É o `blobId` da API do Hugging Face para
/// arquivos que não são LFS, e a única impressão verificável de um repo com gate.
pub fn git_blob_sha1(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Só o que bate com a integridade medida no catálogo. O download usa o caminho de streaming em
/// [`download_to_file`]; este helper fica para o contrato dos testes (vetor de blob git conhecido).
#[cfg(test)]
pub fn bytes_match(bytes: &[u8], integrity: Integrity) -> bool {
    match integrity {
        Integrity::Sha256(expected) => hex::encode(Sha256::digest(bytes)) == expected,
        Integrity::GitBlobSha1(expected) => git_blob_sha1(bytes) == expected,
    }
}

/// Se o modelo já está completo no disco. NUNCA baixa (usado por status / pré-carregamento).
pub fn installed(cache_dir: &Path, model: &PhononModel) -> bool {
    let dir = model_dir(cache_dir, model);
    model.files.iter().all(|file| dir.join(file.name).is_file())
}

fn part_path(target: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{}.part", std::process::id(), COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    target.with_file_name(name)
}

/// Baixa (se ausente) os quatro arquivos e devolve a pasta do checkpoint.
pub async fn ensure_model(model: &PhononModel, cache_dir: &Path, store: &ConfigStore) -> Result<PathBuf, String> {
    let locale = app_locale(store);
    let dir = model_dir(cache_dir, model);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| msg(&locale, "voice.tts.phonon.writeFailed", &[("path", &dir.display().to_string()), ("detail", &e.to_string())]))?;
    if installed(cache_dir, model) {
        return Ok(dir);
    }
    log::info!("Phonon TTS: preparando {} ({})", model.key, model.license.label());
    let token = hf_token(store);
    for file in &model.files {
        let target = dir.join(file.name);
        if !target.is_file() {
            download_to_file(model, file, &token, &target, store, &locale).await?;
        }
    }
    Ok(dir)
}

fn download_error(locale: &str, model: &PhononModel, file: &FileSpec, detail: &str) -> String {
    msg(locale, "voice.tts.phonon.downloadFailed", &[("model", model.key), ("file", file.name), ("detail", detail)])
}

/// 401/403 num modelo com gate: o usuário precisa aceitar os termos e informar um token. A mensagem
/// nomeia os dois passos e a página — nada de "download falhou: HTTP 401".
fn gated_error(locale: &str, model: &PhononModel) -> String {
    msg(locale, "voice.tts.phonon.gated", &[("model", model.key), ("url", &catalog::page(model))])
}

async fn download_to_file(
    model: &PhononModel,
    file: &FileSpec,
    token: &str,
    target: &Path,
    store: &ConfigStore,
    locale: &str,
) -> Result<(), String> {
    let url = catalog::url(model, file.name);
    assert_network_allowed(store, &url, "phonon-tts")?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| msg(locale, "voice.tts.phonon.writeFailed", &[("path", &parent.display().to_string()), ("detail", &e.to_string())]))?;
    }
    let part = part_path(target);
    match stream_and_verify(model, file, token, &url, &part, locale).await {
        Ok(()) => tokio::fs::rename(&part, target)
            .await
            .map_err(|e| msg(locale, "voice.tts.phonon.writeFailed", &[("path", &part.display().to_string()), ("detail", &e.to_string())])),
        Err(err) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(err)
        }
    }
}

async fn stream_and_verify(
    model: &PhononModel,
    file: &FileSpec,
    token: &str,
    url: &str,
    part: &Path,
    locale: &str,
) -> Result<(), String> {
    let mut request = crate::modules::privacy::fixed_endpoint_client().get(url).timeout(DOWNLOAD_TIMEOUT);
    if !token.is_empty() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|e| download_error(locale, model, file, &e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        if model.gated && matches!(status.as_u16(), 401 | 403) {
            return Err(gated_error(locale, model));
        }
        return Err(download_error(locale, model, file, &format!("HTTP {}", status.as_u16())));
    }
    let streamed = write_stream(response, part, file.integrity, locale).await.map_err(|e| download_error(locale, model, file, &e))?;
    if streamed != file.integrity.digest() {
        return Err(msg(locale, "voice.tts.phonon.digestMismatch", &[("file", file.name)]));
    }
    Ok(())
}

/// Escreve o corpo em `part`, calculando SHA-256 e, quando a integridade é blob git, lendo de volta
/// (arquivos pequenos) para o SHA-1 que depende do TAMANHO no prefixo. Devolve o digest calculado.
async fn write_stream(response: reqwest::Response, part: &Path, integrity: Integrity, locale: &str) -> Result<String, String> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| msg(locale, "voice.tts.phonon.writeFailed", &[("path", &part.display().to_string()), ("detail", &e.to_string())]))?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| msg(locale, "voice.tts.phonon.writeFailed", &[("path", &part.display().to_string()), ("detail", &e.to_string())]))?;
    }
    file.flush()
        .await
        .map_err(|e| msg(locale, "voice.tts.phonon.writeFailed", &[("path", &part.display().to_string()), ("detail", &e.to_string())]))?;
    drop(file);
    match integrity {
        Integrity::Sha256(_) => Ok(hex::encode(hasher.finalize())),
        // O prefixo do blob git leva o TAMANHO, então o SHA-1 só fecha depois do arquivo completo.
        Integrity::GitBlobSha1(_) => {
            let bytes = tokio::fs::read(part)
                .await
                .map_err(|e| msg(locale, "voice.tts.phonon.writeFailed", &[("path", &part.display().to_string()), ("detail", &e.to_string())]))?;
            Ok(git_blob_sha1(&bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vetor conhecido do git: `git hash-object` de `"hello\n"` é este blob. Sem um valor externo
    /// medido, um SHA-1 "quase certo" passaria nos próprios testes.
    #[test]
    fn git_blob_sha1_matches_the_known_hash_object_vector() {
        assert_eq!(git_blob_sha1(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a");
        assert_eq!(git_blob_sha1(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn integrity_rejects_bytes_that_are_not_the_measured_ones() {
        let sha = Integrity::Sha256("e1ab3d6a1a6835ab73ac1fc1b326c9a8238292c101d2050296413ab30dba7681");
        assert!(!bytes_match(b"nao-e-o-modelo", sha));
        let blob = Integrity::GitBlobSha1("b45c9e49061f36c362e69aaf94fbd3be8bc5e63c");
        assert!(!bytes_match(b"{}", blob));
        assert!(bytes_match(b"hello\n", Integrity::GitBlobSha1("ce013625030ba8dba906f756967f9e9ca394464a")));
    }

    #[test]
    fn installed_needs_every_file_and_never_downloads() {
        let dir = std::env::temp_dir().join(format!("mika-phonon-installed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = catalog::model(catalog::DEFAULT_MODEL).unwrap();
        assert!(!installed(&dir, model));
        for file in &model.files[..model.files.len() - 1] {
            let path = model_dir(&dir, model).join(file.name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
        }
        assert!(!installed(&dir, model), "falta a voz: nada para carregar");
        std::fs::write(model_dir(&dir, model).join(model.files[3].name), b"x").unwrap();
        assert!(installed(&dir, model));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gated_error_names_the_model_page_code_and_next_step() {
        let model = catalog::model("phonon-40m-pt").unwrap();
        let text = gated_error("pt-BR", model);
        assert!(text.contains("phonon-40m-pt"), "{text}");
        assert!(text.contains("https://huggingface.co/gradium/phonon-40m-pt"), "{text}");
        assert!(text.to_lowercase().contains("token"), "{text}");
    }
}
