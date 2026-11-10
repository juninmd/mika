//! O tooltip da bandeja acompanha o que a Mika está fazendo — porte de `TRAY_STATUS_BY_EVENT`
//! (`src/main/shell/tray-model.ts`) e do `bus.on` de `src/main/tray.ts`.
//!
//! No Electron a bandeja dizia "pensando…", "falando…" ou "erro" ao passar o mouse; em Tauri ela
//! ficou estática em "Mika" desde o primeiro dia, e nenhuma checagem notava — a auditoria de
//! assinaturas TS × Rust é que achou. É o único sinal de estado com a janela escondida.

use tauri::AppHandle;

use crate::events::{bus, EventKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayStatus {
    Idle,
    Thinking,
    Speaking,
    Error,
}

/// Espelha `TRAY_STATUS_BY_EVENT`; chave fora da tabela não mexe no estado.
fn status_for(key: EventKey) -> Option<TrayStatus> {
    match key {
        EventKey::ThinkingStart => Some(TrayStatus::Thinking),
        EventKey::ThinkingStop | EventKey::SpeechFinished => Some(TrayStatus::Idle),
        EventKey::SpeechStarted => Some(TrayStatus::Speaking),
        EventKey::ChatError => Some(TrayStatus::Error),
        _ => None,
    }
}

/// Espelha `tooltipForStatus`: ocioso é o nome, o resto vem traduzido.
fn tooltip_for(status: TrayStatus, locale: &str) -> String {
    let key = match status {
        TrayStatus::Idle => return "Mika".to_string(),
        TrayStatus::Thinking => "shell.tray.statusThinking",
        TrayStatus::Speaking => "shell.tray.statusSpeaking",
        TrayStatus::Error => "shell.tray.statusError",
    };
    crate::modules::settings::t(key, locale)
}

/// Assina o barramento e reescreve o tooltip a cada troca de estado. O locale é lido a cada
/// evento — trocar o idioma vale para o próximo tooltip, mesma regra de `tray::refresh_labels`.
pub fn spawn(app: AppHandle) {
    // Assina ANTES do spawn — `broadcast` não entrega retroativamente.
    let mut sub = bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let mut current = TrayStatus::Idle;
        while let Some(envelope) = sub.recv().await {
            let Some(next) = status_for(envelope.key) else { continue };
            // `ThinkingStop` chega depois de `ChatError` no mesmo turno: sem isto o erro piscaria
            // e sumiria antes de alguém ver. Só um turno novo tira a bandeja do erro.
            if current == TrayStatus::Error && next == TrayStatus::Idle {
                continue;
            }
            current = next;
            let locale = crate::state::config_store(&app)
                .ok()
                .map(|store| crate::config::locale::app_locale(&store))
                .unwrap_or_else(|| crate::config::locale::DEFAULT_LOCALE.to_string());
            crate::tray::set_tooltip(&app, &tooltip_for(current, &locale));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turn_and_the_voice_drive_the_status() {
        assert_eq!(status_for(EventKey::ThinkingStart), Some(TrayStatus::Thinking));
        assert_eq!(status_for(EventKey::SpeechStarted), Some(TrayStatus::Speaking));
        assert_eq!(status_for(EventKey::SpeechFinished), Some(TrayStatus::Idle));
        assert_eq!(status_for(EventKey::ChatError), Some(TrayStatus::Error));
        assert_eq!(status_for(EventKey::VadSample), None);
    }

    #[test]
    fn every_non_idle_status_is_translated() {
        assert_eq!(tooltip_for(TrayStatus::Idle, "pt-BR"), "Mika");
        for status in [TrayStatus::Thinking, TrayStatus::Speaking, TrayStatus::Error] {
            let text = tooltip_for(status, "en-US");
            assert!(!text.starts_with("shell.tray"), "chave de tradução vazou: {text}");
        }
    }
}
