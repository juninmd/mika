use super::*;
use crate::config::registry::{Namespace, SchemaRegistry};
use crate::config::schema::boolean;
use crate::modules::voice::tts::abort_registry::AbortRegistry;

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-piper-test-{tag}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store(offline: bool) -> (ConfigStore, TempDir) {
    let dir = TempDir::new("store");
    let schema = SchemaRegistry::new(vec![Namespace { name: "privacy", fields: vec![boolean("offlineMode", offline)] }]);
    (ConfigStore::open(&dir.0, schema).unwrap(), dir)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn cancel() -> CancelHandle {
    let registry = Box::leak(Box::new(AbortRegistry::new()));
    registry.register_active().handle()
}

// --- parse_voice_key -----------------------------------------------------------------------

#[test]
fn parse_voice_key_splits_locale_speaker_and_quality() {
    let voice = parse_voice_key("pt_BR-faber-medium").expect("chave válida");
    assert_eq!(voice.key, "pt_BR-faber-medium");
    assert_eq!(voice.path, "pt/pt_BR/faber/medium/pt_BR-faber-medium.onnx");
}

#[test]
fn parse_voice_key_joins_a_hyphenated_speaker_name() {
    let voice = parse_voice_key("en_US-jeff-x-low").expect("chave válida");
    assert_eq!(voice.path, "en/en_US/jeff-x/low/en_US-jeff-x-low.onnx");
}

#[test]
fn parse_voice_key_rejects_malformed_keys() {
    assert!(parse_voice_key("").is_none());
    assert!(parse_voice_key("apenas-dois").is_none());
    assert!(parse_voice_key("PT_BR-faber-medium").is_none(), "idioma não é minúsculo");
}

/// A chave vira nome de arquivo no cache e caminho na URL: separador ou `..` escaparia do cache.
#[test]
fn parse_voice_key_rejects_path_characters() {
    assert!(parse_voice_key("pt_BR-../../../x-medium").is_none());
    assert!(parse_voice_key("pt_BR-a/b-medium").is_none());
    assert!(parse_voice_key("pt_BR-a\\b-medium").is_none());
    assert!(parse_voice_key("pt_BR-faber-medium?x=1").is_none());
}

#[tokio::test]
async fn a_download_is_written_under_a_part_name_and_only_renamed_when_complete() {
    let dir = TempDir::new("commit");
    let target = dir.0.join("v.onnx");

    commit_download(&target, b"model", &sha256_hex(b"model"), "pt-BR").await.unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"model");
    assert!(!part_path(&target).exists(), ".part sobrou depois do rename");
    let part = part_path(&target);
    assert!(part.file_name().unwrap().to_string_lossy().ends_with(".part") && part.parent() == target.parent(), "{part:?}");
    assert_ne!(part_path(&target), part, "cada download tem o seu .part");
}

/// B5: `ensure_voice` concorrentes (a fala e o aquecimento, dois turnos) escreviam no MESMO `.part`:
/// um `rename` levava o arquivo do outro e o perdedor voltava erro. Cada download tem o seu `.part`.
#[tokio::test]
async fn concurrent_downloads_of_the_same_voice_file_all_succeed() {
    let dir = TempDir::new("commit-concurrent");
    let target = dir.0.join("v.onnx");
    let digest = sha256_hex(&vec![7u8; 4 * 1024 * 1024]);
    let tasks: Vec<_> = (0..16)
        .map(|_| {
            let (target, digest) = (target.clone(), digest.clone());
            tokio::spawn(async move { commit_download(&target, &vec![7u8; 4 * 1024 * 1024], &digest, "pt-BR").await })
        })
        .collect();
    for task in tasks {
        task.await.unwrap().expect("download concorrente falhou");
    }
    assert_eq!(std::fs::read(&target).unwrap().len(), 4 * 1024 * 1024);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1, "sobrou .part");
}

/// Bytes que não são os medidos no catálogo (CDN comprometida, ref trocada, corpo truncado) nunca
/// viram cache: o rename só acontece depois do SHA-256 bater.
#[tokio::test]
async fn a_download_with_the_wrong_sha256_is_discarded_and_leaves_nothing_on_disk() {
    let dir = TempDir::new("commit-bad-digest");
    let target = dir.0.join("v.onnx");

    let err = commit_download(&target, b"tampered", &sha256_hex(b"model"), "en-US").await.unwrap_err();

    assert!(err.contains("integrity"), "erro não nomeia a verificação: {err}");
    assert!(!target.exists(), "arquivo adulterado virou cache");
    assert!(!part_path(&target).exists(), ".part sobrou");
}

#[test]
fn every_catalog_voice_has_a_parsable_key_matching_its_language_and_full_digests() {
    for voice in catalog::VOICES {
        let parsed = parse_voice_key(voice.key).unwrap_or_else(|| panic!("{} não parseia", voice.key));
        assert!(parsed.path.starts_with(&format!("{}/", voice.language)), "{}: pasta != idioma", voice.key);
        for digest in [voice.onnx_sha256, voice.json_sha256] {
            assert!(digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()), "{}: digest inválido", voice.key);
        }
    }
}

/// Idioma com voz de licença comercial verificada tem fallback; ja/ko/zh não (Piper não tem voz em
/// ja/ko, e a Huayan zh_CN não declara licença) — nada de voz inventada ou de outro idioma.
#[test]
fn language_fallback_exists_only_where_a_verified_voice_exists() {
    for (language, key) in [("de", "de_DE-thorsten-medium"), ("it", "it_IT-paola-medium"), ("ru", "ru_RU-denis-medium")] {
        assert_eq!(catalog::voice_for_language(language).map(|v| v.key), Some(key));
    }
    // `en`: o Supertonic fala inglês, então não há fallback — e a única voz inglesa que havia
    // (`amy`) tinha licença do dataset NÃO verificada.
    for language in ["ja", "ko", "zh", "en"] {
        assert!(catalog::voice_for_language(language).is_none(), "{language} não devia ter voz Piper");
    }
    assert!(catalog::voice("en_US-amy-medium").is_none() && catalog::voice("en_US-lessac-high").is_none());
}

#[test]
fn synthesis_output_paths_are_unique_within_the_process() {
    assert_ne!(synthesis_output_path(), synthesis_output_path());
}

// --- rate_to_length_scale --------------------------------------------------------------------

#[test]
fn rate_to_length_scale_inverts_and_clamps() {
    assert_eq!(rate_to_length_scale(1.0), 1.0);
    assert_eq!(rate_to_length_scale(2.0), 0.5);
    assert_eq!(rate_to_length_scale(0.1), 2.0, "extremo baixo tem de ser clampado, não virar áudio ininteligível");
    assert_eq!(rate_to_length_scale(0.0), 1.0);
    assert_eq!(rate_to_length_scale(f32::NAN), 1.0);
}

// --- build_args ------------------------------------------------------------------------------

#[test]
fn build_args_omits_length_scale_at_the_default_rate() {
    let args = build_args(Path::new("/voices/v.onnx"), Path::new("/tmp/out.wav"), 1.0);
    assert_eq!(args, vec!["--model", "/voices/v.onnx", "--output_file", "/tmp/out.wav"]);
}

#[test]
fn build_args_includes_length_scale_when_rate_is_not_default() {
    let args = build_args(Path::new("/voices/v.onnx"), Path::new("/tmp/out.wav"), 0.5);
    assert_eq!(args, vec!["--model", "/voices/v.onnx", "--output_file", "/tmp/out.wav", "--length_scale", "0.5"]);
}

// --- resolve_binary --------------------------------------------------------------------------

#[test]
fn resolve_binary_fails_named_when_the_configured_path_does_not_exist() {
    let err = resolve_binary("/does/not/exist/piper", "pt-BR").unwrap_err();
    assert!(err.contains("caminho configurado não existe"), "erro não nomeia a causa: {err}");
}

#[test]
fn resolve_binary_accepts_a_configured_path_that_exists_without_executing_it() {
    let dir = TempDir::new("binary");
    let fake = dir.0.join("not-really-an-executable");
    std::fs::write(&fake, b"#!/bin/sh\necho fake\n").unwrap();
    // `existsSync`, não execução: um arquivo qualquer no caminho configurado já resolve, igual o TS.
    let resolved = resolve_binary(fake.to_str().unwrap(), "pt-BR").unwrap();
    assert_eq!(resolved, fake.to_str().unwrap());
}

#[test]
fn resolve_binary_fails_named_when_nothing_is_configured_and_the_path_has_no_piper() {
    // Ambiente de CI/dev não tem `piper`/`piper.exe` no PATH — se algum dia tiver, este teste
    // passa a provar o caminho de sucesso em vez do de erro (nenhum dos dois é falso positivo).
    let err = resolve_binary("", "pt-BR");
    if let Err(message) = err {
        assert!(message.to_lowercase().contains("piper não encontrado"), "erro não nomeia a causa: {message}");
    }
}

// --- ensure_voice ----------------------------------------------------------------------------

#[tokio::test]
async fn ensure_voice_fails_named_for_a_malformed_key_without_touching_the_network() {
    let (store, _dir) = store(false);
    let cache = TempDir::new("cache");
    let err = ensure_voice(&cache.0, "chave-invalida", &store).await.unwrap_err();
    assert!(err.contains("chave de voz inválida"), "erro não nomeia a causa: {err}");
}

/// Chave bem-formada mas fora do catálogo não baixa nada: sem SHA medido não há como verificar.
#[tokio::test]
async fn ensure_voice_refuses_a_well_formed_key_that_is_not_in_the_catalog() {
    let (store, _dir) = store(false);
    let cache = TempDir::new("cache-uncataloged");
    let err = ensure_voice(&cache.0, "pt_BR-ghost-medium", &store).await.unwrap_err();
    assert!(err.contains("pt_BR-ghost-medium"), "erro não nomeia a voz: {err}");
    assert!(std::fs::read_dir(&cache.0).unwrap().next().is_none(), "criou arquivo para voz fora do catálogo");
}

#[tokio::test]
async fn ensure_voice_is_blocked_by_offline_mode_before_any_download() {
    let (store, _dir) = store(true);
    let cache = TempDir::new("cache-offline");
    let err = ensure_voice(&cache.0, "pt_BR-faber-medium", &store).await.unwrap_err();
    assert!(err.to_lowercase().contains("offline"), "não bloqueou por modo offline: {err}");
}

#[tokio::test]
async fn ensure_voice_reuses_an_already_cached_model_without_touching_the_network() {
    let (store, _dir) = store(true); // offline: se tentasse rede, o teste falharia pelo guard
    let cache = TempDir::new("cache-hit");
    let key = "pt_BR-faber-medium";
    std::fs::write(cache.0.join(format!("{key}.onnx")), b"fake-model").unwrap();
    std::fs::write(cache.0.join(format!("{key}.onnx.json")), b"{}").unwrap();

    let path = ensure_voice(&cache.0, key, &store).await.unwrap();
    assert_eq!(path, cache.0.join(format!("{key}.onnx")));
}

/// Voz que o usuário JÁ baixou e que depois saiu do catálogo (`en_US-lessac-high` estava na UI) é
/// usada do cache: o catálogo só decide o que BAIXA, não o que já está no disco. Antes, o catálogo
/// era consultado primeiro e a voz baixada virava `voiceNotInCatalog`.
#[tokio::test]
async fn a_cached_voice_that_left_the_catalog_is_still_used() {
    let (store, _dir) = store(true);
    let cache = TempDir::new("cache-retired");
    let key = "en_US-lessac-high";
    std::fs::write(cache.0.join(format!("{key}.onnx")), b"fake-model").unwrap();
    std::fs::write(cache.0.join(format!("{key}.onnx.json")), b"{}").unwrap();

    assert_eq!(ensure_voice(&cache.0, key, &store).await.unwrap(), cache.0.join(format!("{key}.onnx")));
}

// --- generate --------------------------------------------------------------------------------

#[tokio::test]
async fn generate_fails_named_when_the_binary_is_missing_before_touching_the_network() {
    let (store, _dir) = store(false);
    let cache = TempDir::new("generate-no-binary");
    let opts = PiperOptions {
        voice: "pt_BR-faber-medium".into(),
        binary_path: "/does/not/exist/piper".into(),
        rate: 1.0,
        cache_dir: cache.0.clone(),
    };
    let err = generate("oi", &opts, &store, &cancel()).await.unwrap_err();
    assert!(err.contains("caminho configurado não existe"), "erro não nomeia a causa: {err}");
    assert!(!cache.0.join("pt_BR-faber-medium.onnx").exists(), "não deveria ter baixado nada");
}

#[tokio::test]
async fn generate_rejects_empty_text_before_resolving_the_binary() {
    let (store, _dir) = store(false);
    let cache = TempDir::new("generate-empty-text");
    let opts = PiperOptions { voice: "pt_BR-faber-medium".into(), binary_path: String::new(), rate: 1.0, cache_dir: cache.0.clone() };
    let err = generate("   ", &opts, &store, &cancel()).await.unwrap_err();
    assert!(err.contains("texto vazio"));
}

/// Cancelar a síntese mata o Piper, mas sem `wait()` o processo morto ficava zumbi (um por barge-in)
/// até o app fechar. `sleep` faz o papel do binário lento; procura-se um filho zumbi nosso no /proc.
#[cfg(target_os = "linux")]
#[test]
fn a_cancelled_synthesis_reaps_the_killed_child() {
    fn zombie_sleeps() -> usize {
        let me = std::process::id().to_string();
        std::fs::read_dir("/proc")
            .unwrap()
            .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path().join("stat")).ok())
            .filter(|stat| {
                // `pid (comm) estado ppid ...` — o comm pode ter espaços, então corta no último `)`.
                let tail = stat.rsplit_once(") ").map_or("", |(_, tail)| tail);
                let mut fields = tail.split(' ');
                stat.contains("(sleep)") && fields.next() == Some("Z") && fields.next() == Some(me.as_str())
            })
            .count()
    }
    let handle = cancel();
    handle.cancel();
    let err = run_synthesis("sleep", &["30".to_string()], "oi", &handle, "pt-BR").unwrap_err();
    assert!(err.contains("cancelled"), "{err}");
    assert_eq!(zombie_sleeps(), 0, "o filho morto não foi colhido");
}
