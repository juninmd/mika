//! As tool calls de UMA rodada do turno, em ordem: executa cada uma pelo registro compartilhado,
//! alimenta o `failure_tracker` e devolve o que volta para o modelo (chamadas + resultados).

use std::collections::HashMap;
use std::time::Duration;

use rig_core::message::{AssistantContent, ToolCall, ToolResultContent, UserContent};
use serde_json::Value;

use super::{failure_tracker, llm_gate, TurnAbortSignal, TurnError, TurnEvent, TurnObserver};

/// Teto de UMA tool call: o mesmo de [`llm_gate::BACKGROUND_TIMEOUT`], porque tools como `content`
/// pedem texto ao LLM no meio do turno. Sem teto, uma tool travada (rede, processo filho, MCP) deixava
/// o turno — e o HUD "pensando" — presos para sempre.
pub(super) const TOOL_CALL_TIMEOUT: Duration = llm_gate::BACKGROUND_TIMEOUT;

pub(super) struct ToolStep {
    pub(super) calls: Vec<AssistantContent>,
    pub(super) results: Vec<UserContent>,
    pub(super) executed: Vec<String>,
}

/// `succeeded` guarda, por turno, o resultado de cada chamada idêntica que já deu certo: o modelo
/// chamando `launch` de novo abria duas calculadoras — a repetição responde com o resultado anterior
/// em vez de repetir o efeito colateral.
pub(super) async fn run(
    tool_calls: &[ToolCall],
    abort: &TurnAbortSignal,
    observer: &dyn TurnObserver,
    succeeded: &mut HashMap<String, Value>,
    limit: Duration,
) -> Result<ToolStep, TurnError> {
    let mut step = ToolStep { calls: Vec::new(), results: Vec::new(), executed: Vec::new() };
    for tool_call in tool_calls {
        step.calls.push(AssistantContent::ToolCall(tool_call.clone()));
        let name = tool_call.function.name.to_string();
        let action = tool_call.function.arguments.get("action").and_then(Value::as_str).map(str::to_owned);
        let arguments = tool_call.function.arguments_value();
        observer.emit(TurnEvent::ToolCallStarted { name: name.clone() });

        if abort.is_aborted() {
            observer.emit(TurnEvent::ThinkingStop);
            return Err(TurnError::Aborted);
        }

        let started = std::time::Instant::now();
        let call_key = format!("{name}\u{0}{arguments}");
        let outcome = match succeeded.get(&call_key) {
            Some(previous) => Ok(serde_json::json!({ "success": true, "alreadyDone": true, "previous": previous })),
            // Argumentos que não são JSON válido (resposta cortada pelo teto de tokens, por exemplo)
            // chegam aqui só como o PREFIXO que deu para ler: rodar a tool com isso executaria uma
            // ação pela metade. O modelo recebe o erro e pode refazer a chamada.
            None if tool_call.function.invalid_arguments.is_some() => Err("tool arguments were not valid JSON (the call was cut off); nothing was run".to_string()),
            None => match execute_observing_abort(&name, arguments, abort, limit).await {
                Some(outcome) => outcome,
                None => {
                    let result = serde_json::json!({ "success": false, "error": "cancelled" });
                    observer.emit(TurnEvent::ToolCallFinished { name, action, ok: false, result, duration_ms: started.elapsed().as_millis() as u64 });
                    observer.emit(TurnEvent::ThinkingStop);
                    return Err(TurnError::Aborted);
                }
            },
        };
        let duration_ms = started.elapsed().as_millis() as u64;
        let ok = crate::modules::tools::outcome::succeeded(&outcome);
        record_outcome(&name, ok, &outcome, call_key, succeeded);
        let (content, result) = match outcome {
            Ok(value) => (ToolResultContent::json(value.clone()), value),
            Err(err) => (ToolResultContent::text(format!("Tool execution failed: {err}")), serde_json::json!({ "success": false, "error": err })),
        };
        observer.emit(TurnEvent::ToolCallFinished { name: name.clone(), action, ok, result, duration_ms });
        step.results.push(UserContent::tool_result(tool_call.id.clone(), tool_call.function.name.clone(), vec![content]));
        step.executed.push(name);
    }
    Ok(step)
}

/// Executa a tool numa task própria e espera por ela, pelo abort e pelo teto. `None` = abortado.
///
/// O que o abort/teto cancelam é a ESPERA, e só uma chamada de efeito nulo é de fato interrompida
/// (`JoinHandle::abort`: ler/consultar é seguro de largar no meio). "Destrutiva" aqui é a tool
/// marcada `destructive` OU a chamada que o broker pergunta ao usuário (`is_llm_gated_call`:
/// `memory_console clear_episodic`, `habits clear`, `export_data`, `desktop launch`... — risco que
/// depende da AÇÃO, não do nome). Essa — já aprovada, ou ainda esperando a aprovação — roda até o fim na task solta: largá-la no
/// meio de um `delete`/`launch` deixaria o efeito pela metade, e o turno só deixa de esperar o
/// resultado (que o modelo não vai mais ler). Custo aceito: uma aprovação concedida DEPOIS do abort
/// ainda executa a ação que o próprio usuário aprovou.
async fn execute_observing_abort(name: &str, args: Value, abort: &TurnAbortSignal, limit: Duration) -> Option<Result<Value, String>> {
    let registry = crate::modules::tools::registry();
    let destructive = registry.must_finish_once_started(name, &args);
    let owned_name = name.to_string();
    let mut task = tauri::async_runtime::spawn(llm_gate::carry_turn_context(async move { registry.execute_as(crate::modules::tools::ToolOrigin::Llm, &owned_name, args).await }));
    let mut abort = abort.clone();
    let outcome = tokio::select! {
        joined = &mut task => return Some(joined.unwrap_or_else(|e| Err(format!("tool task failed: {e}")))),
        _ = abort.wait_aborted() => None,
        _ = tokio::time::sleep(limit) => Some(Err(format!("timeout: {name} did not finish within {}s", limit.as_secs()))),
    };
    if !destructive {
        task.abort();
    }
    outcome
}

/// Port de `recordOutcome` (`tool-call-tracker.ts`) — toda falha aqui conta como "strike" (sem
/// `errorCode` estruturado para separar soft failure, ver o cabeçalho de `failure_tracker.rs`),
/// menos a recusa do usuário no diálogo de aprovação: a tool nem rodou, não está quebrada. A
/// dica de `record_failure` só é logada, nunca reinjetada por chamada individual — só o bloco
/// agregado (`self_healing_context`) entra no prompt.
fn record_outcome(name: &str, ok: bool, outcome: &Result<Value, String>, call_key: String, succeeded: &mut HashMap<String, Value>) {
    if crate::modules::tools::outcome::is_user_denial(outcome) {
        return;
    }
    if ok {
        failure_tracker::failure_tracker().record_success(name);
        if let Ok(value) = outcome {
            succeeded.entry(call_key).or_insert_with(|| value.clone());
        }
    } else if let Some(hint) = failure_tracker::failure_tracker().record_failure(name) {
        log::warn!("{hint}");
    }
}
