//! Módulo `settings` em Rust — port de `apps/desktop/src/main/ipc/settings.ts`.
//!
//! Reivindica os 4 canais de settings. `mika:settings-declarations` foi o último a sair do
//! sidecar, em 2026-09-21 — o histórico de por que ele ficou para trás está preservado abaixo,
//! porque o motivo (catálogo parcial apaga tela em silêncio) vale para qualquer canal que sirva um
//! REGISTRO populado em runtime, não só para este. O handler
//! (`registerDesktopInvokeRoute(...settingsDeclarations, ...)`)
//! não lê `ConfigStore` — chama `featureRegistryPort().getAllSettings()`
//! (`packages/sdk/src/features/feature-registry.ts`), monta a lista de painéis (`{tabId, groups,
//! ...}`) a partir do que cada feature registrou em RUNTIME, e passa o resultado por
//! `localizeFeatureSettings()` para resolver chaves i18n em texto antes de devolver. O `Declaration`
//! do `ConfigStore` (`{namespace, fields}`) não tem tabId nem grupos — reivindicar o canal hoje
//! substituiria a tela de Settings inteira por um payload que o renderer não reconhece.
//!
//! **A fonte não é só "as features ainda não portadas" (`modules/features/`, hoje vazio de
//! propósito).** `apps/desktop/src/main/services/bootstrap/tools.ts::bootstrapTools` registra no
//! MESMO `featureRegistry`, via `registerFeatureWithTools`, cinco módulos do NÚCLEO que já têm
//! módulo Rust — `mika.brain` (`BRAIN_SETTINGS`, `LLM_SETTINGS`), `mika.embeddings`
//! (`EMBEDDING_SETTINGS`), `mika.voice` (`VOICE_SETTINGS`, `STT_SETTINGS`), `mika.vision`
//! (`VISION_SETTINGS`) e `mika.browser` (`BROWSER_SETTINGS`) — nenhum dos quais monta hoje, do lado
//! Rust, o equivalente a um painel de settings (`FeatureSettingsDeclaration`); eles só portaram
//! config/comportamento, não a declaração de UI. Some a isso as 9 "features tardias"
//! (`character`, `desktop`, `gamer-companion`, `mcp`, `mood`, `person-catalog`, `privacy`,
//! `scheduler`, `user`) cujos painéis só existem depois que `PluginLoader::boot()` roda `activate()`
//! em cada manifest — runtime real, não algo que dá para ler do disco. Portar `modules/features/`
//! sozinho NÃO desbloqueia este canal: faltaria ainda um `FeatureRegistryService` equivalente em
//! Rust que os módulos de núcleo também alimentassem. Reivindicar parcial (só o que já existe em
//! Rust) apagaria da tela os painéis de brain/voice/vision/browser/embeddings e de toda feature
//! tardia, em silêncio — o modo de falha que esta tarefa pede para evitar. Fica documentado aqui,
//! e não como TODO solto, para quem tocar `modules/features/` a seguir não reabrir a investigação:
//! ver também `modules/tools.rs` (`mika:plugin-ui-catalog` bate na mesma parede) e
//! `modules/features/mod.rs`.
//!
//! `get`/`set`/`ui-prefs`, ao contrário, são só leitura e escrita no `ConfigStore`, que já existe e
//! já tem paridade com os schemas Zod portados — por isso só estes três saem do sidecar aqui.
//!
//! `handlers.rs` traz a lógica de cada rota; os literais de canal ficam aqui porque o gate de
//! atrito (`scripts/checks/sidecar-attrition.mts`) só resolve `const` na MESMA fonte que declara
//! `fn channels()` — espalhar os literais para `handlers.rs` faria o placar contar estes 3 canais
//! como sidecar de novo.
//!
//! ## Atualização (rodada seguinte): i18n resolvido, registro ainda parcial
//!
//! `i18n/` fecha o pré-requisito de i18n descrito acima (`localize_feature_settings`/`t`,
//! testados, port de `localizeFeatureSettings`/`t` do lado TS). `modules/vision/feature.rs` e
//! `modules/llm/feature.rs` (frentes próprias) também já publicam suas abas no
//! `feature_registry::shared()`. Ainda **não reivindico o canal**: `mika.brain`, `mika.embeddings`,
//! `mika.browser`, `mika.voice` e as 9 features tardias (`modules/features/`, `modules/plugins/`)
//! continuam sem registrar as próprias abas — populado PARCIAL apagaria o resto da tela em
//! silêncio, o mesmo modo de falha que este porte existe para evitar. Ver o relatório da rodada
//! para o inventário exato de quem falta.
//!
//! ## Atualização (rodada 2026-09-21, gamer-companion/person-catalog/scheduler): 4 → 13 de 14, um
//! ## nome só falta — `voice`
//!
//! `gamer::feature`, `features::person_catalog::feature` e `features::scheduler::feature` (frente
//! própria) publicaram as 3 abas que faltavam para fechar `mika.brain`/`mika.browser`/
//! `mika.embeddings` (já hand-declared) + `mika.character`/`mika.user`/`mika.mood` (portados em
//! paralelo). **Ainda não reivindico o canal.** Medido AO VIVO, não por suposição: um teste
//! temporário (`zzz_probe_full_boot_tab_count`, chamado uma vez e removido antes deste commit —
//! nunca ficou no histórico como produção) simulou o boot inteiro — todo `modules()`/`register()`
//! que não pede `AppHandle` de verdade — e `feature_registry::shared().get_all_settings()` devolveu
//! 13 `tabId`: `brain, browser, embeddings, character, user, mood, vision, llm, mcp, privacy,
//! gamer-companion, person-catalog, scheduler`. **Falta só `voice`**
//! (`apps/desktop/src/main/services/voice/voice.schema.ts`, `tab: { id: 'voice' }`,
//! `VOICE_SETTINGS`/`STT_SETTINGS`) — sem `modules/voice/feature.rs`, e `modules/voice/` é de outra
//! frente nesta rodada, fora do meu escopo tocar. Reivindicar agora apagaria a aba Voz da tela do
//! usuário em silêncio — a MESMA regressão que motivou não reivindicar antes, só que agora falta
//! UM nome nomeado, não "a maioria". `handlers::build_declarations` já existe, pronta e testada
//! (mesmo padrão de `localize_feature_settings` ter ficado pronta sem chamador por uma rodada
//! inteira) — ligar o canal quando `voice` publicar sua aba é uma linha em `CHANNELS` mais um braço
//! de `match` em `invoke()`, nada mais muda aqui.
//!
//! ## Reivindicado (2026-09-21): 15 abas, e por que "13 de 14" e "13 de 15" estavam os DOIS errados
//!
//! `modules/voice/feature.rs` publicou `voice` e `stt` (um `voice.schema.ts`, DOIS `tabId`), e com
//! isso o canal saiu do sidecar. O número de abas, porém, foi medido errado três vezes seguidas
//! nesta migração — 14, 15 e 13 — e a lição não é o número:
//!
//! **Catálogo incompleto se mede pela diferença dos CONJUNTOS, nos dois sentidos, nunca pelo
//! total.** Uma medição intermediária desta rodada deu "TS 13 × Rust 12, falta 1" e quase levou a
//! reivindicar. Os conjuntos não se sobrepunham: aquela contagem tinha perdido `brain`, `browser` e
//! `embeddings` (3) e contava `mood` e `scheduler` (2), que só o Rust declara. Ter reivindicado por
//! causa do "falta 1" teria apagado TRÊS telas.
//!
//! A causa do erro de medição é estrutural e vai se repetir: existem DOIS caminhos de registro de
//! aba, e um probe que chame só um deles enxerga metade do catálogo.
//!
//! 1. **Efeito colateral de `modules()`**, em `native::registry()` — `brain`, `browser` e
//!    `embeddings` (esta última mora em `modules/memory/settings.rs`, sob `pluginId`
//!    `mika.embeddings`, não em `modules/llm/`). Aba que não precisa de `ConfigStore` nem de
//!    `AppHandle` cabe aqui.
//! 2. **`modules::register_config_bound_tools(app)`**, de `lib.rs::setup` — as oito
//!    `register_feature()` mais `character`/`user`/`mood`, que declaram a aba DINAMICAMENTE por
//!    `features/settings_declaration.rs`. Estas três não têm literal `tabId` no fonte: `grep` não
//!    as acha, e foi por isso que duas contagens anteriores as perderam.
//!
//! Medido ao vivo com os dois caminhos (probe descartável, removido antes deste commit):
//! `brain, browser, character, embeddings, gamer-companion, llm, mcp, mood, person-catalog,
//! privacy, scheduler, stt, user, vision, voice` — 15, cobrindo as 13 do TS mais `mood` e
//! `scheduler`, que são namespaces só-Rust. `modules/mod.rs::the_catalog_covers_every_tab_the_ts_declares`
//! fixa isso: lê os 13 `defineFeatureSchema` do fonte TypeScript e exige cada `tabId` no registro
//! vivo, para o canal nunca mais poder ser servido com tela faltando.
//!
//! `mika:plugin-ui-catalog`, ao contrário, **já foi reivindicado** (`plugin_ui.rs`, mesmo diretório
//! — canal e `invoke()` próprios, ver o cabeçalho de lá) — aquele canal lê `views`/`settings_panels`,
//! nunca `settings`, e os dois únicos `FeatureDefinition` que declaram algo ali (`gamer-companion`,
//! `person-catalog`) já registram as próprias views/painéis; a lacuna de `voice` não o alcança.

mod handlers;
mod i18n;
pub mod plugin_test;
/// Higiene de segredo na mensagem que o teste de conexão devolve para a tela — mora fora de
/// `plugin_test` porque vale para todo ramo, não é detalhe de um deles.
mod test_redaction;
mod plugin_ui;

use async_trait::async_trait;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::native::NativeModule;
use crate::protocol::{InvokeContext, WindowType};
use handlers::{assert_owner_surface, build_declarations, build_get_all, build_ui_prefs, handle_set};

pub use i18n::{localize_feature_settings, t, tf};
pub use plugin_ui::PluginUiModule;

const CH_GET_ALL: &str = "mika:settings-get-all";
const CH_SET: &str = "mika:settings-set";
const CH_UI_PREFS: &str = "mika:settings-ui-prefs";
const CH_DECLARATIONS: &str = "mika:settings-declarations";
/// O botão "Validar" de cada aba. Mora neste módulo porque a rota é `ipc/settings.ts`, e a
/// implementação por plugin está em [`plugin_test`].
const CH_PLUGIN_TEST: &str = "mika:plugin-test";
const CHANNELS: &[&str] = &[CH_GET_ALL, CH_SET, CH_UI_PREFS, CH_DECLARATIONS, CH_PLUGIN_TEST];

/// Espelha `DESKTOP_EVENT_CHANNELS.ConfigChanged` (`config-changed`,
/// `packages/sdk/src/contracts/desktop-ipc-channels.ts`). `useDesktopShellBootstrap`/
/// `useSettingsController` (renderer) só atualizam o estado local por ESTE evento —
/// `saveSetting`/`api.runtime.saveSetting` nunca escrevem de volta no próprio `then()`, e a UI
/// não relê a config sozinha. Sem emitir aqui, `ConfigStore::set` grava de verdade e a tela nunca
/// reflete: fica presa no valor antigo até um refetch manual (reabrir a janela) — o "controle sem
/// resposta" do E2E (`controls-audit.spec.ts`) e o `stage-backdrop-fill` que nunca muda de opacidade
/// em `avatar-background-modes.spec.ts`. Esta constante e `broadcast_config_changed` abaixo
/// faltavam por INTEIRO (não só na segunda chamada) — o `CH_SET` de antes devolvia
/// `{success:true, change:{accepted:true, ...}}` sem nunca notificar ninguém, provado por
/// `tests::two_consecutive_sets_emit_config_changed_twice`.
const EVENT_CONFIG_CHANGED: &str = "config-changed";

/// Genérico em `R: tauri::Runtime` de propósito — não em `AppHandle` (alias para
/// `AppHandle<tauri::Wry>`, o runtime real) — para o teste de regressão poder chamar a MESMA
/// função com o `AppHandle<MockRuntime>` de `tauri::test::mock_app()`, sem precisar de uma janela
/// de verdade. `NativeModule::invoke` continua preso ao `AppHandle` concreto (a trait não é
/// genérica em runtime — mudar isso alcançaria uns 15 outros módulos, fora do escopo cirúrgico
/// deste fix), por isso a extração: é a MENOR fatia testável sem um webview real por trás.
///
/// NÃO usa `renderer_event_targets` (a regra de `assistant_bridge`/`expression_bridge`): aquela
/// função devolve só `main` quando ela existe, o que descartaria a PRÓPRIA janela `settings` —
/// exatamente quem chamou `saveSetting` e precisa do eco para sair do estado otimista.
/// `config-changed` é estado de app, não evento de conversa/avatar: todo mundo que não seja
/// `plugin-view` (superfície não confiável, mesma regra) precisa saber.
fn broadcast_config_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>, change: Value) {
    for label in app.webview_windows().keys().cloned().collect::<Vec<_>>() {
        if WindowType::from_label(&label) == WindowType::PluginView {
            continue;
        }
        let _ = app.emit_to(label.as_str(), EVENT_CONFIG_CHANGED, change.clone());
    }
}

pub struct SettingsModule;

#[async_trait]
impl NativeModule for SettingsModule {
    fn name(&self) -> &'static str {
        "settings"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        let store = crate::state::config_store(app)?;

        match channel {
            CH_GET_ALL => {
                assert_owner_surface(ctx, "settingsGetAll")?;
                Ok(build_get_all(&store))
            }
            // Sem `assertOwnerSurface` de propósito, igual ao TS: só locale+tema, para a
            // plugin-view não abrir com aparência errada.
            CH_UI_PREFS => Ok(build_ui_prefs(&store)),
            // Também sem `assertOwnerSurface`, e também igual ao TS: a rota é metadado de tela, e
            // uma plugin-view precisa dele para se desenhar. A guarda que falta aqui não é
            // esquecimento — é o contrato, e divergir dele quebraria a view do plugin.
            CH_DECLARATIONS => Ok(build_declarations(&store)),
            CH_SET => {
                assert_owner_surface(ctx, "settingsSet")?;
                let result = handle_set(&store, &payload);
                // Trocar o idioma tem de alcançar a bandeja: o menu dela é montado uma vez, no
                // boot, e não relê a config sozinho. Era o sidecar quem reagia à mudança e
                // devolvia os rótulos por `setTrayMenu`; sem este gancho, a bandeja ficaria no
                // idioma antigo até reiniciar — funcionando e errada, sem um único erro.
                if payload.get("path").and_then(Value::as_str) == Some("app.locale")
                    && result.get("success").and_then(Value::as_bool) == Some(true)
                {
                    crate::tray::refresh_labels(app);
                }
                // Roda em TODA chamada bem-sucedida, não só na primeira — sem `if let Some(sent_once)`
                // nem estado de "já emiti". Ver `broadcast_config_changed` para o porquê do alvo.
                if result.get("success").and_then(Value::as_bool) == Some(true) {
                    if let Some(change) = result.get("change") {
                        broadcast_config_changed(app, change.clone());
                    }
                }
                Ok(result)
            }
            CH_PLUGIN_TEST => {
                // Mesmo `app_data_dir`/`supertonic` que `voice::speak_tool::register` resolve para
                // o cache de TTS — o teste de voz gera áudio de verdade e precisa cair no MESMO
                // cache da fala, não num diretório próprio que ninguém mais lê.
                let cache_dir = crate::state::app_data_dir(app).map_err(|e| format!("não resolveu app_data_dir: {e}"))?.join("supertonic");
                plugin_test::dispatch(&store, ctx, &cache_dir, &payload).await
            }
            other => Err(format!("settings: canal '{other}' declarado mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    // Dois `NativeModule`s: `PluginUiModule` reivindica `mika:plugin-ui-catalog` sozinho, sem
    // depender de `ConfigStore` nem da guarda de superfície de `SettingsModule` — ver o cabeçalho
    // de `plugin_ui.rs` para o porquê dos dois não se misturarem num só.
    vec![Box::new(SettingsModule), Box::new(PluginUiModule)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_the_exact_literals() {
        let module = SettingsModule;
        assert_eq!(
            module.channels(),
            &[
                "mika:settings-get-all",
                "mika:settings-set",
                "mika:settings-ui-prefs",
                "mika:settings-declarations",
                // Último canal do sidecar, reivindicado em 2026-09-22 — a catraca chega a zero.
                "mika:plugin-test",
            ]
        );
    }

    // -- As premissas do porte de `mika:settings-declarations` -----------------------------------
    //
    // Estes três nasceram prendendo a investigação de por que o canal FICAVA no sidecar. O canal
    // saiu (2026-09-21) e eles continuam valendo, com o papel invertido: agora prendem as premissas
    // que o porte assumiu sobre o lado TS. Se qualquer um dos textos-fonte citados mudar de forma,
    // quem mexer é forçado a reler — em vez de a premissa envelhecer calada.

    // `ipc/settings.ts` foi removido (backend Node deletado na migração para Tauri) — a premissa
    // que este teste prendia ("o handler monta a resposta a partir do featureRegistry, não do
    // ConfigStore") deixou de ter um lado TS para divergir: `build_declarations` lendo
    // `feature_registry::shared()` é agora a própria produção, não um porte a verificar contra o
    // original. Histórico preservado no comentário, não no `include_str!`.

    // A fonte não era só "features tardias": cinco módulos de NÚCLEO com módulo Rust próprio
    // (`brain`, `embeddings`, `voice`, `vision`, `browser`) também registram painéis de settings
    // no MESMO `featureRegistry`. Era o que fazia "portar `modules/features/`" não desbloquear o
    // canal, e é o que explica os DOIS caminhos de registro do lado Rust — ver o cabeçalho deste
    // arquivo.
    // `bootstrap/tools.ts` foi removido (backend Node deletado na migração para Tauri) — a
    // premissa "cinco módulos de núcleo (brain/embeddings/voice/vision/browser) também registram
    // painel no featureRegistry" continua verdadeira do lado Rust, só que agora verificada em
    // produção por `modules::mod.rs::the_catalog_covers_every_tab_the_ts_declares` (que chama
    // `vision::register_feature()`/`llm::register_feature()`/etc. e confere os tabIds publicados),
    // não mais por comparação contra o TS.

    // -- `config-changed`: a REGRESSÃO real, não a hipotética -----------------------------------
    //
    // Achada diagnosticando `avatar-background-modes.spec.ts` (E2E): `saveSetting` resolvia
    // `{accepted:true}`, `getSettingsConfig()` confirmava o valor NOVO gravado, e mesmo assim um
    // listener fresco em `onConfigChanged`, registrado 1s antes da chamada, nunca recebia nada —
    // nem uma vez, não só "na segunda". `git show HEAD:...settings/mod.rs` confirmou: o `CH_SET`
    // não tinha `emit_to` NENHUM. `EVENT_CONFIG_CHANGED` acima e o laço em `CH_SET` são o fix; este
    // teste prova as DUAS emissões — uma por `set`, não só a primeira — usando o `App<MockRuntime>`
    // oficial de `tauri::test` (crate já em `[dependencies]`, feature `test` só em
    // `[dev-dependencies]` — não entra em `cargo build --release`).
    use std::sync::{Arc, Mutex};

    use serde_json::json;
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::{Listener, WebviewWindowBuilder};

    use crate::config::store::ConfigStore;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mika-settings-config-changed-test-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A prova em si: dois `set` bem-sucedidos em sequência emitem DUAS vezes, não uma. Escuta na
    /// `WebviewWindow` "main" (`EventTarget::WebviewWindow{label:"main"}`) — o mesmo alvo que
    /// `app.emit_to("main", ...)` casa (`filter_target` em `tauri::manager`); `app.listen(...)`
    /// puro (alvo `EventTarget::App`) NÃO casaria com um `emit_to` por label e daria falso
    /// negativo, então não é usado aqui.
    ///
    /// Chama `handle_set` + `broadcast_config_changed` diretamente, não `SettingsModule::invoke`
    /// através do trait: `NativeModule::invoke` está preso ao `AppHandle` concreto (`AppHandle<Wry>`),
    /// e `tauri::test::mock_app()` devolve `AppHandle<MockRuntime>` — os dois nem compilam juntos
    /// (`E0308`). Esta é a MESMA dupla que `CH_SET` chama em produção, só sem a casca do `match` do
    /// `invoke()` (que é 3 linhas triviais de checagem de `path`/`success`, já coberta por
    /// `handlers::tests::set_accepts_a_declared_path...`).
    #[test]
    fn two_consecutive_sets_emit_config_changed_twice() {
        let dir = TempDir::new();
        let store = std::sync::Arc::new(ConfigStore::open(&dir.0, crate::config::schemas::registry()).unwrap());

        let app = mock_builder().build(mock_context(noop_assets())).unwrap();
        app.manage(crate::state::ShellState::for_test_with_config_store(Arc::clone(&store)));

        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("janela mock 'main' não subiu");

        let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let received_for_listener = Arc::clone(&received);
        window.listen(EVENT_CONFIG_CHANGED, move |event| {
            received_for_listener.lock().unwrap().push(event.payload().to_string());
        });

        let handle = app.handle();

        let first = handle_set(&store, &json!({ "path": "memory.enabled", "value": false }));
        assert_eq!(first["success"], json!(true), "primeiro set falhou: {first:?}");
        broadcast_config_changed(handle, first["change"].clone());

        let second = handle_set(&store, &json!({ "path": "memory.enabled", "value": true }));
        assert_eq!(second["success"], json!(true), "segundo set falhou: {second:?}");
        broadcast_config_changed(handle, second["change"].clone());

        let events = received.lock().unwrap();
        assert_eq!(
            events.len(),
            2,
            "esperava 2 emissões de '{EVENT_CONFIG_CHANGED}' (uma por set bem-sucedido), recebeu {}: {events:?}",
            events.len()
        );
        assert!(events[0].contains("\"value\":false"), "primeira emissão não carrega o primeiro valor: {}", events[0]);
        assert!(events[1].contains("\"value\":true"), "segunda emissão não carrega o segundo valor: {}", events[1]);
    }
}
