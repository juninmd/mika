//! Os quatro canais de tool, servidos em Rust — porte de `main/ipc/tools.ts` e do lado de recepção
//! de `main/ipc/tool-approval.ts`.
//!
//! `mika:tools-list`, `mika:tools-list-by-plugin`, `mika:tool-execute` e `tool-approval-response`.
//! Os quatro juntos, nunca em partes, e o motivo está no cabeçalho de `mod.rs`: `bridge.rs` roteia
//! por canal INTEIRO, então reivindicar só `tool-approval-response` interceptaria a resposta de
//! toda aprovação que o sidecar estivesse esperando — o usuário clica, o evento nunca chega ao
//! `Map` do Node, e a chamada fica pendurada até o backstop. Pior do que não migrar.
//!
//! ## A ordem das guardas, que é a segurança inteira deste arquivo
//!
//! `execute` faz, nesta ordem e sem atalho:
//! 1. **Superfície e identidade** (`access::assert_tool_execution_allowed`) — antes de tocar no
//!    registro, antes de ler argumento. Uma `plugin-view` sem identidade é recusada aqui.
//! 2. **Aprovação por risco da CHAMADA** (`ensure_broker_approval`) — nome + argumentos, não só o
//!    nome: `desktop:window` e `desktop:launch` são a mesma tool e decisões diferentes.
//! 3. **`ToolRegistry::execute`**, que ainda valida os argumentos contra o schema e aplica o gate
//!    de tool marcada `destructive`.
//!
//! O passo 2 PULA a tool já marcada `destructive` — não por economia, mas porque o passo 3 vai
//! gatear a mesma chamada, e dois diálogos para uma decisão só significam que o segundo chega
//! depois de o primeiro já ter liberado. Mesma condição do `toolDef?.destructive` no TS.
//!
//! ## O que diverge do TypeScript, nomeado
//!
//! O TS publica `Events.TOOL_CALL`/`TOOL_RESULT` no EventBus interno; aqui só os eventos de HUD
//! vão para a janela que pediu. Os dois assinantes reais daquele barramento —
//! `person-catalog-lifecycle.ts` (marca "usei tool de pessoa" para o cooldown de menção) e
//! `voice-auto-speak-events.ts` (`onToolCall`, para não falar por cima de uma tool em curso) — não
//! têm porte em Rust, e `events/keys.rs` só aceita chave com consumidor real. Declarar as duas
//! agora seria enum morto disfarçado de pronto; a consequência de não declarar está escrita aqui
//! para quem portar aqueles dois saber que tem de voltar neste arquivo.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::access;
use super::approval::{ApprovalDecision, ApprovalScope};
use super::catalog::catalog_entry;
use super::registry::{registry, ToolOrigin};
use crate::native::NativeModule;
use crate::protocol::InvokeContext;

const CH_TOOLS_LIST: &str = "mika:tools-list";
const CH_TOOLS_LIST_BY_PLUGIN: &str = "mika:tools-list-by-plugin";
const CH_TOOL_EXECUTE: &str = "mika:tool-execute";
const CH_TOOL_APPROVAL_RESPONSE: &str = "tool-approval-response";

const CHANNELS: &[&str] = &[CH_TOOLS_LIST, CH_TOOLS_LIST_BY_PLUGIN, CH_TOOL_EXECUTE, CH_TOOL_APPROVAL_RESPONSE];

/// Os mesmos canais de HUD que `chat/send.rs` já emite para o turno de LLM — a UI de tools não tem
/// um par próprio, e inventar um deixaria a mesma informação em dois lugares divergentes.
const HUD_TOOL_CALL_EVENT: &str = "mika:event:chat:tool-call";
const HUD_TOOL_RESULT_EVENT: &str = "mika:event:chat:tool-result";

pub struct ToolsModule;

#[async_trait]
impl NativeModule for ToolsModule {
    fn name(&self) -> &'static str {
        "tools"
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
        match channel {
            CH_TOOLS_LIST => list(ctx, None),
            CH_TOOLS_LIST_BY_PLUGIN => {
                let plugin_id = payload.get("pluginId").and_then(Value::as_str).unwrap_or_default().trim();
                // `pluginId` vazio lista TUDO, igual ao `pluginId || undefined` do TS — e não é
                // buraco de permissão: o filtro é conveniência da UI, a guarda é `assert_*`.
                list(ctx, Some(plugin_id).filter(|id| !id.is_empty()))
            }
            CH_TOOL_EXECUTE => execute(app, ctx, payload).await,
            CH_TOOL_APPROVAL_RESPONSE => resolve_approval(ctx, &payload),
            other => Err(format!("canal de tools desconhecido: {other}")),
        }
    }
}

fn list(ctx: &InvokeContext, plugin_id: Option<&str>) -> Result<Value, String> {
    access::assert_tool_listing_allowed(ctx.window_type)?;

    let tools: Vec<Value> = registry()
        .get_all()
        .iter()
        .filter(|item| match plugin_id {
            Some(id) => item.plugin_id == id,
            None => true,
        })
        .map(catalog_entry)
        .collect();

    Ok(json!({ "tools": tools }))
}

/// Porte de `ensureBrokerApproval` — a política vive em `ToolRegistry::ensure_call_approval`
/// (`registry/broker.rs`), a MESMA que o loop de chat aplica à origem `Llm`; aqui só entra o
/// `ConfigStore` do app e a origem `User`. Config ilegível (store não abriu) cai para PERGUNTAR.
async fn ensure_broker_approval(app: &AppHandle, name: &str, args: &Value, call_id: &str) -> Option<Value> {
    let store = crate::state::config_store(app).ok();
    registry().ensure_call_approval(ToolOrigin::User, store.as_deref(), name, args, call_id).await
}

async fn execute(app: &AppHandle, ctx: &InvokeContext, payload: Value) -> Result<Value, String> {
    let name = payload.get("name").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let args = payload.get("args").cloned().unwrap_or_else(|| json!({}));
    let origin = hud_origin(&payload);

    access::assert_tool_execution_allowed(ctx.window_type, ctx.plugin_id.as_deref(), &name, registry().get(&name).as_ref())?;

    let call_id = format!("desktop-{}-{}", if name.is_empty() { "tool" } else { &name }, uuid::Uuid::new_v4());

    if let Some(denial) = ensure_broker_approval(app, &name, &args, &call_id).await {
        emit_hud(app, ctx, HUD_TOOL_RESULT_EVENT, json!({ "name": name, "success": false, "result": denial, "origin": origin }));
        return Ok(denial);
    }

    emit_hud(app, ctx, HUD_TOOL_CALL_EVENT, json!({ "name": name, "origin": origin }));

    // `execute` devolve `Err` para tool desconhecida e argumento fora do schema. O TS transforma a
    // exceção em `{ success: false, error }` e a devolve como RESULTADO — o renderer desenha o erro
    // na linha da tool em vez de estourar um rejeito de IPC sem contexto. Mesma forma aqui.
    let started = std::time::Instant::now();
    let outcome = registry().execute_as(ToolOrigin::User, &name, args).await;
    let duration_ms = started.elapsed().as_millis() as u64;
    let (result, ok) = match outcome {
        Ok(value) => {
            let ok = value.get("success").and_then(Value::as_bool) != Some(false);
            (value, ok)
        }
        Err(error) => (json!({ "success": false, "error": error }), false),
    };

    emit_hud(app, ctx, HUD_TOOL_RESULT_EVENT, json!({ "name": name, "success": ok, "result": result, "durationMs": duration_ms, "origin": origin }));
    Ok(result)
}

/// `origin` from the renderer (`ToolCallOrigin`): `background` (MoodHUD's poll) must not light the
/// thinking HUD. Anything else is a visible `turn`, like TS's default.
fn hud_origin(payload: &Value) -> &'static str {
    match payload.get("origin").and_then(Value::as_str) {
        Some("background") => "background",
        _ => "turn",
    }
}

/// Só para a janela que pediu — mesma escolha de `chat/send.rs`: uma `plugin-view` executando a
/// própria tool não pode acender o HUD da janela principal, e o avatar não precisa saber que a
/// tela de settings listou algo.
fn emit_hud(app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) {
    use tauri::Emitter;
    let _ = app.emit_to(ctx.window_label.as_str(), channel, payload);
}

/// `tool-approval-response`: a decisão do usuário chegando de volta.
///
/// `toolCallId` inexistente é NO-OP silencioso, não erro — mesma tolerância do `if (!handler)
/// return` no TS. A resposta pode chegar depois do timeout de 30s do gate, e transformar isso em
/// rejeição só encheria o console do renderer sem mudar nada.
///
/// **Só superfície do dono responde.** Uma `plugin-view` (código de terceiro) que adivinhasse/ouvisse
/// um `toolCallId` aprovaria `desktop:launch`/`clear_history` no lugar do usuário — a recusa vem
/// ANTES de ler o payload.
fn resolve_approval(ctx: &InvokeContext, payload: &Value) -> Result<Value, String> {
    if !crate::shell_actions::is_owner_surface(ctx.window_type) {
        log::warn!("tool-approval-response recusado: janela não é superfície do dono");
        return Err("Tool approval denied: caller window is not an owner surface".into());
    }
    let Some(tool_call_id) = payload.get("toolCallId").and_then(Value::as_str).filter(|id| !id.is_empty()) else {
        log::warn!("tool-approval-response without a toolCallId; ignoring");
        return Ok(Value::Null);
    };

    // Ausência de `approved` é NEGAÇÃO. Um payload malformado não pode virar "aprovado" — é o
    // único ramo deste arquivo onde um default errado executaria a ação em vez de recusá-la.
    let approved = payload.get("approved").and_then(Value::as_bool).unwrap_or(false);
    let scope = match payload.get("scope").and_then(Value::as_str) {
        Some("session") => ApprovalScope::Session,
        Some("always") => ApprovalScope::Always,
        _ => ApprovalScope::Once,
    };

    match super::window_approval::installed() {
        Some(handler) => handler.resolve(tool_call_id, ApprovalDecision { approved, scope }),
        // Sem handler instalado ninguém está esperando — o gate já negou por padrão.
        None => log::warn!("tool-approval-response with no approval handler installed; ignoring"),
    }

    Ok(Value::Null)
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
