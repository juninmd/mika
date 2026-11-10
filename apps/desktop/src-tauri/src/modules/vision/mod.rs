//! Módulo `vision` em Rust — port do backend TypeScript correspondente
//! (`apps/desktop/src/main/services/vision/`, ~4823 LOC).
//!
//! `modules()` devolve `vec![]`: NENHUM canal de IPC de `vision` existe hoje (ver §1) — mas isso não
//! quer dizer "nada portado". [`register`] monta uma tool real, testada, pronta para o
//! `ToolRegistry` nativo; só falta uma chamada que este arquivo não pode fazer sozinho (§3).
//!
//! ## 1. Nenhum canal de IPC próprio para reivindicar
//!
//! `DESKTOP_INVOKE_CHANNELS`/`DESKTOP_RUNTIME_CORE_ROUTES`/`DESKTOP_RUNTIME_OPS_ROUTES`/
//! `DESKTOP_SHELL_INVOKE_ROUTES` (`packages/sdk/src/contracts/desktop-ipc*.ts`) — zero entradas com
//! prefixo `vision`. A única string relacionada é o EVENTO (main→renderer, nunca invocado)
//! `VisionScreenObservation = 'mika:event:vision:screen-observation'`. `vision.module.ts` confirma o
//! motivo: `vision` roda como laço proativo interno (timer, não request-response do renderer), e as
//! duas tools que declara (`vision_capture`, `vision_read`, `vision.feature.ts`) são despachadas
//! pelo canal genérico `mika:tool-execute` — dono `modules::tools`, outra frente, "tem dono".
//!
//! ## 2. Tabela de paridade TS × Rust (remedida em 2026-09-22 — a tabela anterior estava ERRADA)
//!
//! A rodada anterior classificou as 5 actions abaixo como bloqueadas por "não há chamada de LLM
//! multimodal em Rust". Falso: `rig-core` (já no grafo via `modules/brain/turn.rs`) suporta
//! imagem em `UserContent::image_base64`, e o provedor Ollama (`providers/ollama.rs`) já traduz isso
//! para o campo `images` do `/api/chat` — medido lendo o crate, não a partir de memória. As 5
//! despacham agora, com um cliente Ollama próprio ([`llm_client`], ver seu cabeçalho para por que
//! não deu para reusar `brain::turn` diretamente) e captura de pixels reais (`xcap`, já no crate via
//! `crate::capture`, que `frame` já provava e só descartava o `thumbnailBase64` por completar menos
//! que o necessário):
//!
//! | Tool | Action | Rust hoje |
//! |---|---|---|
//! | `vision_capture` | `frame` | portado ([`capture_tool`]) — só captura, sem análise |
//! | `vision_capture` | `screenshot` | **portado** ([`capture_tool`]) — captura + Ollama (`generateText`) |
//! | `vision_capture` | `watch_now` | **portado** ([`capture_tool`]) — idem, resposta em 1ª pessoa |
//! | `vision_read` | `describe` | **portado** ([`read_tool`]) — imagem do chamador + Ollama |
//! | `vision_read` | `ocr` | **portado** ([`read_tool`]) — captura + Ollama, prompt de transcrição |
//! | `vision_read` | `people` | **portado** ([`read_tool`]) — captura + Ollama + parse de JSON |
//!
//! O que ainda falta em relação ao TS, nomeado (nenhum vira "não suportado" — cada gap é uma
//! simplificação REAL, testada, não um mock):
//! - **Sem `LLMFactory` multi-provedor.** Só Ollama, mesmo recorte já aceito em `brain::turn`
//!   ("LLM local padrão", AGENTS.md §2.6) — um provedor remoto pago NUNCA roda por default aqui.
//! - **`people` sem tipo estruturado.** TS usa `generateObject` (schema Zod validado no cliente);
//!   Rust pede o mesmo shape por instrução de prompt e faz `serde_json::from_str` — ver o cabeçalho
//!   de [`people_tool`] para por que `schemars` (a via para replicar 1:1) foi considerada e recusada.
//! - **Sem auto-upsert no catálogo de pessoas NESTA tool** (ação `people` de `vision_read`) e
//!   **sem `SCREEN_OBSERVATION`/`SCREEN_ENTITIES`** (eventos que `screenshot`/`ocr` emitem no TS) —
//!   mesma classe de lacuna do §3 abaixo: dependem de `AppHandle`/EventBus que este handler não
//!   recebe. `features::person_catalog::catalog_store` também exige `&AppHandle`, mesmo obstáculo.
//!   O laço periódico ([`watcher`], que TEM `AppHandle` desde `spawn_screen_watcher`) já faz esse
//!   auto-upsert — ver o cabeçalho dele — este gap é só da tool sob demanda.
//! - **Sem contexto de app em primeiro plano** (`getForegroundAppContext`) em `screenshot`/
//!   `watch_now` — capacidade nova por SO, já descartada em `capture_gate.rs` pelo mesmo motivo.
//! - **Sem cache de OCR** (`OCR_CACHE_TTL_MS`, 15s) — otimização, não corretude.
//!
//! ## 3. `frame` está PORTADO mas não AUTOMATICAMENTE registrado — falta AppHandle, não falta xcap
//!
//! `capture::get_sources` (`xcap`) e `images::resize_bitmap` já existem e cobrem a captura em si.
//! [`register`] já monta as duas tools, com `action=frame` gateado por
//! `privacy::is_screen_capture_allowed` (o chokepoint de privacidade deste repositório: a captura
//! TEM de ser gateada NO ADAPTADOR, nunca pelo chamador) mais o escaneamento de janela visível de
//! [`capture_gate`]. O que falta é só quem CHAMA `vision::register(store)`:
//!
//! - `ToolHandler::call` (`modules/tools/registry.rs`) não recebe `AppHandle`, de propósito
//!   (comentário lá: "para não acoplar um caminho testável a um app Tauri inteiro").
//! - `modules()` (este arquivo) também não recebe `AppHandle` — roda em `ShellState::new()`, ANTES
//!   do Tauri montar o app (`state.rs`).
//! - Não existe hoje nenhum ponto entre "um módulo declara suas tools" e "o app tem `AppHandle`"
//!   onde um handler possa capturar o `ConfigStore` do processo. `.setup(\|app\| {...})`
//!   (`lib.rs`) é o único lugar com `AppHandle` no boot, e está fora do meu escopo nesta rodada.
//!
//! Três alternativas, todas descartadas por violar uma regra dura deste repositório em vez de
//! respeitá-la:
//!
//! - Abrir um `ConfigStore` próprio aqui reproduziria exatamente o bug que
//!   `state::tests::single_config_store_per_process` existe para impedir (dois caches do mesmo
//!   JSON; a segunda escrita apaga a primeira, sem erro).
//! - Registrar a tool sem checar `privacy.screenCapture` reabriria o vazamento que este repositório
//!   já corrigiu do lado TS uma vez (captura sem consentimento).
//! - Registrar com o gate hardcoded para sempre negar equivale a não registrar — pior, na prática,
//!   porque ocupa um slot no catálogo com uma tool que nunca pode ter sucesso.
//!
//! Isto é maior que `vision`: nenhum módulo do repositório resolveu esse problema ainda —
//! `modules/tools/registry.rs` documenta, na própria rodada 2026-09-21c, que `register()` só é
//! chamado em teste hoje, em todo o crate. [`register`] fica pronta, testada com `ConfigStore`
//! injetado (dependência explícita no construtor do handler, como `tools/registry.rs` pede), para
//! quem tiver o `AppHandle` primeiro — o boot (`lib.rs::setup`) ou quem ligar
//! `mika:tool-execute`/`mika:tools-list` (`modules/tools/`) chama `vision::register` uma vez.
//!
//! ## O que mais este módulo porta: heurísticas puras + os dois laços que agora as CHAMAM
//!
//! Remedido em 2026-09-22: a rodada anterior deste cabeçalho dizia "dormentes até a fase 6" para
//! [`image_hash`]/[`text_density`] — falso a partir desta rodada. As duas continuam puras (sem
//! `ConfigStore`/`AppHandle` na própria assinatura), mas agora têm CHAMADOR real:
//!
//! - [`image_hash`] — `image-hash.ts`: hash de mudança (dHash) entre observações. Consumido por
//!   [`watcher::spawn_screen_watcher`] (dedup entre ticks) e [`clipboard_watch::spawn_clipboard_watch`]
//!   (dedup de imagem colada).
//! - [`text_density`] — `text-density-gate.ts`: densidade de texto/UI de uma captura. Consumido por
//!   [`watcher::spawn_screen_watcher`] quando `vision.codeAwareness` está ligado.
//! - [`capture_gate`] — a metade pura de `isCaptureBlocked` que NÃO depende de janela em foco
//!   (`parseBlocklist`/`isBlockedWindowTitle`, `infrastructure/os/capture-blocklist.ts`) — já em uso
//!   por `capture_tool`/`screen_capture` (via `capture_screen_frames`, que [`watcher`] também chama),
//!   e fechando parte real da lacuna de privacidade multiplataforma sem inventar detecção de
//!   foreground window (capacidade nova, fora de escopo — ver seu cabeçalho).
//! - [`watcher`] — port funcional de `capture/screen-watcher.service.ts` (+ `capture-cadence.ts` +
//!   `capture-gate.ts::shouldObserve` + `observation-events.ts`): laço periódico, gate de
//!   observação, dedup, gate de densidade, análise, publicação de `EventKey::ScreenObservation`.
//!   Ver o cabeçalho do arquivo para as simplificações nomeadas (sem modo jogo, sem
//!   `foregroundApp`/`imagePaths`/`entities`, `onChange` virou reler o intervalo a cada volta).
//! - [`clipboard_watch`] — port funcional de `capture/clipboard-watch.ts`: laço de 1.5s, os TRÊS
//!   gates (`vision.enabled` + `vision.readClipboardImages` + `privacy.clipboardLogging`), leitura
//!   via `tauri-plugin-clipboard-manager`, encode RGBA->PNG, dedup, publicação de
//!   `EventKey::VisionClipboardImage`.
//! - [`clipboard_analysis`] — desde 2026-09-22, o consumidor real de `EventKey::VisionClipboardImage`
//!   que fechava a lacuna acima: porte de `vision-proactive.ts::analyzeClipboardImage` (análise via
//!   [`llm_client`] + publicação de `EventKey::BrainProactiveThought`).
//!
//! Nenhuma das duas heurísticas reimplementa decode/resize (reusam [`bitmap::resize_to_bitmap`], que
//! por sua vez reusa `images::resize_bitmap`, já portado).
//!
//! **Os dois laços RODAM em produção** — `modules/mod.rs::register_config_bound_tools` já chama os
//! dois (esta nota ficou parada de uma rodada anterior; corrigida em 2026-09-22 junto da assinatura
//! de `spawn_screen_watcher`, que ganhou um 4º parâmetro — `Option<Arc<GameTensionState>>`, o
//! relógio de tensão compartilhado com `brain::awareness::spawn_proactive_engine`):
//!
//! ```ignore
//! vision::spawn_screen_watcher(Arc::clone(&store), crate::events::bus(), app.clone(), Some(Arc::clone(&game_tension)));
//! vision::spawn_clipboard_watch(Arc::clone(&store), crate::events::bus(), app.clone());
//! ```
//!
//! Ver `docs/tauri-rust-migration.md` (fases 3–6) e o registro em `native/mod.rs`.
//!
//! ## 4. `feature.rs` — registro DIFERENTE, mesma lacuna de chamador
//!
//! [`feature::register_feature`] publica a declaração de settings (`VISION_SETTINGS`) no
//! `feature_registry::shared()` — não é o `ToolRegistry` do §3. Mesma classe de defeito, outro
//! registro: pronta, testada, sem `AppHandle` na assinatura (não precisa: é dado estático), e sem
//! chamador em produção ainda. Ver o cabeçalho de `feature.rs`.

mod bitmap;
mod capture_gate;
/// Também lido pelo contador de uso de programas (`features::desktop::usage`): app sensível na
/// blocklist nunca entra nas estatísticas.
pub(crate) use capture_gate::is_foreground_blocked;
mod capture_tool;
mod clipboard_analysis;
mod clipboard_watch;
mod compression;
mod feature;
mod image_hash;
mod llm_client;
mod observation_events;
mod people_tool;
mod read_tool;
mod responses;
mod screen_capture;
mod screenshot_thought;
mod text_density;
mod watcher;

use std::sync::Arc;

use crate::config::store::ConfigStore;
use crate::modules::tools::ToolDefinition;
use crate::native::NativeModule;

pub use clipboard_analysis::spawn_clipboard_image_analysis;
pub use clipboard_watch::spawn_clipboard_watch;
pub use feature::register_feature;
pub(crate) use screenshot_thought::proactive_thought_payload_for_screenshot;
#[cfg(any(test, feature = "e2e"))]
pub use watcher::ingest_analysis_for_e2e;
pub use watcher::spawn_screen_watcher;

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![]
}

/// Registra `vision_capture`/`vision_read` no `ToolRegistry` nativo. Chame UMA VEZ, com o
/// `ConfigStore` do processo — ver o §3 acima para por que isto não roda dentro de [`modules()`].
///
/// `app`: WP9 item 4 — só para `token_usage::record` (surface `"vision"`) nas 5 actions que chamam
/// o LLM sob demanda (`screenshot`/`watch_now`/`describe`/`ocr`/`people`), a MESMA lacuna que o
/// watcher já fechava (`analyze_with_usage`) e que estas 5 ainda não fechavam — o painel de custo só
/// via o gasto do laço periódico, nunca o de uma pergunta direta ("o que tem na minha tela?").
/// `Option<AppHandle>` (não `AppHandle` puro), MESMO motivo de `gamer::game_watcher::GameWatcher`
/// (ver o cabeçalho do campo `app` de lá): `AppHandle<Wry>` (produção) e `AppHandle<MockRuntime>`
/// (`tauri::test::mock_app()`) não compilam juntos, então os testes deste arquivo/dos dois handlers
/// continuam construindo sem app nenhum, e nunca gravam uso de token — sem precisar montar um app
/// Tauri real só para testar despacho de tool.
pub fn register(store: Arc<ConfigStore>, app: Option<tauri::AppHandle>) {
    crate::modules::tools::registry().register(
        ToolDefinition {
            name: "vision_capture".into(),
            description: Some(capture_tool::DESCRIPTION.into()),
            input_schema: capture_tool::schema(),
            plugin_id: "vision".into(),
            requires_window: false,
        },
        Arc::new(capture_tool::VisionCaptureHandler::new(Arc::clone(&store), app.clone())),
    );
    crate::modules::tools::registry().register(
        ToolDefinition {
            name: "vision_read".into(),
            description: Some(read_tool::DESCRIPTION.into()),
            input_schema: read_tool::schema(),
            plugin_id: "vision".into(),
            requires_window: false,
        },
        Arc::new(read_tool::VisionReadHandler::new(store, app)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// Ancora a decisão do §1: nenhum canal de IPC é reivindicado por `vision` — quem for registrar
    /// um precisa apagar este teste de propósito, mesma garantia que
    /// `modules/tools/mod.rs::claims_no_channels_yet` dá para o lado dele.
    #[test]
    fn claims_no_ipc_channel() {
        assert!(super::modules().is_empty());
    }

    /// Pin da investigação: as duas tools reais e as 6 actions que elas somam. `vision.feature.ts`
    /// e `vision-tools.ts` foram removidos (backend Node deletado na migração para Tauri) —
    /// `capture_tool.rs`/`read_tool.rs` são agora a única fonte de verdade, então o pin passa a ler
    /// o próprio dispatcher/`inputSchema` deles em vez do TS.
    #[test]
    fn feature_surface_matches_the_expected_tool_contract() {
        // O literal do enum é o contrato pinado (as 6 actions); o objeto ao redor ganhou uma
        // `description` (item 7, smoke: Gemma mandava `action` fora do enum) e virou multilinha —
        // formatação não é o que este teste protege.
        let capture_schema = include_str!("capture_tool.rs");
        assert!(capture_schema.contains(r#""enum": ["screenshot", "frame", "watch_now"]"#));

        let read_schema = include_str!("read_tool.rs");
        assert!(read_schema.contains(r#""enum": ["describe", "ocr", "people"]"#));
    }

    /// Pin do bloqueio do §3: se `ToolHandler::call` algum dia passar a receber algo que dê acesso
    /// ao `AppHandle`/`ConfigStore`, esta assinatura muda e este teste força reler se alguém já pode
    /// chamar `vision::register` de dentro do próprio `modules()`.
    #[test]
    fn tool_handler_still_does_not_receive_an_app_handle() {
        // `registry/mod.rs`, não `registry/tests.rs`: o que se quer fixar é a DECLARAÇÃO do trait.
        // Apontar para o arquivo de teste deixaria o gate verde lendo um `impl` de dublê.
        let source = include_str!("../tools/registry/mod.rs");
        assert!(
            source.contains("async fn call(&self, args: Value) -> Result<Value, String>"),
            "assinatura de ToolHandler::call mudou — releia o §3 do cabeçalho deste arquivo: pode ter destravado o registro automático de vision_capture(frame)"
        );
    }

    /// Mesmo padrão de `privacy.rs`/`capture_tool.rs`: diretório isolado por teste.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-vision-register-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `mika:tool-execute`/`mika:tools-list` (quando reivindicados) leem/despacham pelo
    /// `ToolRegistry` global — prova ponta a ponta que as duas tools aparecem no catálogo com o
    /// `plugin_id`/descrição certos, e que `action=frame` responde de verdade através do REGISTRO
    /// global (`vision.enabled=false` nesta store: determinístico, não toca `xcap` de verdade).
    #[tokio::test]
    async fn register_publishes_both_tools_and_frame_runs_through_the_global_registry() {
        use crate::config::registry::{Namespace, SchemaRegistry};
        use crate::config::schema::boolean;

        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "vision", fields: vec![boolean("enabled", false)] },
            Namespace { name: "privacy", fields: vec![boolean("screenCapture", false)] },
        ]);
        let store = ConfigStore::open(&dir.0, schema).unwrap();

        register(Arc::new(store), None);

        let all = crate::modules::tools::registry().get_all();
        let capture = all.iter().find(|tool| tool.name == "vision_capture").expect("vision_capture registrada");
        let read = all.iter().find(|tool| tool.name == "vision_read").expect("vision_read registrada");
        assert_eq!(capture.plugin_id, "vision");
        assert_eq!(read.plugin_id, "vision");
        assert!(capture.description.as_deref().unwrap().contains("Look at the user's screen"));

        let executed =
            crate::modules::tools::registry().execute("vision_capture", json!({ "action": "frame" })).await.unwrap();
        assert_eq!(executed["success"], json!(false));
        assert_eq!(executed["errorCode"], json!("not_configured"));
    }
}
