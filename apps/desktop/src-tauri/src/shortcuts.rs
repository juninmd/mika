//! Atalhos globais, sobre `tauri_plugin_global_shortcut` (registrado em lib.rs).
//!
//! O registro em si roda no thread principal via `run_on_main_thread`, dentro do próprio
//! plugin — bloqueante, mas rápido (é só a chamada nativa do SO: `RegisterHotKey` e
//! afins). O que chega até este módulo já é o resultado REAL do SO, não uma suposição:
//! `Ok` quer dizer que o SO aceitou o atalho para este processo, `Err` quer dizer que
//! recusou (sintaxe inválida ou outro app já dono da combinação).
//!
//! Quem dispara o atalho não é quem chamou `register` — o SO entrega o evento a
//! qualquer momento depois disso, então o aviso de volta ao sidecar vai pelo canal
//! fogo-e-esquece (`sidecar::notify`), igual a qualquer outro evento que nasce no shell
//! sem um pedido que o tenha originado (atalho, suspensão, bateria).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Canais que o renderer escuta quando o atalho dispara. Espelham `DESKTOP_EVENT_CHANNELS` em
/// `packages/sdk/src/contracts/desktop-ipc-channels.ts`.
const TOGGLE_MIC: &str = "shortcut:toggle-mic";
const TOGGLE_INPUT: &str = "shortcut:toggle-input";
/// Segurar-para-falar: `{ "pressed": bool }` na borda de descida e na de subida da tecla.
const PUSH_TO_TALK: &str = "shortcut:push-to-talk";

/// O que cada atalho do app faz. Espelha `appShortcuts()` de `src/main/shell/app-shortcuts.ts`.
#[derive(Clone, Copy)]
enum AppShortcut {
    /// F9: avisa o renderer, e **não** traz a janela para a frente — mexer no microfone não pode
    /// roubar o foco de quem está jogando.
    ToggleMic,
    /// Traz para a frente ANTES de avisar: o usuário vai digitar, então a janela precisa do foco.
    ToggleInput,
    /// Alterna visibilidade. Não manda evento nenhum — é ação de janela, não do renderer.
    ToggleVisibility,
}

/// Os três atalhos que o shell registra no BOOT, como `main-window-shortcuts.ts` faz no Electron.
const APP_SHORTCUTS: [(&str, AppShortcut); 3] = [
    ("F9", AppShortcut::ToggleMic),
    ("CommandOrControl+Space", AppShortcut::ToggleInput),
    ("CommandOrControl+Shift+M", AppShortcut::ToggleVisibility),
];

/// Registra os atalhos do app no boot: os três fixos de [`APP_SHORTCUTS`], mais o mudo de fala
/// ([`register_mute_shortcut`]) e a captura de tela ([`register_vision_screenshot_shortcut`]) — os
/// outros dois atalhos fixos que o TS também registrava sem rota de rebind.
///
/// **Isto não existia no shell Tauri.** Havia só a rota sob demanda (`register`/`unregister`, que o
/// backend Node chamava); ninguém registrava os fixos, então nenhum deles fazia nada no app
/// portado — sem erro, sem log, sem teste vermelho, porque o que falta é uma CHAMADA, não um
/// comportamento errado. A rota sob demanda saiu em 2026-09-22 junto com o sidecar: sem backend
/// Node e sem UI que remapeie tecla, ela não tinha mais quem a chamasse. Rebind pelo usuário
/// continua sem chamador (UI ou settings key) — investigado e registrado em
/// `docs/tauri-rust-migration.md` ("Rebind de atalho pelo usuário").
///
/// Atalho recusado pelo SO não derruba o boot: o Electron mostra um aviso e segue, e recusa é
/// esperada (outro app já dono da combinação). A divergência é que aqui ela vira log, não
/// notificação — `shortcutRejectedNotice` depende do locale do usuário, que o shell não resolve.
pub fn register_app_shortcuts(app: &AppHandle) {
    for (accelerator, action) in APP_SHORTCUTS {
        let handle = app.clone();

        let result = app.global_shortcut().on_shortcut(accelerator, move |_app, _shortcut, event| {
            // O SO emite Pressed E Released; sem este filtro cada aperto dispara a ação duas vezes.
            if event.state() != ShortcutState::Pressed {
                return;
            }
            run(&handle, action);
        });

        if let Err(error) = result {
            log::warn!("atalho {accelerator} recusado pelo sistema: {error}");
        }
    }

    register_mute_shortcut(app);
    register_vision_screenshot_shortcut(app);
    register_push_to_talk_shortcut(app);
}

/// F8 segurado = microfone aberto; soltou, fecha — o mesmo caminho de segurar o botão de mic do dock
/// (`onSetMic(true/false)` no renderer), só que sem precisar do foco na janela. Fica ao lado do F9
/// (alternar) e, como ele, NÃO traz a janela para a frente.
const PUSH_TO_TALK_ACCELERATOR: &str = "F8";

/// Tecla segurada agora? O SO repete `Pressed` enquanto a tecla está baixa (auto-repeat); só as
/// bordas viram evento.
static PUSH_TO_TALK_HELD: AtomicBool = AtomicBool::new(false);

/// Borda que o renderer precisa ver, ou `None` para auto-repeat / soltar sem ter apertado.
fn push_to_talk_edge(held: bool, state: ShortcutState) -> Option<bool> {
    let pressed = state == ShortcutState::Pressed;
    (pressed != held).then_some(pressed)
}

fn register_push_to_talk_shortcut(app: &AppHandle) {
    use tauri::{Emitter, Manager};

    let handle = app.clone();
    let result = app.global_shortcut().on_shortcut(PUSH_TO_TALK_ACCELERATOR, move |_app, _shortcut, event| {
        let Some(pressed) = push_to_talk_edge(PUSH_TO_TALK_HELD.load(Ordering::SeqCst), event.state()) else {
            return;
        };
        PUSH_TO_TALK_HELD.store(pressed, Ordering::SeqCst);
        if let Some(window) = handle.get_webview_window(crate::windows::MAIN) {
            let _ = window.emit(PUSH_TO_TALK, json!({ "pressed": pressed }));
        }
    });

    if let Err(error) = result {
        log::warn!("atalho {PUSH_TO_TALK_ACCELERATOR} (segurar para falar) recusado pelo sistema: {error}");
    }
}

/// Quarto atalho fixo do TS — `MUTE_ACCELERATOR` em `main/infrastructure/mute-shortcut.ts`. Fica
/// fora de [`APP_SHORTCUTS`] porque, ao contrário dos três acima, ele carrega estado (a janela de
/// mudo em curso) em vez de só emitir um canal fixo.
const MUTE_ACCELERATOR: &str = "CommandOrControl+Shift+S";

/// Espelha `MUTE_WINDOW_MS` do TS: 15 minutos de mudo por aperto.
///
/// `pub(crate)`: `voice::silence_command` reusa a MESMA janela para o comando de voz equivalente
/// (`"cala a boca"`) — um comando falado não é mais nem menos definitivo que o atalho de teclado.
pub(crate) const MUTE_WINDOW_MS: i64 = 15 * 60_000;

/// Instante (epoch ms) até quando a fala está muda; `0` = não muda agora. Um único processo, então
/// um estático basta — mesmo padrão de `now_ms()` privado por módulo já usado no resto do crate.
fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Porte de `nextMutePayload` (`main/infrastructure/mute-shortcut.ts`): alterna — se já está mudo,
/// desmuda agora; senão muda pelos próximos [`MUTE_WINDOW_MS`]. `currently_muted` vem do MESMO
/// estado que a TTS respeita (`speech_mute`): um estado próprio aqui inverteria o atalho sempre que
/// o mudo viesse de outro lugar (a tool `voice mute`).
fn next_mute_payload(currently_muted: bool, now: i64) -> serde_json::Value {
    if currently_muted {
        return json!({ "muted": false });
    }
    json!({ "muted": true, "untilMs": now + MUTE_WINDOW_MS })
}

/// Ctrl+Shift+S alterna o mudo da fala. Porte de `registerMuteShortcut`: o TS publicava
/// `Events.MUTE_SPEECH` no barramento e deixava quem escuta (TTS, avatar) reagir — aqui é o mesmo
/// `EventKey::MuteSpeech` que `voice::tts::bus_bridge` já consome via `speech_mute::apply`, então
/// publicar é o bastante; nenhuma chamada direta a `modules/voice` (fora do meu escopo).
fn register_mute_shortcut(app: &AppHandle) {
    let result = app.global_shortcut().on_shortcut(MUTE_ACCELERATOR, move |_app, _shortcut, event| {
        if event.state() != ShortcutState::Pressed {
            return;
        }
        let now = now_ms();
        let payload = next_mute_payload(crate::modules::voice::tts::speech_mute::is_muted(now), now);
        crate::events::bus().publish(crate::events::EventKey::MuteSpeech, payload);
    });

    if let Err(error) = result {
        // Mesma divergência já documentada acima para os 3 atalhos fixos: o TS também avisava o
        // usuário por `Events.NOTIFICATION` (com locale); aqui vira só log, pelo mesmo motivo.
        // Tier humano: efeito explícito — o atalho não vai funcionar, não só "recusado".
        crate::human_log!(
            log::Level::Warn,
            "shortcuts: Ctrl+Shift+S de mudo não vai funcionar porque o sistema recusou o registro ({error})"
        );
    }
}

/// Ctrl+Shift+T captura a tela e manda analisar. Porte de `handleScreenshotShortcut`
/// (`main/services/vision/handlers/vision-proactive.ts`), despachando pela API pública do módulo
/// `vision` (`crate::modules::tools::registry().execute("vision_capture", ...)`) em vez de chamar
/// `modules/vision` direto — fora do meu escopo editar aquele módulo.
///
/// Desde 2026-09-22, com a análise pronta, publica `EventKey::BrainProactiveThought` — o MESMO
/// evento que o TS emitia (`Events.BRAIN_PROACTIVE_THOUGHT`), consumido por
/// `modules::brain::thought::spawn_thought_service`, que entrega ao motor proativo
/// (`ProactiveEngine::request`). Antes desta chave existir, este atalho publicava
/// `EventKey::Notification` como análogo mais próximo (aviso imediato ao HUD, sem passar pela
/// cadência do motor proativo) — divergência agora fechada.
fn register_vision_screenshot_shortcut(app: &AppHandle) {
    let result = app.global_shortcut().on_shortcut(VISION_SCREENSHOT_ACCELERATOR, move |_app, _shortcut, event| {
        if event.state() != ShortcutState::Pressed {
            return;
        }
        tauri::async_runtime::spawn(async move {
            dispatch_screenshot_shortcut().await;
        });
    });

    if let Err(error) = result {
        log::warn!("atalho de captura de tela recusado pelo sistema: {error}");
    }
}

const VISION_SCREENSHOT_ACCELERATOR: &str = "CommandOrControl+Shift+T";

async fn dispatch_screenshot_shortcut() {
    let result = crate::modules::tools::registry()
        .execute("vision_capture", json!({ "action": "screenshot" }))
        .await;

    match result {
        Ok(value) => {
            if value["success"].as_bool() != Some(true) {
                // Falha de negócio (visão desligada, captura vazia, provedor fora) — mesmo
                // tratamento silencioso do TS (`ctx.logger.warn`, sem propagar ao usuário).
                log::warn!("atalho de captura de tela: vision_capture sem sucesso: {value}");
                return;
            }
            let Some(analysis) = value["data"]["analysis"].as_str() else {
                return;
            };
            crate::events::bus().publish(
                crate::events::EventKey::BrainProactiveThought,
                crate::modules::vision::proactive_thought_payload_for_screenshot(analysis),
            );
        }
        Err(error) => log::warn!("atalho de captura de tela: vision_capture falhou: {error}"),
    }
}

fn run(app: &AppHandle, action: AppShortcut) {
    use tauri::{Emitter, Manager};

    let Some(window) = app.get_webview_window(crate::windows::MAIN) else {
        return;
    };

    let to_front = |window: &tauri::WebviewWindow| {
        let _ = window.show();
        let _ = window.set_focus();
    };

    match action {
        AppShortcut::ToggleMic => {
            let _ = window.emit(TOGGLE_MIC, ());
        }
        AppShortcut::ToggleInput => {
            to_front(&window);
            let _ = window.emit(TOGGLE_INPUT, ());
        }
        AppShortcut::ToggleVisibility => {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            } else {
                to_front(&window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// O atalho alterna a partir do estado que a TTS respeita: mudo vindo da tool também desmuda.
    #[test]
    fn the_mute_shortcut_toggles_from_the_real_speech_state() {
        use super::{next_mute_payload, MUTE_WINDOW_MS};
        use serde_json::json;
        assert_eq!(next_mute_payload(true, 1_000), json!({ "muted": false }));
        assert_eq!(next_mute_payload(false, 1_000), json!({ "muted": true, "untilMs": 1_000 + MUTE_WINDOW_MS }));
    }

    use super::{AppShortcut, APP_SHORTCUTS, MUTE_ACCELERATOR, TOGGLE_INPUT, TOGGLE_MIC, VISION_SCREENSHOT_ACCELERATOR};
    use super::{push_to_talk_edge, PUSH_TO_TALK, PUSH_TO_TALK_ACCELERATOR};
    use tauri_plugin_global_shortcut::ShortcutState;

    /// Auto-repeat do SO manda `Pressed` várias vezes com a tecla baixa: só a primeira abre o mic, e
    /// soltar fecha uma vez — sem isto o renderer recebia um `setMic(true)` a cada ~30 ms.
    #[test]
    fn push_to_talk_emits_only_the_press_and_release_edges() {
        assert_eq!(push_to_talk_edge(false, ShortcutState::Pressed), Some(true));
        assert_eq!(push_to_talk_edge(true, ShortcutState::Pressed), None);
        assert_eq!(push_to_talk_edge(true, ShortcutState::Released), Some(false));
        assert_eq!(push_to_talk_edge(false, ShortcutState::Released), None);
    }

    /// O canal tem de existir no contrato que a ponte escuta, ou o F8 emite para ninguém.
    #[test]
    fn push_to_talk_channel_is_in_the_desktop_ipc_contract() {
        let contract = include_str!("../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(contract.contains(&format!("ShortcutPushToTalk: '{PUSH_TO_TALK}'")), "canal {PUSH_TO_TALK} fora de desktop-ipc-channels.ts");
    }

    /// O card "Atalhos" (e o wizard) listam `GLOBAL_SHORTCUTS` de `app-shortcuts.ts`: a lista tem de
    /// ser EXATAMENTE o que este arquivo registra no SO — nem tecla anunciada que não funciona, nem
    /// atalho real escondido do usuário.
    #[test]
    fn the_shortcuts_card_lists_exactly_the_registered_global_shortcuts() {
        let catalog = include_str!("../../src/renderer/src/lib/app-shortcuts.ts");
        let block = catalog
            .split("export const GLOBAL_SHORTCUTS")
            .nth(1)
            .and_then(|rest| rest.split("export const WINDOW_SHORTCUTS").next()).expect("bloco GLOBAL_SHORTCUTS");
        let mut listed: Vec<&str> = block.split("accelerator: '").skip(1).filter_map(|rest| rest.split('\'').next()).collect();
        let mut registered: Vec<&str> = APP_SHORTCUTS.iter().map(|(accelerator, _)| *accelerator).collect();
        registered.extend([MUTE_ACCELERATOR, VISION_SCREENSHOT_ACCELERATOR, PUSH_TO_TALK_ACCELERATOR]);
        listed.sort_unstable();
        registered.sort_unstable();
        assert_eq!(listed, registered);
    }

    // `mute-shortcut.ts`, `vision/index.ts` e `shell/app-shortcuts.ts` foram removidos (shell
    // Electron deletado na migração para Tauri) — o Rust é agora o único shell, então os
    // aceleradores/canais que essas leituras pinavam viram literais congelados, extraídos do fonte
    // no momento da deleção. Divergir de um literal continua registrando uma tecla que o usuário
    // nunca apertou, ou emitindo para um ouvinte que não existe — só que sem um segundo lado TS
    // para comparar.

    #[test]
    fn mute_accelerator_matches_the_frozen_electron_shell_value() {
        assert_eq!(MUTE_ACCELERATOR, "CommandOrControl+Shift+S");
    }

    #[test]
    fn vision_screenshot_accelerator_matches_the_frozen_electron_shell_value() {
        assert_eq!(VISION_SCREENSHOT_ACCELERATOR, "CommandOrControl+Shift+T");
    }

    /// Os três aceleradores e os dois canais têm de ser os MESMOS do shell Electron (congelado
    /// abaixo, já que o shell foi deletado).
    #[test]
    fn app_shortcuts_match_the_frozen_electron_shell_values() {
        let accelerators: Vec<&str> = APP_SHORTCUTS.iter().map(|(accelerator, _)| *accelerator).collect();
        assert_eq!(accelerators, vec!["F9", "CommandOrControl+Space", "CommandOrControl+Shift+M"]);

        assert_eq!(TOGGLE_MIC, "shortcut:toggle-mic");
        assert_eq!(TOGGLE_INPUT, "shortcut:toggle-input");
    }

    /// F9 avisa o renderer sem trazer a janela para a frente; Ctrl+Space traz. Essa diferença é o
    /// ponto do F9 — mexer no microfone no meio de um jogo não pode roubar o foco. O corpo dos
    /// dois braços é exercido direto (não por leitura de fonte), já que o TS que documentava a
    /// assimetria foi removido.
    #[test]
    fn only_the_input_shortcut_brings_the_window_to_front() {
        let mic = APP_SHORTCUTS.iter().find(|(accelerator, _)| *accelerator == "F9").expect("F9 sumiu de APP_SHORTCUTS");
        assert!(matches!(mic.1, AppShortcut::ToggleMic), "F9 não aponta mais para ToggleMic — releia a assimetria com Ctrl+Space");

        let input = APP_SHORTCUTS
            .iter()
            .find(|(accelerator, _)| *accelerator == "CommandOrControl+Space")
            .expect("Ctrl+Space sumiu de APP_SHORTCUTS");
        assert!(matches!(input.1, AppShortcut::ToggleInput), "Ctrl+Space não aponta mais para ToggleInput");
    }
}
