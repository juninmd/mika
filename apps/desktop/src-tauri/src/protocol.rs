//! Tipos do contrato de IPC: tipo de janela, contexto de invocação e o roteamento de evento
//! para o renderer. Era também o contrato do sidecar Node (JSON por linha sobre stdio); as
//! mensagens `ToSidecar`/`FromSidecar` saíram em 2026-09-22 junto com o processo.
//!
//! O `InvokeContext` é montado AQUI, a partir do label da janela que o Tauri conhece.
//! No Electron esse contexto era farejado da URL do `webContents` (`detectWindowType`),
//! o que um renderer comprometido podia forjar; aqui o renderer não tem como escrever
//! nesse campo — ele nem trafega na chamada vinda do webview.

use serde::{Deserialize, Serialize};

/// Espelha `DesktopInvokeContext['windowType']` em packages/sdk/src/contracts/desktop-ipc.ts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowType {
    Main,
    Settings,
    Wizard,
    PluginView,
    Unknown,
}

impl WindowType {
    /// Label da janela é a fonte da verdade. Um label desconhecido cai em `Unknown`,
    /// que `assertOwnerSurface` recusa — negar por omissão, não permitir por omissão.
    pub fn from_label(label: &str) -> Self {
        match label.split("::").next().unwrap_or(label) {
            "main" => Self::Main,
            "settings" => Self::Settings,
            "wizard" => Self::Wizard,
            "plugin-view" => Self::PluginView,
            _ => Self::Unknown,
        }
    }
}

/// Para quem vai um evento de renderer que NÃO está em `ALL_WINDOW_EVENTS` (o TS
/// `sendRendererEvent`, em `main/infrastructure/renderer-event-bridge.ts`): a janela `main` quando
/// ela existe; senão, toda janela que não seja `plugin-view`.
///
/// **Mora aqui, e não em cada ponte, porque isto é regra de SUPERFÍCIE, não de módulo.** É a mesma
/// fronteira de confiança de [`WindowType::from_label`] logo acima: `plugin-view` é a superfície
/// que `is_owner_surface` barra em 10 rotas de IPC, e o que trafega por estes eventos é conversa do
/// usuário — a fala da Mika (`chat:assistant-message`) e a pose do avatar (`avatar:expression`).
/// Usar `app.emit`, que alcança TODA janela viva, alargaria a superfície por acidente de API: sem
/// rota nova, sem nada falhando.
///
/// Nasceu duplicada em `chat::assistant_bridge` e `features::character::expression_bridge`, no
/// mesmo dia, cada uma com seus próprios testes. Duas cópias de uma regra de confiança não erram
/// juntas — erram uma de cada vez, e a que ninguém revisou é a que vaza. A terceira ponte copiaria
/// de qual das duas?
///
/// Fail-closed: uma `plugin-view` sozinha não vira alvo por falta de alternativa. Sem janela dona
/// viva o evento simplesmente não sai, mesma política de [`WindowType::Unknown`].
pub fn renderer_event_targets(labels: Vec<String>) -> Vec<String> {
    if let Some(main) = labels.iter().find(|label| WindowType::from_label(label) == WindowType::Main) {
        return vec![main.clone()];
    }

    labels.into_iter().filter(|label| WindowType::from_label(label) != WindowType::PluginView).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeContext {
    #[serde(rename = "windowType")]
    pub window_type: WindowType,
    #[serde(rename = "windowLabel")]
    pub window_label: String,
    #[serde(rename = "pluginId")]
    pub plugin_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// Caminho normal: com a `main` viva, ela é o alvo ÚNICO — nem `settings` recebe, igual ao TS.
    #[test]
    fn the_main_window_is_the_only_target_when_it_exists() {
        assert_eq!(renderer_event_targets(labels(&["settings", "main", "plugin-view::mika.gamer"])), vec!["main".to_string()]);
    }

    /// Fallback do TS: sem `main`, vai para as outras — MENOS `plugin-view`. Esta é a asserção que
    /// justifica as pontes não usarem `app.emit`.
    #[test]
    fn the_fallback_reaches_every_window_except_a_plugin_view() {
        let targets = renderer_event_targets(labels(&["settings", "plugin-view::mika.gamer", "wizard"]));
        assert_eq!(targets, vec!["settings".to_string(), "wizard".to_string()]);
    }

    /// Fail-closed: uma `plugin-view` sozinha não vira alvo por falta de alternativa.
    #[test]
    fn a_lone_plugin_view_receives_nothing() {
        assert!(renderer_event_targets(labels(&["plugin-view::mika.gamer"])).is_empty());
    }

    /// O label carrega o id do plugin depois de `::`, e `from_label` corta no separador. Sem esse
    /// corte a view passaria pelo filtro por não bater com o literal "plugin-view".
    #[test]
    fn a_suffixed_plugin_view_label_is_still_recognised_as_a_plugin_view() {
        assert_eq!(WindowType::from_label("plugin-view::mika.person-catalog"), WindowType::PluginView);
        assert!(renderer_event_targets(labels(&["plugin-view::mika.person-catalog"])).is_empty());
    }

    /// Um label desconhecido NÃO é plugin-view, então recebe no fallback — o mesmo `Unknown` que
    /// `assertOwnerSurface` recusa em rota de IPC é aceito aqui de propósito: o TS filtra por
    /// `plugin-view`, e divergir disso deixaria uma janela legítima muda.
    #[test]
    fn an_unknown_label_still_receives_in_the_fallback_like_the_ts_does() {
        assert_eq!(renderer_event_targets(labels(&["overlay-hud"])), vec!["overlay-hud".to_string()]);
    }
}
