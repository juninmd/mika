//! Porte de `apps/desktop/src/main/services/voice/tts/tts-provider-dispatch.ts`. Supertonic, Piper
//! e os 4 provedores remotos (ElevenLabs, OpenAI, Azure, Google) têm gerador real; Kokoro e
//! `system` são casos especiais, cada um por um motivo verificado, não por falta de tempo:
//!
//! | Provedor | Tratamento | Por quê |
//! |---|---|---|
//! | Kokoro | CORTE NOMEADO — `Err` | fonemização depende de `espeak-ng` (pacote npm `espeak-ng`, WASM via Emscripten). `Cargo.toml` já documenta a decisão: `espeakng` (a única rota nativa) passa por `bindgen`/`clang-sys` e quebra o build da árvore inteira sem LLVM no ambiente — "quem for portar o TTS local decide entre exigir LLVM ou outro caminho" (comentário no `[dependencies]`). Nenhuma outra rota Rust verificada para IPA fiel ao espeak-ng nesta rodada. |
//! | `system` | sucesso silencioso — `Ok(None)` | porte fiel do TS (`factory.ts`: `// 'system' or unknown: no runtime audio generation`, devolve `null`). Não é um corte: é a escolha do usuário de não ter voz nenhuma (`TTSProviderPanel.tsx`: "Sistema — Não gera áudio"). `Err` aqui faria `speak_chunk` (`speaker/chunk.rs`) logar um erro por FRASE para um usuário que pediu exatamente isto. |
//!
//! Selecionar Kokoro devolve `Err` NOMEADO — nunca silêncio nem áudio inventado (regra dura do
//! repositório, "zero mock em produção"). Selecionar `system` devolve `Ok(None)` — nunca um erro
//! disfarçado de corte.
//!
//! **ElevenLabs com o fallback automático para Supertonic.** Porte de `generateElevenLabs`/
//! `fallbackToLocal` (`tts-provider-dispatch.ts`) + `processSingleChunk` (`tts-speaker.ts`): sem
//! `apiKey`/`voiceId`, ou quando a chamada falha por credencial/cota/voz-fora-do-plano
//! (`elevenlabs.rs::classify`), a síntese cai para o Supertonic e publica
//! `EventKey::TtsFallback` — mesmo payload e mesma string de motivo do TS
//! (`reason: 'Missing credentials for selected provider'`, único valor que o TS emitia,
//! independente da causa exata). Falha de TRANSPORTE (rede fora, timeout) NÃO cai: propaga como
//! `Err` normal, porque o TS só troca de provedor numa resposta HTTP com corpo, nunca numa exceção
//! de fetch.

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};
use crate::modules::voice::tts::abort_registry::CancelHandle;
use crate::modules::voice::tts::providers::piper::{self, PiperOptions};
use crate::modules::voice::tts::providers::remote::{generate_azure, generate_elevenlabs, generate_google, generate_openai};
use crate::modules::voice::tts::providers::supertonic::{self, SupertonicOptions};
use crate::modules::voice::tts::state::TtsState;

pub struct GeneratedAudio {
    pub audio: Vec<u8>,
    pub mime_type: &'static str,
}

/// Mão em vez de `derive`: um `assert`/`expect` que falha nunca deve despejar o buffer de áudio
/// inteiro na mensagem — só os metadados importam para diagnosticar.
impl std::fmt::Debug for GeneratedAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeneratedAudio").field("audio_bytes", &self.audio.len()).field("mime_type", &self.mime_type).finish()
    }
}

const CUT_PROVIDERS: &[&str] = &["kokoro"];

/// Mime-type dos 4 provedores remotos: todos devolvem mp3 hoje (`output_format`/`response_format`/
/// `audioEncoding` de cada request), igual ao fallback `'audio/mpeg'` que `tts-speaker.ts` aplica
/// quando `TTSResult.mimeType` vem `undefined` — nenhum dos 4 preenche esse campo no TS.
const REMOTE_MIME_TYPE: &str = "audio/mpeg";

fn supertonic_options(state: &TtsState, cache_dir: &std::path::Path) -> SupertonicOptions {
    SupertonicOptions {
        voice: state.supertonic_voice.clone(),
        language: state.supertonic_language.clone(),
        speed: state.effective_speed(None),
        steps: state.supertonic_steps,
        cache_dir: cache_dir.to_path_buf(),
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// As três strings que `elevenlabs.rs::classify` embute na mensagem de erro — a única classificação
/// disponível deste lado (a chamada HTTP não devolve um enum estruturado, só `Result<_, String>`).
/// Falha de TRANSPORTE (timeout, DNS, conexão recusada) nunca produz nenhuma delas: essas só
/// aparecem depois de uma resposta HTTP de verdade, exatamente a distinção que o TS faz entre
/// `buffer === null` (cai) e uma exceção de `fetch` (propaga).
fn is_elevenlabs_permanent_failure(error: &str) -> bool {
    ["sem crédito na conta", "credencial inválida", "voz não coberta pelo plano"].iter().any(|needle| error.contains(needle))
}

/// Fala local para o idioma efetivo: Supertonic quando o modelo o pronuncia; senão a voz Piper
/// verificada do idioma (`piper::catalog`, só com o binário do Piper instalado pelo usuário); senão o
/// erro nomeado que manda escolher um provedor de nuvem. Devolve também quem falou de fato.
async fn generate_local(
    state: &TtsState,
    text: &str,
    cache_dir: &std::path::Path,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<(Vec<u8>, &'static str), String> {
    let language = state.supertonic_language.as_str();
    if supertonic::supports_language(language) {
        return Ok((supertonic::generate(text, &supertonic_options(state, cache_dir), store, cancel).await?, "supertonic"));
    }
    let Some(voice) = piper::catalog::voice_for_language(language) else {
        return Err(supertonic::unsupported_language_message(store));
    };
    let opts = PiperOptions { voice: voice.key.into(), rate: state.effective_speed(None), ..piper_options(state, cache_dir) };
    Ok((piper::generate(text, &opts, store, cancel).await?, "piper"))
}

/// Porte de `fallbackToLocal` + o `events.emit(Events.TTS_FALLBACK, ...)` de `processSingleChunk`
/// (`tts-speaker.ts`). `reason` é sempre o mesmo literal do TS — não uma mensagem nova inventada
/// aqui — porque o TS nunca diferenciava a causa exata no payload do evento, só no log local.
async fn fallback_to_local(
    state: &TtsState,
    text: &str,
    cache_dir: &std::path::Path,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Option<GeneratedAudio>, String> {
    let (audio, actual_provider) = generate_local(state, text, cache_dir, store, cancel).await?;
    bus().publish(
        EventKey::TtsFallback,
        serde_json::json!({
            "requestedProvider": "elevenlabs",
            "actualProvider": actual_provider,
            "reason": "Missing credentials for selected provider",
            "at": now_ms(),
        }),
    );
    Ok(Some(GeneratedAudio { audio, mime_type: "audio/wav" }))
}

async fn generate_elevenlabs_with_fallback(
    state: &TtsState,
    text: &str,
    cache_dir: &std::path::Path,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Option<GeneratedAudio>, String> {
    if state.eleven_labs.api_key.is_empty() || state.eleven_labs.voice_id.is_empty() {
        return fallback_to_local(state, text, cache_dir, store, cancel).await;
    }
    match generate_elevenlabs(text, &state.eleven_labs, store, cancel).await {
        Ok(audio) => Ok(Some(GeneratedAudio { audio, mime_type: REMOTE_MIME_TYPE })),
        Err(error) if is_elevenlabs_permanent_failure(&error) => fallback_to_local(state, text, cache_dir, store, cancel).await,
        Err(error) => Err(error),
    }
}

fn piper_options(state: &TtsState, cache_dir: &std::path::Path) -> PiperOptions {
    PiperOptions {
        voice: state.piper_voice.clone(),
        binary_path: state.piper_binary_path.clone(),
        rate: state.piper_rate,
        cache_dir: cache_dir.join("piper-voices"),
    }
}

/// Porte de `generateWithProvider`. `store` alcança o chokepoint de privacidade de rede
/// (`crate::modules::privacy::assert_network_allowed`, mesmo ponto único que `createLoggedFetch`
/// é do lado TS) antes de qualquer download de modelo/voz — ver `providers/supertonic/assets.rs`
/// e `providers/piper.rs`. `cancelled` é checado pelo chamador (`speaker.rs`) antes E depois — aqui
/// só existe para o Supertonic/Piper poderem abortar no meio da síntese (`spawn_blocking` não é
/// interrompível por fora; ver `providers/supertonic/model.rs`/`providers/piper.rs`).
///
/// `Ok(None)` é "nenhum áudio, sem erro" — só o provedor `system` devolve isto (ver o cabeçalho).
/// Todo outro sucesso é `Ok(Some(_))`.
pub async fn generate_with_provider(
    state: &TtsState,
    text: &str,
    cache_dir: &std::path::Path,
    store: &ConfigStore,
    cancel: &CancelHandle,
) -> Result<Option<GeneratedAudio>, String> {
    match state.provider.as_str() {
        "system" => Ok(None),
        "supertonic" => {
            let (audio, _) = generate_local(state, text, cache_dir, store, cancel).await?;
            Ok(Some(GeneratedAudio { audio, mime_type: "audio/wav" }))
        }
        "piper" => {
            let audio = piper::generate(text, &piper_options(state, cache_dir), store, cancel).await?;
            Ok(Some(GeneratedAudio { audio, mime_type: "audio/wav" }))
        }
        "elevenlabs" => generate_elevenlabs_with_fallback(state, text, cache_dir, store, cancel).await,
        "openai-tts" => {
            let audio = generate_openai(text, &state.openai_tts, store, cancel).await?;
            Ok(Some(GeneratedAudio { audio, mime_type: REMOTE_MIME_TYPE }))
        }
        "azure-tts" => {
            let audio = generate_azure(text, &state.azure_tts, store, cancel).await?;
            Ok(Some(GeneratedAudio { audio, mime_type: REMOTE_MIME_TYPE }))
        }
        "google-tts" => {
            let audio = generate_google(text, &state.google_tts, store, cancel).await?;
            Ok(Some(GeneratedAudio { audio, mime_type: REMOTE_MIME_TYPE }))
        }
        other if CUT_PROVIDERS.contains(&other) => Err(format!(
            "provedor de TTS '{other}' ainda não foi portado para Rust — ver o cabeçalho de dispatch.rs \
             para o porquê do corte. Selecione 'supertonic' para voz local."
        )),
        other => Err(format!("provedor de TTS desconhecido: '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::boolean;
    use crate::modules::voice::tts::abort_registry::AbortRegistry;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-tts-dispatch-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "privacy",
            fields: vec![boolean("offlineMode", false), boolean("screenCapture", false)],
        }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    fn cancel() -> crate::modules::voice::tts::abort_registry::CancelHandle {
        let registry = Box::leak(Box::new(AbortRegistry::new()));
        registry.register_active().handle()
    }

    fn offline_store() -> (ConfigStore, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![Namespace {
            name: "privacy",
            fields: vec![boolean("offlineMode", true), boolean("screenCapture", false)],
        }]);
        (ConfigStore::open(&dir.0, schema).unwrap(), dir)
    }

    #[tokio::test]
    async fn cut_providers_fail_named_instead_of_silently_falling_back() {
        let (store, _dir) = store();
        for provider in CUT_PROVIDERS {
            let mut state = TtsState::default();
            state.provider = provider.to_string();
            let err = generate_with_provider(&state, "oi", std::path::Path::new("."), &store, &cancel())
                .await
                .expect_err(&format!("'{provider}' devia falhar nomeado, não gerar áudio"));
            assert!(err.contains(provider), "erro não nomeia o provedor cortado: {err}");
        }
    }

    /// A prova central do item 1: `system` NÃO é um corte. Porte fiel de `factory.ts` ("'system' or
    /// unknown: no runtime audio generation" -> `null`) — `Ok(None)`, nunca `Err`, para que
    /// `speak_chunk` (`speaker/chunk.rs`) não logue um erro por frase para quem escolheu esta opção
    /// de propósito.
    #[tokio::test]
    async fn system_provider_is_a_silent_success_not_a_named_cut() {
        let (store, _dir) = store();
        let mut state = TtsState::default();
        state.provider = "system".into();
        let result = generate_with_provider(&state, "oi", std::path::Path::new("."), &store, &cancel()).await;
        assert!(matches!(result, Ok(None)), "'system' devia devolver Ok(None), não {result:?}");
    }

    #[tokio::test]
    async fn unknown_provider_also_fails_named_not_silently() {
        let (store, _dir) = store();
        let mut state = TtsState::default();
        state.provider = "not-a-real-provider".into();
        let err = generate_with_provider(&state, "oi", std::path::Path::new("."), &store, &cancel()).await.unwrap_err();
        assert!(err.contains("desconhecido"));
    }

    /// Prova de wiring, não de HTTP: os 3 provedores remotos SEM fallback automático (OpenAI, Azure,
    /// Google) não caem mais no branch de `CUT_PROVIDERS` — cada um chega ao gerador de verdade e
    /// falha pela credencial ausente (checada antes de qualquer rede), não pela mensagem genérica
    /// "ainda não foi portado". ElevenLabs sai desta lista porque credencial ausente não é mais
    /// falha — é o gatilho do fallback (ver os testes de `generate_elevenlabs_with_fallback`).
    ///
    /// `openaiTtsBaseUrl` default é `localhost:8880` — um Kokoro-FastAPI de verdade pode estar no
    /// ar na máquina de quem roda o teste (é o cenário que o próprio produto espera). Sem apontar
    /// para uma porta morta aqui, o teste ficaria flaky por vizinho, a mesma classe que
    /// `gamer/llm_dead_port.rs` já documenta.
    #[tokio::test]
    async fn the_three_provider_only_remotes_reach_their_real_generator_instead_of_the_cut_branch() {
        let (store, _dir) = store();
        for provider in ["openai-tts", "azure-tts", "google-tts"] {
            let mut state = TtsState::default();
            state.provider = provider.to_string();
            if provider == "openai-tts" {
                state.openai_tts.base_url = "http://127.0.0.1:1".into();
            }
            let err = generate_with_provider(&state, "oi", std::path::Path::new("."), &store, &cancel())
                .await
                .expect_err(&format!("'{provider}' sem credencial devia falhar"));
            assert!(
                !err.contains("ainda não foi portado"),
                "'{provider}' ainda cai no branch de corte nomeado: {err}"
            );
        }
    }

    /// `piper` também saiu de `CUT_PROVIDERS` — chega ao gerador real e falha pelo binário ausente
    /// (`providers/piper.rs::resolve_binary`), não pela mensagem genérica de corte.
    #[tokio::test]
    async fn piper_reaches_its_real_generator_instead_of_the_cut_branch() {
        let (store, _dir) = store();
        let mut state = TtsState::default();
        state.provider = "piper".into();
        state.piper_binary_path = "/does/not/exist/piper".into();
        let err = generate_with_provider(&state, "oi", std::path::Path::new("."), &store, &cancel()).await.unwrap_err();
        assert!(!err.contains("ainda não foi portado"), "piper ainda cai no branch de corte nomeado: {err}");
        assert!(err.contains("caminho configurado não existe"), "erro não é o do piper de verdade: {err}");
    }

    // --- idioma que o Supertonic não fala -------------------------------------------------------

    fn locale_store(locale: &str) -> (ConfigStore, TempDir) {
        use crate::config::schema::choice;
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "privacy", fields: vec![boolean("offlineMode", false), boolean("screenCapture", false)] },
            Namespace { name: "app", fields: vec![choice("locale", "pt-BR", &["pt-BR", "de-DE", "ja-JP"])] },
        ]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();
        store.set("app.locale", serde_json::json!(locale)).unwrap();
        (store, dir)
    }

    fn supertonic_state(language: &str) -> TtsState {
        let mut state = TtsState::default();
        state.provider = "supertonic".into();
        state.supertonic_language = language.into();
        state.piper_binary_path = "/does/not/exist/piper".into();
        state
    }

    /// Alemão não está no Supertonic, mas há voz Piper verificada: a fala chega no Piper (que aqui
    /// falha pelo binário ausente, com o caminho configurado no erro) em vez de morrer no erro de idioma.
    #[tokio::test]
    async fn supertonic_falls_back_to_the_piper_voice_of_a_language_the_model_cannot_speak() {
        let (store, _dir) = locale_store("de-DE");
        let err = generate_with_provider(&supertonic_state("de"), "Hallo", std::path::Path::new("."), &store, &cancel())
            .await
            .unwrap_err();
        assert!(err.contains("/does/not/exist/piper"), "não chegou no Piper: {err}");
    }

    /// Japonês não tem voz Piper verificada: o erro nomeia o idioma e manda para a nuvem — nunca
    /// fonemas de outro idioma nem voz inventada.
    #[tokio::test]
    async fn a_language_without_any_verified_local_voice_names_the_language_and_points_to_the_cloud() {
        let (store, _dir) = locale_store("ja-JP");
        let err = generate_with_provider(&supertonic_state("ja"), "こんにちは", std::path::Path::new("."), &store, &cancel())
            .await
            .unwrap_err();
        assert!(err.contains("Japanese") && err.contains("Google"), "{err}");
        assert!(!err.contains("/does/not/exist/piper"), "tentou o Piper sem voz verificada: {err}");
    }

    // --- fallback ElevenLabs -> Supertonic ------------------------------------------------------

    #[test]
    fn is_elevenlabs_permanent_failure_matches_only_the_three_classified_causes() {
        assert!(is_elevenlabs_permanent_failure("ElevenLabs TTS: sem crédito na conta (HTTP 401): ..."));
        assert!(is_elevenlabs_permanent_failure("ElevenLabs TTS: credencial inválida (HTTP 403): ..."));
        assert!(is_elevenlabs_permanent_failure("ElevenLabs TTS: voz não coberta pelo plano (HTTP 402): ..."));
        // Falha de TRANSPORTE (timeout, DNS, conexão recusada) nunca é permanente — propaga como
        // erro normal, mesma distinção que o TS faz entre `buffer === null` e uma exceção de fetch.
        assert!(!is_elevenlabs_permanent_failure("ElevenLabs TTS falhou: error sending request for url"));
    }

    /// Credencial ausente pula direto para o Supertonic — mesmo gatilho de `generateElevenLabs`
    /// (`if (!state.apiKey || !state.voiceId)`). Modo offline aqui não é o que está sob teste: é o
    /// jeito determinístico de provar que o caminho tentado foi o Supertonic (erro de rede
    /// bloqueada, não "ElevenLabs: apiKeys.elevenLabs não configurada"), sem tocar rede real nem
    /// baixar os pesos do modelo.
    #[tokio::test]
    async fn missing_elevenlabs_credentials_attempt_the_supertonic_fallback_instead_of_failing_locally() {
        let (store, _dir) = offline_store();
        let mut state = TtsState::default();
        state.provider = "elevenlabs".into();
        let err = generate_with_provider(&state, "oi", std::path::Path::new("."), &store, &cancel()).await.unwrap_err();
        assert!(err.to_lowercase().contains("offline"), "não tentou o Supertonic (rota de fallback): {err}");
        assert!(!err.contains("apiKeys.elevenLabs"), "não devia ter chegado no erro nomeado do ElevenLabs: {err}");
    }
}
