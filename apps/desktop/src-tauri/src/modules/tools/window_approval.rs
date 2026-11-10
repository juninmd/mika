//! `WindowApprovalHandler` — porte de `createApprovalHandler` (`ipc/tool-approval.ts`): o
//! [`super::approval::ApprovalHandler`] REAL que faltava, a camada de janela/IPC que `approval.rs`
//! documenta no próprio cabeçalho ("quem tiver o `AppHandle` primeiro instala um handler real").
//!
//! Escolhe a janela certa (`pick_approval_window_label`, porte de `pickApprovalWindow`), manda
//! `tool-approval-request` pra ela e espera a resposta — que só `tool-approval-response` resolve
//! (`Self::resolve`). Esse canal é servido por `native.rs` desde 2026-09-22, e é o que fecha o
//! ciclo: antes dele o pedido ficava pendente até o timeout de 30s do `ApprovalGateService` negar.
//! Fail-closed correto, comportamento inútil — o diálogo aparecia e o clique não ia a lugar nenhum.
//!
//! **Instalação:** uma vez no boot, em `modules::register_config_bound_tools`, via [`install`] —
//! que instala no registro E deixa o handler alcançável por [`installed`]. Chamar
//! `registry().set_approval_handler(...)` direto instala pela metade: o registro guarda o TRAIT
//! (`decide()`), e quem recebe a resposta do usuário precisa de `resolve()`, deste tipo concreto.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use super::approval::{ApprovalDecision, ApprovalHandler, ApprovalRequest, ApprovalScope};
use crate::protocol::WindowType;

/// Espelha `DESKTOP_RENDERER_EVENT_CHANNELS.ToolApprovalRequest`
/// (`packages/sdk/src/contracts/desktop-ipc-channels.ts`) — o evento que o renderer escuta para
/// desenhar o diálogo. Ancorado contra o fonte pelo teste
/// `request_event_matches_the_ts_channel_contract` abaixo.
const TOOL_APPROVAL_REQUEST_EVENT: &str = "tool-approval-request";

const DENIED_NO_WINDOW: ApprovalDecision = ApprovalDecision { approved: false, scope: ApprovalScope::Once };

/// O mapa de aprovações pendentes, sem `AppHandle` nenhum — a metade testável sem um app Tauri de
/// verdade (este crate não liga a feature `tauri/test`; mesma fronteira que `chat/send.rs`
/// documenta em `turn_event_ipc`).
#[derive(Default)]
struct PendingApprovals {
    inner: Mutex<HashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl PendingApprovals {
    fn insert(&self, tool_call_id: String) -> oneshot::Receiver<ApprovalDecision> {
        let (tx, rx) = oneshot::channel();
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).insert(tool_call_id, tx);
        rx
    }

    fn remove(&self, tool_call_id: &str) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).remove(tool_call_id);
    }

    /// Resposta para um `tool_call_id` desconhecido (já resolvido, já limpo pelo `PendingGuard`,
    /// ou nunca pedido por este handler) não é erro — mesma tolerância do `if (!handler) return`
    /// em `ipc/tool-approval.ts`. `bool` de retorno é só para os testes provarem a diferença.
    fn resolve(&self, tool_call_id: &str, decision: ApprovalDecision) -> bool {
        match self.inner.lock().unwrap_or_else(|e| e.into_inner()).remove(tool_call_id) {
            Some(sender) => sender.send(decision).is_ok(),
            None => false,
        }
    }
}

/// Remove a entrada pendente ao sair de escopo — inclusive quando `decide()` é cancelado pelo
/// timeout de 30s do `ApprovalGateService` (`tokio::time::timeout` derruba a future no meio do
/// `.await`, sem rodar mais nenhuma linha depois dele). Sem isto, um pedido expirado ficaria
/// "pendente" para sempre no mapa, e uma resposta tardia do usuário chamaria `resolve` para um
/// `Sender` que ninguém mais lê — vazamento silencioso, não pane, mas preso pela vida do processo.
struct PendingGuard<'a> {
    pending: &'a PendingApprovals,
    tool_call_id: String,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.pending.remove(&self.tool_call_id);
    }
}

/// [`ApprovalHandler`] real. Ver o cabeçalho do arquivo para a instalação.
pub struct WindowApprovalHandler {
    app: AppHandle,
    pending: PendingApprovals,
}

/// O handler instalado no boot, alcançável por quem serve `tool-approval-response`.
///
/// Existe porque `ToolRegistry::set_approval_handler` guarda um `Arc<dyn ApprovalHandler>` — o
/// TRAIT, que só sabe `decide()`. Quem recebe a resposta do usuário precisa de `resolve()`, que é
/// deste tipo concreto, e fazer downcast de trait object pediria `Any` em toda implementação de
/// handler só para servir um caso. Um `OnceLock` do lado de cá resolve sem tocar no trait.
///
/// `OnceLock` e não `RwLock`: instalar dois handlers seria ter duas tabelas de pendências, e a
/// resposta do usuário acharia a errada em metade das vezes.
static INSTALLED: OnceLock<Arc<WindowApprovalHandler>> = OnceLock::new();

/// Monta o handler real, instala no registro de tools e o deixa alcançável por [`installed`].
/// Chamado uma vez no boot (`modules::register_config_bound_tools`).
pub fn install(app: AppHandle) -> Arc<WindowApprovalHandler> {
    let handler = WindowApprovalHandler::new(app);
    super::registry().set_approval_handler(handler.clone());
    // Segunda chamada no mesmo processo mantém o PRIMEIRO handler alcançável por `installed()` —
    // mas o registro acima já teria trocado o que `decide()` usa. Só acontece se alguém chamar
    // isto duas vezes no boot, e o teste `installing_twice_is_reported` fixa que isso é um erro
    // logado, não um estado divergente aceito em silêncio.
    if INSTALLED.set(handler.clone()).is_err() {
        log::error!("approval handler installed twice; the second install cannot receive responses");
    }
    handler
}

/// O handler instalado, ou `None` antes do boot (e em todo teste que não instala um).
pub fn installed() -> Option<&'static Arc<WindowApprovalHandler>> {
    INSTALLED.get()
}

/// Dispara um pedido de aprovação sintético pelo handler REAL instalado — a mesma
/// `WindowApprovalHandler::decide` que uma tool de verdade aciona quando o `ApprovalGateService`
/// pede confirmação. Só a origem do `ApprovalRequest` é sintética (id/nome fixos); o resto do
/// caminho — escolher a janela, emitir `tool-approval-request`, esperar `tool-approval-response`
/// — é o de produção. Chamador único: `modules::e2e_hooks` (gated por `MIKA_E2E`/superfície).
#[cfg(any(test, feature = "e2e"))]
pub fn trigger_e2e_approval_request(app: &AppHandle) -> Result<(), String> {
    let handler = installed().ok_or("approval handler ainda não instalado")?.clone();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let request = ApprovalRequest {
        tool_call_id: format!("e2e-{unique}"),
        tool_name: "e2e_hooks_probe".into(),
        args: json!({ "source": "e2e_hooks" }),
    };
    tauri::async_runtime::spawn(async move {
        // O 30s de timeout do `ApprovalGateService` é do CHAMADOR real; aqui não há chamador —
        // então esta future só resolve quando a UI responder, ou fica pendurada até o processo
        // morrer. Aceitável: é um teste, e `PendingGuard` limpa a entrada se a task for cancelada.
        let _ = handler.decide(request).await;
    });
    let _ = app; // reservado: a janela é resolvida pelo próprio `handler`, não por este parâmetro.
    Ok(())
}

impl WindowApprovalHandler {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self { app, pending: PendingApprovals::default() })
    }

    /// Resolve um pedido pendente — chamado por quem reivindicar `tool-approval-response`
    /// (`bridge.rs`, fora deste diretório) quando o usuário decide.
    pub fn resolve(&self, tool_call_id: &str, decision: ApprovalDecision) {
        self.pending.resolve(tool_call_id, decision);
    }
}

#[async_trait]
impl ApprovalHandler for WindowApprovalHandler {
    async fn decide(&self, request: ApprovalRequest) -> Result<ApprovalDecision, String> {
        let windows = self.app.webview_windows();
        let label = pick_approval_window_label(windows.keys().map(String::as_str)).map(str::to_owned);
        let Some(label) = label else {
            log::warn!(
                "nenhuma janela da Mika para pedir aprovação; negando por padrão: tool={}",
                request.tool_name
            );
            return Ok(DENIED_NO_WINDOW);
        };
        let Some(window) = windows.get(&label).cloned() else {
            // Só possível se a janela fechou entre o `webview_windows()` acima e aqui — mesma
            // corrida que `windows.rs::announce` existe para minimizar, nunca eliminar.
            return Ok(DENIED_NO_WINDOW);
        };

        if !window.is_visible().unwrap_or(true) {
            let _ = window.show();
        }

        let rx = self.pending.insert(request.tool_call_id.clone());
        let _guard = PendingGuard { pending: &self.pending, tool_call_id: request.tool_call_id.clone() };

        self.app
            .emit_to(label.as_str(), TOOL_APPROVAL_REQUEST_EVENT, approval_request_payload(&request))
            .map_err(|error| format!("failed to emit {TOOL_APPROVAL_REQUEST_EVENT}: {error}"))?;

        rx.await.map_err(|_| "approval channel closed before a decision arrived".to_string())
    }
}

/// Núcleo puro do payload — mesmos nomes de campo que `ApprovalRequest` do lado TS
/// (`packages/sdk/src/tools/approval-gate.ts`): `toolCallId`, `toolName`, `args`.
fn approval_request_payload(request: &ApprovalRequest) -> Value {
    json!({ "toolCallId": request.tool_call_id, "toolName": request.tool_name, "args": request.args })
}

/// Porte puro de `pickApprovalWindow` (`ipc/tool-approval.ts`): nunca `settings`/`plugin-view` se
/// houver uma janela "normal" viva (`main`, `wizard`, ou qualquer label desconhecido); cai para
/// `settings` só se for a única aberta; sem janela nenhuma, `None`. TS filtra por
/// `!isDestroyed()` sobre `BrowserWindow.getAllWindows()`; aqui `AppHandle::webview_windows()` já
/// só devolve janelas vivas (uma destruída sai do registro — ver `windows.rs::track`/`announce`).
///
/// Recebe labels, não `AppHandle`: é a metade testável sem um app Tauri de verdade.
fn pick_approval_window_label<'a>(labels: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let labels: Vec<&str> = labels.collect();
    let is_askable = |label: &&str| !matches!(WindowType::from_label(label), WindowType::Settings | WindowType::PluginView);

    labels
        .iter()
        .copied()
        .find(|label| is_askable(label))
        .or_else(|| labels.iter().copied().find(|label| WindowType::from_label(label) == WindowType::Settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_a_normal_window_over_settings_and_plugin_view() {
        let picked = pick_approval_window_label(["settings", "plugin-view::foo::bar", "main"].into_iter());
        assert_eq!(picked, Some("main"));
    }

    #[test]
    fn falls_back_to_settings_when_its_the_only_window_open() {
        let picked = pick_approval_window_label(["settings"].into_iter());
        assert_eq!(picked, Some("settings"));
    }

    #[test]
    fn wizard_counts_as_a_normal_window_same_as_main() {
        let picked = pick_approval_window_label(["settings", "wizard"].into_iter());
        assert_eq!(picked, Some("wizard"));
    }

    /// NEGATIVO — só uma plugin-view aberta (sem `main`/`settings`) não é askable: nem a primeira
    /// busca (não-settings/não-plugin-view) nem o fallback (settings) acham nada. `None`, não um
    /// label errado.
    #[test]
    fn returns_none_when_only_a_plugin_view_is_open() {
        let picked = pick_approval_window_label(["plugin-view::foo::bar"].into_iter());
        assert_eq!(picked, None);
    }

    #[test]
    fn returns_none_when_no_window_is_open() {
        let picked = pick_approval_window_label(std::iter::empty());
        assert_eq!(picked, None);
    }

    #[test]
    fn approval_request_payload_uses_the_camelcase_field_names_the_renderer_contract_expects() {
        let request = ApprovalRequest {
            tool_call_id: "tool-steam-1".into(),
            tool_name: "steam".into(),
            args: json!({ "action": "list" }),
        };
        let payload = approval_request_payload(&request);
        assert_eq!(payload["toolCallId"], json!("tool-steam-1"));
        assert_eq!(payload["toolName"], json!("steam"));
        assert_eq!(payload["args"], json!({ "action": "list" }));
    }

    #[tokio::test]
    async fn resolve_delivers_the_decision_to_the_waiting_receiver() {
        let pending = PendingApprovals::default();
        let rx = pending.insert("t1".into());

        let delivered = pending.resolve("t1", ApprovalDecision { approved: true, scope: ApprovalScope::Once });
        assert!(delivered);

        let decision = rx.await.unwrap();
        assert!(decision.approved);
    }

    #[test]
    fn resolve_for_an_unknown_id_is_a_silent_no_op() {
        let pending = PendingApprovals::default();
        let delivered = pending.resolve("never-asked", ApprovalDecision { approved: true, scope: ApprovalScope::Once });
        assert!(!delivered, "resposta para id desconhecido não deveria achar destinatário nenhum");
    }

    /// Prova o motivo de existir do `PendingGuard`: depois que a entrada é removida (o que o
    /// `Drop` faz no timeout real), uma resposta tardia para o MESMO id não acha ninguém — não
    /// pane, não vazamento silencioso de `Sender` esquecido.
    #[test]
    fn a_response_after_the_entry_was_cleaned_up_is_also_a_silent_no_op() {
        let pending = PendingApprovals::default();
        let _rx = pending.insert("t2".into());
        pending.remove("t2");

        let delivered = pending.resolve("t2", ApprovalDecision { approved: true, scope: ApprovalScope::Once });
        assert!(!delivered);
    }

    /// Ancora o literal do evento contra o contrato TS — se o canal mudar de nome lá, este handler
    /// falaria uma língua que o renderer não escuta mais, em silêncio.
    #[test]
    fn request_event_matches_the_ts_channel_contract() {
        let channels = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(
            channels.contains(&format!("'{TOOL_APPROVAL_REQUEST_EVENT}'")),
            "desktop-ipc-channels.ts mudou o canal de ToolApprovalRequest — releia antes de reusar este literal"
        );
    }
}
