//! O registro de turno em curso por janela — a peça que faltava para `mika:chat-cancel`.
//!
//! `brain::turn::TurnAbortHandle`/`TurnAbortSignal` já provam, em teste, que abortar cancela a
//! chamada em voo em vez de deixar uma segunda resposta escapar (ver o cabeçalho de `turn.rs`).
//! O que faltava não era o mecanismo — era onde guardar o handle do turno EM CURSO entre a chamada
//! de `mika:chat-send` que o cria (`send.rs::handle_chat_send`) e uma chamada de `mika:chat-cancel`
//! SEGUINTE. `ChatModule.turns` (`Mutex<HashMap<window_label, TurnAbortHandle>>`, em `mod.rs`) é
//! esse lugar: vive pela vida do app porque `ChatModule` é construído uma vez em `modules()` e fica
//! dentro de `ShellState.native` — sem precisar editar `state.rs`. Uma entrada por JANELA, não uma
//! só, porque `is_owner_surface` permite `main`/`settings`/`wizard` chamarem `chat-send`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde_json::{json, Value};

use crate::modules::brain::turn;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

/// Por janela, TODOS os turnos vivos, cada um com o token que `register_turn` devolveu — duas
/// fontes podem rodar sob o mesmo label ao mesmo tempo (resposta a barge-in + `chat-send`).
pub type Turns = Mutex<HashMap<String, Vec<(u64, turn::TurnAbortHandle)>>>;

/// Token monotônico de registro: quem terminou só desregistra o PRÓPRIO turno.
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// O registro é ÚNICO no processo, não um campo privado de `ChatModule`.
///
/// Motivo concreto, não estético: o barge-in mora em `modules/voice` e precisa abortar o turno
/// quando o usuário fala por cima da Mika. Com o registro preso dentro de `ChatModule`, a frente da
/// voz portou os dois consumidores de VAD, provou que não perde janela de áudio, e ainda assim NÃO
/// pôde reivindicar `voice:vad-audio`: o barge-in dispararia certo e não acionaria nada — o turno
/// seguiria rodando. Detectar sem poder atuar é o dano silencioso que este porte recusa.
///
/// Uma instância só pelo mesmo motivo que o `ConfigStore` é um só: duas divergem em silêncio, e
/// aqui divergir significa abortar um handle que ninguém espera enquanto o turno de verdade segue
/// vivo. Mesmo padrão do `OnceLock` de `modules/tools` e de `modules/mcp`.
static TURNS: OnceLock<Turns> = OnceLock::new();

pub fn registry() -> &'static Turns {
    TURNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Aborta TODOS os turnos em curso de uma janela, de fora do módulo `chat`.
///
/// `false` não é erro — é "nada em voo", o mesmo contrato que `handle_chat_cancel` já honra e que o
/// TS tem em `abortCurrentRequest`, que sempre responde sucesso.
pub fn abort_turn(window_label: &str) -> Result<bool, String> {
    abort_all(registry(), window_label)
}

fn abort_all(turns: &Turns, window_label: &str) -> Result<bool, String> {
    let handles = lock(turns)?.remove(window_label).unwrap_or_default();
    handles.iter().for_each(|(_, handle)| handle.abort());
    Ok(!handles.is_empty())
}

fn lock(turns: &Turns) -> Result<MutexGuard<'_, HashMap<String, Vec<(u64, turn::TurnAbortHandle)>>>, String> {
    turns.lock().map_err(|_| "registro de turnos envenenado".to_string())
}

/// Guarda o handle do turno desta janela e devolve o token para `deregister_turn`. Um turno já
/// registrado sob o mesmo label NÃO é sobrescrito nem perdido (barge-in `send_interruption_reply` e
/// `chat-send` de "main" coexistem): os dois ficam, e abortar a janela aborta os dois.
pub fn register_turn(turns: &Turns, window_label: &str, handle: turn::TurnAbortHandle) -> Result<u64, String> {
    let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    lock(turns)?.entry(window_label.to_string()).or_default().push((token, handle));
    Ok(token)
}

/// Remove só o turno de `token` — um turno que terminou nunca desregistra o de outro.
pub fn deregister_turn(turns: &Turns, window_label: &str, token: u64) -> Result<(), String> {
    let mut map = lock(turns)?;
    if let Some(entries) = map.get_mut(window_label) {
        entries.retain(|(entry_token, _)| *entry_token != token);
        if entries.is_empty() {
            map.remove(window_label);
        }
    }
    Ok(())
}

/// Registro de turno que se desfaz sozinho ao sair de escopo — sucesso, erro, panic ou future
/// descartada (janela fechando). `deregister_turn` só no caminho feliz deixava o handle no registro
/// nos outros casos.
pub struct TurnGuard<'a> {
    turns: &'a Turns,
    window_label: String,
    token: u64,
}

impl Drop for TurnGuard<'_> {
    fn drop(&mut self) {
        // Registro envenenado: nada a desfazer que valha derrubar quem já está saindo de escopo.
        let _ = deregister_turn(self.turns, &self.window_label, self.token);
    }
}

pub fn register_guarded<'a>(turns: &'a Turns, window_label: &str, handle: turn::TurnAbortHandle) -> Result<TurnGuard<'a>, String> {
    let token = register_turn(turns, window_label, handle)?;
    Ok(TurnGuard { turns, window_label: window_label.to_string(), token })
}

/// `mika:chat-cancel`: aborta o turno em curso da janela chamadora, se houver. Sempre
/// `{ success: true }` — mesma convenção de `chat/ipc.ts::registerChatIPC`: não haver turno para
/// abortar não é erro, é ausência de trabalho a cancelar.
pub fn handle_chat_cancel(ctx: &InvokeContext, turns: &Turns) -> Result<Value, String> {
    if !is_owner_surface(ctx.window_type) {
        return Err(format!("mika:chat-cancel recusado para {:?}", ctx.window_type));
    }
    abort_all(turns, &ctx.window_label)?;
    Ok(json!({ "success": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;

    fn ctx(window_type: WindowType, window_label: &str) -> InvokeContext {
        InvokeContext { window_type, window_label: window_label.to_string(), plugin_id: None }
    }

    /// O caminho central deste módulo: um handle registrado por `chat-send` É alcançado e abortado
    /// por uma chamada `chat-cancel` SEGUINTE — a prova de que o registro por janela resolve o
    /// problema descrito no cabeçalho do arquivo (handle local, sem onde guardá-lo entre chamadas).
    #[test]
    fn handle_chat_cancel_aborts_the_turn_registered_for_the_same_window() {
        let turns: Turns = Mutex::new(HashMap::new());
        let (handle, signal) = turn::TurnAbortHandle::new();
        register_turn(&turns, "main", handle).unwrap();
        assert!(!signal.is_aborted());

        let response = handle_chat_cancel(&ctx(WindowType::Main, "main"), &turns).unwrap();

        assert_eq!(response, json!({ "success": true }));
        assert!(signal.is_aborted());
        assert!(lock(&turns).unwrap().is_empty(), "o handle abortado tem de sair do registro");
    }

    /// Sem turno em curso, `chat-cancel` não é erro — idempotente, mesma convenção do TS.
    #[test]
    fn handle_chat_cancel_is_a_no_op_success_when_nothing_is_in_flight() {
        let turns: Turns = Mutex::new(HashMap::new());
        let response = handle_chat_cancel(&ctx(WindowType::Main, "main"), &turns).unwrap();
        assert_eq!(response, json!({ "success": true }));
    }

    /// `chat-cancel` só aborta o handle da PRÓPRIA janela — outra janela em curso não é afetada.
    #[test]
    fn handle_chat_cancel_only_touches_the_calling_window() {
        let turns: Turns = Mutex::new(HashMap::new());
        let (main_handle, main_signal) = turn::TurnAbortHandle::new();
        let (settings_handle, settings_signal) = turn::TurnAbortHandle::new();
        register_turn(&turns, "main", main_handle).unwrap();
        register_turn(&turns, "settings", settings_handle).unwrap();

        handle_chat_cancel(&ctx(WindowType::Main, "main"), &turns).unwrap();

        assert!(main_signal.is_aborted());
        assert!(!settings_signal.is_aborted());
        assert!(lock(&turns).unwrap().contains_key("settings"));
    }

    /// Barge-in (`send_interruption_reply`) e `chat-send` podem compartilhar o label "main". Antes,
    /// o segundo `register_turn` sobrescrevia o handle do primeiro e o `deregister_turn(label)` de
    /// quem terminasse primeiro removia o handle do OUTRO — o turno real virava não abortável.
    #[test]
    fn a_turn_finishing_never_unregisters_another_turn_under_the_same_label() {
        let turns: Turns = Mutex::new(HashMap::new());
        let (real_handle, real_signal) = turn::TurnAbortHandle::new();
        let (reply_handle, reply_signal) = turn::TurnAbortHandle::new();
        register_turn(&turns, "main", real_handle).unwrap();
        let reply = register_turn(&turns, "main", reply_handle).unwrap();

        deregister_turn(&turns, "main", reply).unwrap();

        let response = handle_chat_cancel(&ctx(WindowType::Main, "main"), &turns).unwrap();
        assert_eq!(response, json!({ "success": true }));
        assert!(real_signal.is_aborted(), "o turno real tem de continuar abortável");
        assert!(!reply_signal.is_aborted(), "quem já terminou não é abortado");
    }

    /// Dois turnos vivos sob o mesmo label: abortar a janela aborta os dois, nenhum handle se perde.
    #[test]
    fn aborting_a_window_aborts_every_turn_registered_under_its_label() {
        let turns: Turns = Mutex::new(HashMap::new());
        let (first, first_signal) = turn::TurnAbortHandle::new();
        let (second, second_signal) = turn::TurnAbortHandle::new();
        register_turn(&turns, "main", first).unwrap();
        register_turn(&turns, "main", second).unwrap();

        handle_chat_cancel(&ctx(WindowType::Main, "main"), &turns).unwrap();

        assert!(first_signal.is_aborted() && second_signal.is_aborted());
        assert!(lock(&turns).unwrap().is_empty());
    }

    #[test]
    fn handle_chat_cancel_refuses_a_non_owner_surface() {
        let turns: Turns = Mutex::new(HashMap::new());
        assert!(handle_chat_cancel(&ctx(WindowType::PluginView, "plugin-view"), &turns).is_err());
    }

    /// `deregister_turn` só no caminho feliz deixava o handle no registro quando o turno dava panic ou
    /// a future era descartada (janela fechando): o `chat-cancel` seguinte abortava um handle morto e o
    /// registro crescia. O guard desregistra no `Drop`.
    #[test]
    fn a_turn_guard_unregisters_on_drop_even_when_the_turn_panics() {
        let turns: Turns = Mutex::new(HashMap::new());
        let (handle, _signal) = turn::TurnAbortHandle::new();

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = register_guarded(&turns, "main", handle).unwrap();
            assert!(lock(&turns).unwrap().contains_key("main"));
            panic!("turno quebrou");
        }));

        assert!(outcome.is_err());
        assert!(lock(&turns).unwrap().is_empty(), "o panic deixou o handle no registro");
    }

    /// O guard só desfaz o PRÓPRIO registro — outro turno da mesma janela segue abortável.
    #[test]
    fn dropping_a_guard_leaves_the_other_turns_of_the_window_registered() {
        let turns: Turns = Mutex::new(HashMap::new());
        let (first, _) = turn::TurnAbortHandle::new();
        let (second, _) = turn::TurnAbortHandle::new();
        register_turn(&turns, "main", first).unwrap();

        drop(register_guarded(&turns, "main", second).unwrap());

        assert_eq!(lock(&turns).unwrap()["main"].len(), 1);
    }
}
