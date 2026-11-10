//! `send_awareness`/`send_reminder`/`send_proactive_unless` — um turno de chat SEM mensagem de usuário, disparado pelo motor proativo
//! (`brain::awareness::ProactiveEngine`). Reusa o MESMO par observador/barramento de `send.rs`
//! (`turn_event_bus`) para que a fala automática (AutoSpeak) trate um turno proativo como qualquer
//! outro. O renderer recebe só a mensagem final (`assistant_bridge`), como no TS: repassar o
//! thinking/stream acendia "PENSANDO" no HUD para uma fala que ninguém pediu.
//!
//! `run_turn_llm_raw` (não `run_turn_llm`) é o turno certo aqui: aceita `user_content` sem exigir
//! que ele seja literalmente "o que o usuário disse" — é o mesmo par que `game_session_tool`/
//! `game_knowledge_tool` já usam para turnos instrucionais sem mensagem de usuário real.
//!
//! Histórico: só a fala (papel `assistant`) é gravada, via o chokepoint de escrita já existente
//! (`super::append_message`, sobre o MESMO `chat.db`) — um turno proativo não tem "mensagem de
//! usuário" para gravar, mesmo comportamento de `sendProactive` no TS.
//!
//! **`send_interruption_reply`** (defects.md "barge-in", item 2): a variante que
//! `brain::awareness::engine::ProactiveEngine::on_interruption` chama. Sem isto, o
//! `TurnAbortHandle` da resposta a uma interrupção nunca era registrado em `chat::turns` — um
//! SEGUNDO barge-in na mesma janela cortava o áudio (`ForceStopTts`) mas não achava nada para
//! abortar em `chat_turns::abort_turn(window_label)`, e a chamada de LLM em curso seguia rodando
//! sozinha. Registra sob a MESMA janela que sofreu o barge-in (`window_label`, publicado por
//! `voice::vad_audio::interruption_event_payload`) via [`with_registered_turn`], que qualquer
//! chamada futura que precise do mesmo tratamento pode reusar.
//!
//! **`barge_in_at_ms`** (code review, major): `run_proactive_turn` já descartava a resposta se um
//! turno real COMEÇASSE enquanto ela esperava o LLM (`chat_turns_started() != turns_before`) — mas
//! `turns_before` só era capturado quando esta função começava a rodar, ou seja, DEPOIS que o Rust
//! termina de gravar (2.5s fixos) e transcrever o áudio pós-barge-in. O STT contínuo do renderer, que
//! só precisa de ~1.2s de silêncio depois da fala, comumente já respondeu e TERMINOU a mesma
//! utterance antes disso — um turno que já tinha começado E terminado nunca aparecia na diferença.
//! `barge_in_at_ms` é o instante em que o barge-in foi DETECTADO (`vad_audio.rs::react_to_barge_in`,
//! bem mais cedo que qualquer turno para a MESMA fala poderia ter começado), e
//! `turn::chat_turn_completed_since` fecha essa janela comparando contra ele, não contra o instante
//! em que esta função foi chamada.

use tauri::AppHandle;

use super::send::turn_event_bus;
use crate::config::store::ConfigStore;
use crate::events::{bus, EventBus, EventKey};
use crate::modules::brain::turn;

/// Holds the turn's bus events until the finished text is approved: AutoSpeak streams
/// `StreamChunk`, so publishing live spoke drafts that the repetition guard or the staleness
/// checks in `run_proactive_turn` later discarded (smoke 2026-09-26).
struct ProactiveTurnObserver<'a> {
    bus: &'a EventBus,
    held: std::sync::Mutex<Vec<(EventKey, serde_json::Value)>>,
}

impl<'a> ProactiveTurnObserver<'a> {
    fn new(bus: &'a EventBus) -> Self {
        Self { bus, held: std::sync::Mutex::new(Vec::new()) }
    }

    fn release(&self) {
        for (key, payload) in self.held.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
            self.bus.publish(key, payload);
        }
    }
}

impl turn::TurnObserver for ProactiveTurnObserver<'_> {
    fn emit(&self, event: turn::TurnEvent) {
        // Turno proativo não tem "conteúdo do usuário" — string vazia no mapeamento reaproveitado de
        // `send.rs` (só `ThinkingStart` usa `userContent`, e HUD/AutoSpeak não dependem dele aqui).
        if let Some((key, mut payload)) = turn_event_bus(&event, "") {
            // Keeps the post-turn pipeline from storing a speech nobody asked for as a conversation.
            if let Some(map) = payload.as_object_mut() {
                map.insert("origin".into(), serde_json::Value::from(crate::modules::brain::post_turn::BACKGROUND_ORIGIN));
            }
            self.held.lock().unwrap_or_else(|e| e.into_inner()).push((key, payload));
        }
    }
}

/// A fired reminder: never held back as a repeat — "beber água" twice a day is the point.
pub async fn send_reminder(app: &AppHandle, store: &ConfigStore, prompt: &str) -> String {
    let (_handle, signal) = turn::TurnAbortHandle::new();
    run_proactive_turn(app, store, prompt, signal, &|| false, None, false).await.into_text()
}

/// How a proactive turn ended. `HeldBack` is a choice (a repeat, or the model passed), not a failure:
/// the awareness engine keeps its cooldown for it so she waits instead of rephrasing right away.
enum ProactiveReply {
    Spoken(String),
    HeldBack,
    Silent,
}

impl ProactiveReply {
    fn into_text(self) -> String {
        match self {
            Self::Spoken(text) => text,
            Self::HeldBack | Self::Silent => String::new(),
        }
    }
}

/// The awareness engine's turn: sees what she said lately and may pass, see [`super::proactive_novelty`].
pub async fn send_awareness(app: &AppHandle, store: &ConfigStore, prompt: &str) -> String {
    let recent = crate::modules::brain::guards::repetition_guard().recent().to_vec();
    let prompt = super::proactive_novelty::with_recent_lines(prompt, &recent);
    let (_handle, signal) = turn::TurnAbortHandle::new();
    match run_proactive_turn(app, store, &prompt, signal, &|| false, None, true).await {
        ProactiveReply::HeldBack => crate::modules::brain::awareness::engine::HELD_BACK.to_string(),
        reply => reply.into_text(),
    }
}

/// The repetition guard's meaning check runs the ONNX embedder: blocking work, kept off the async
/// workers that also drive the user's chat turn.
async fn approve_off_thread(text: &str) -> bool {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || crate::modules::brain::guards::approve_proactive(&text))
        .await
        .unwrap_or_else(|error| {
            log::warn!("guarda de repetição falhou, fala proativa retida: {error}");
            false
        })
}

/// Turno proativo que desiste se `stale()` virar verdade — antes do LLM e antes de falar/gravar.
/// O wizard usa isto: clicar rápido por 4 etapas não pode enfileirar 4 falas, das quais o balão
/// mostra só a última.
pub async fn send_proactive_unless(
    app: &AppHandle,
    store: &ConfigStore,
    prompt: &str,
    stale: &(dyn Fn() -> bool + Sync),
) -> String {
    if stale() {
        return String::new();
    }
    let (_handle, signal) = turn::TurnAbortHandle::new();
    // `None`: nenhum barge-in envolvido aqui, então a checagem extra de `chat_turn_completed_since`
    // não se aplica (ver o cabeçalho do arquivo, "barge_in_at_ms").
    run_proactive_turn(app, store, prompt, signal, stale, None, true).await.into_text()
}

/// Variante de `send_proactive_unless` para a resposta a uma interrupção de voz
/// (`brain::awareness::engine::ProactiveEngine::on_interruption`) — registra o handle de abort sob
/// `window_label` (a janela que sofreu o barge-in) ANTES de chamar o LLM, para que um SEGUNDO
/// barge-in na mesma janela aborte esta chamada via `chat::turns::abort_turn`, não só o áudio (ver
/// o cabeçalho do arquivo, item 2). Sem `stale()`: uma resposta a interrupção não tem "próxima
/// etapa do wizard" que a torne velha — o próprio `run_proactive_turn` já descarta se um turno REAL
/// (`chat-send`) começar enquanto ela espera o LLM, OU já tiver terminado antes mesmo disso
/// (`barge_in_at_ms`, ver o cabeçalho do arquivo).
pub async fn send_interruption_reply(
    app: &AppHandle,
    store: &ConfigStore,
    prompt: &str,
    window_label: &str,
    barge_in_at_ms: Option<u64>,
) -> String {
    let (handle, signal) = turn::TurnAbortHandle::new();
    // Foreground: it answers the user, and behind the background queue it took 40 s (smoke 2026-09-26).
    let reply = run_proactive_turn(app, store, prompt, signal, &|| false, barge_in_at_ms, true);
    with_registered_turn(window_label, handle, turn::run_as_foreground(reply)).await.into_text()
}

/// Registra `handle` sob `window_label` para a duração de `work`, desregistrando sempre ao final —
/// sucesso, erro, abort ou future descartada (o `Drop` do guard). Extraído para ser testável sem precisar de um turno de LLM real (`work`
/// pode ser qualquer future em teste).
async fn with_registered_turn<F: std::future::Future>(window_label: &str, handle: turn::TurnAbortHandle, work: F) -> F::Output {
    let _guard = super::turns::register_guarded(super::turns::registry(), window_label, handle)
        .inspect_err(|error| log::warn!("resposta a interrupção: não registrou o turno de {window_label}: {error}"))
        .ok();
    work.await
}

/// O corpo comum de `send_proactive_unless`/`send_interruption_reply`: chama o LLM, descarta se um
/// turno real começou enquanto esperava (`stale`/`chat_turns_started`) OU já tinha terminado antes
/// mesmo do barge-in que motivou esta resposta (`barge_in_at_ms`, `None` para candidatos sem barge-in
/// envolvido — ver o cabeçalho do arquivo), grava e fala se o texto veio não-vazio e passou pelo
/// guarda de repetição.
async fn run_proactive_turn(
    app: &AppHandle,
    store: &ConfigStore,
    prompt: &str,
    signal: turn::TurnAbortSignal,
    stale: &(dyn Fn() -> bool + Sync),
    barge_in_at_ms: Option<u64>,
    repetition_guarded: bool,
) -> ProactiveReply {
    let observer = ProactiveTurnObserver::new(bus());
    let turns_before = turn::chat_turns_started();
    // `PromptFamily::Chat`: jogada proativa é a mesma família de prompt do chat principal (persona)
    // — mesmo slot de KV-cache, ver `turn::kv_cache_id_slot` (item 6).
    let outcome = turn::run_turn_llm_raw_family(turn::PromptFamily::Chat, store, None, prompt, signal, &observer).await;
    let already_answered = barge_in_at_ms.is_some_and(turn::chat_turn_completed_since);
    // A fila de fundo espera o chat: se o usuário falou enquanto esta fala esperava, ela nasceu
    // velha (um "pode falar" DEPOIS da resposta ao que ele disse) — ou o turno real já tinha
    // respondido esta MESMA interrupção antes mesmo de `turns_before` ser capturado.
    let outdated = outcome.is_ok() && (turn::chat_turns_started() != turns_before || stale() || already_answered);
    // Before the guard: approving records the line, and a pass is not a line.
    let repeated = match &outcome {
        Ok(result) if repetition_guarded && !outdated && !super::proactive_novelty::is_pass(&result.text) => {
            !approve_off_thread(&result.text).await
        }
        _ => false,
    };
    match outcome {
        Ok(_) if outdated => ProactiveReply::Silent,
        Ok(result) if super::proactive_novelty::is_pass(&result.text) => ProactiveReply::HeldBack,
        // Same observation, same line: TS dropped it here too (`chat-proactive.runtime.ts`).
        Ok(_) if repeated => ProactiveReply::HeldBack,
        Ok(result) if !result.text.trim().is_empty() => {
            match super::chat_db_path(app) {
                Ok(db_path) => {
                    if let Err(error) = super::append_message(&db_path, "assistant", &result.text) {
                        log::warn!("fala proativa gerada mas não persistida no histórico: {error}");
                    }
                }
                Err(error) => log::warn!("fala proativa gerada mas chat.db não resolveu: {error}"),
            }
            observer.release();
            let (provider, model) = super::send::resolve_health_key(store);
            let cost = if result.served_by_failover {
                Some(0.0)
            } else {
                crate::modules::token_usage::cost_for(app, &provider, &model, &result.usage)
            };
            crate::modules::token_usage::record(app, "chat", &provider, &model, &result.usage, cost);
            let context_window = crate::modules::llm::model_profile::effective_context_window(store, &provider, &model).await;
            let payload = super::assistant_bridge::turn_payload(&result.text, super::send::now_millis(), &result.usage, cost, context_window);
            super::assistant_bridge::emit_to_renderer(app, &payload);
            ProactiveReply::Spoken(result.text)
        }
        Ok(_) => ProactiveReply::Silent,
        Err(error) => {
            log::warn!("turno proativo falhou: {error}");
            ProactiveReply::Silent
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::chat::turns as chat_turns;

    /// A prova central do item 2: um handle registrado por `with_registered_turn` é alcançado por
    /// `chat::turns::abort_turn` — a peça que faltava para um segundo barge-in cortar a chamada de
    /// LLM da resposta a interrupção, não só o áudio (`vad_audio.rs::react_to_barge_in` já fazia
    /// isto para o turno PRINCIPAL; faltava só para esta resposta de fundo).
    #[tokio::test]
    async fn with_registered_turn_lets_a_later_abort_reach_the_registered_handle() {
        let window_label = "proactive-test::registered-turn";
        let (handle, signal) = turn::TurnAbortHandle::new();

        let (found, aborted) = with_registered_turn(window_label, handle, async {
            // Simula um SEGUNDO barge-in chegando enquanto a resposta ainda está "em voo".
            let found = chat_turns::abort_turn(window_label).unwrap();
            (found, signal.is_aborted())
        })
        .await;

        assert!(found, "abort_turn precisa achar o handle registrado para esta janela");
        assert!(aborted, "o sinal do turno tem de virar abortado");
        assert!(
            chat_turns::registry().lock().unwrap().get(window_label).is_none(),
            "o turno tem de sair do registro ao final, abortado ou não"
        );
    }

    /// Smoke 2026-09-26: a draft the repetition guard later drops was already streaming to TTS.
    #[tokio::test]
    async fn proactive_turn_events_reach_the_bus_only_once_released() {
        use turn::TurnObserver;
        let local = EventBus::new();
        let mut sub = local.subscribe();
        let observer = ProactiveTurnObserver::new(&local);
        observer.emit(turn::TurnEvent::TextChunk("oi".into()));
        let early = tokio::time::timeout(std::time::Duration::from_millis(50), sub.recv()).await;
        assert!(early.is_err(), "nothing may be spoken before the text is approved");
        observer.release();
        let late = tokio::time::timeout(std::time::Duration::from_millis(50), sub.recv()).await.unwrap().unwrap();
        assert_eq!(late.key, EventKey::StreamChunk);
    }

    /// Sem um segundo barge-in, o turno termina normalmente e AINDA ASSIM desregistra — um handle
    /// esquecido no registro abortaria por engano uma resposta futura na mesma janela.
    #[tokio::test]
    async fn with_registered_turn_deregisters_even_when_nothing_aborts_it() {
        let window_label = "proactive-test::clean-finish";
        let (handle, _signal) = turn::TurnAbortHandle::new();

        let result = with_registered_turn(window_label, handle, async { "ok" }).await;

        assert_eq!(result, "ok");
        assert!(chat_turns::registry().lock().unwrap().get(window_label).is_none());
    }

    /// Janela fechando / app saindo descarta a future no meio: o handle não pode ficar no registro
    /// (o `chat-cancel` seguinte abortaria um turno que já não existe).
    #[tokio::test]
    async fn with_registered_turn_deregisters_when_the_future_is_dropped_midway() {
        let window_label = "proactive-test::dropped-future";
        let (handle, _signal) = turn::TurnAbortHandle::new();

        let dropped = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            with_registered_turn(window_label, handle, std::future::pending::<()>()),
        )
        .await;

        assert!(dropped.is_err(), "a future pendente tem de estourar o timeout e ser descartada");
        assert!(chat_turns::registry().lock().unwrap().get(window_label).is_none(), "o handle ficou no registro");
    }
}
