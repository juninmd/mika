//! Guarda por RESULTADO: o turno que chamou uma tool de efeito (lembrete, nota, catálogo, perfil,
//! abrir/rodar algo) e NENHUMA delas deu certo não pode terminar com a Mika dizendo que fez.
//!
//! O detector antigo (`action_claim`) lê o texto em português ("abrindo", "cliquei"); em japonês ou
//! em inglês não pega nada, e é no idioma errado que o "pode deixar" falso nasce. Este guarda não
//! lê texto nenhum: olha o desfecho das tools (`ToolCallFinished { ok }`). Quando só há falha de
//! efeito, a fala final do modelo é segurada (nem texto, nem voz) e trocada por "não consegui fazer
//! isso" no idioma do app (`chat.effectFailed`); com qualquer sucesso, ou sem tool de efeito, tudo
//! passa intacto. Segurar o `ThinkingStop` junto mantém o par usuário/Mika que a memória monta a
//! partir do barramento coerente: o aviso entra antes do fim do turno.

use std::sync::Mutex;

use super::{TurnEvent, TurnObserver, TurnResult};

/// Tools cujo sucesso é o efeito que o usuário pediu e que a resposta poderia afirmar.
const EFFECT_TOOLS: [&str; 7] = ["reminder", "memory_console", "person_catalog", "user_profile", "desktop", "computer", "code"];

/// As ações de LEITURA dessas tools (enum do schema de cada uma): ler e não achar (`person_catalog
/// get` → `not_found`) não é "a ação que o usuário pediu falhou", e a resposta honesta ("não
/// conheço") não pode virar "Não consegui fazer isso.". Declara-se a leitura, não o efeito: ação
/// nova ou ausente de uma tool de efeito conta como efeito até alguém provar que só lê.
const READ_ACTIONS: [(&str, &[&str]); 6] = [
    ("reminder", &["list"]),
    ("memory_console", &["snapshot", "search_episodic", "facts"]),
    ("person_catalog", &["get", "search", "list", "relationships", "path"]),
    ("user_profile", &["get", "query"]),
    ("desktop", &["window", "process"]),
    ("computer", &["find"]),
];

fn is_effect_call(name: &str, action: Option<&str>) -> bool {
    EFFECT_TOOLS.contains(&name) && !READ_ACTIONS.iter().any(|(tool, reads)| *tool == name && action.is_some_and(|a| reads.contains(&a)))
}

#[derive(Default)]
struct State {
    effect_calls: usize,
    effect_successes: usize,
    held: Vec<TurnEvent>,
}

impl State {
    /// Houve tool de efeito e nenhuma deu certo: a próxima fala do modelo não é confiável.
    fn only_failures(&self) -> bool {
        self.effect_calls > 0 && self.effect_successes == 0
    }
}

pub(super) struct EffectGuard<'a> {
    inner: &'a dyn TurnObserver,
    notice: String,
    state: Mutex<State>,
}

impl<'a> EffectGuard<'a> {
    /// `notice`: o "não consegui fazer isso" já no idioma do app.
    pub(super) fn new(inner: &'a dyn TurnObserver, notice: String) -> Self {
        Self { inner, notice, state: Mutex::new(State::default()) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn release_held(&self, state: &mut State) {
        for event in state.held.drain(..) {
            self.inner.emit(event);
        }
    }

    /// Fecha o turno: com só falha de efeito, a fala segurada vira o aviso (e `result.text` também,
    /// o que vai para o histórico e para a resposta do chat); fora disso solta o que estiver segurado.
    pub(super) fn finish(&self, mut result: Result<TurnResult, super::TurnError>) -> Result<TurnResult, super::TurnError> {
        let mut state = self.lock();
        if state.only_failures() {
            if let Ok(turn) = result.as_mut() {
                let stop = state.held.iter().any(|event| matches!(event, TurnEvent::ThinkingStop));
                state.held.clear();
                self.inner.emit(TurnEvent::TextChunk(self.notice.clone()));
                if stop {
                    self.inner.emit(TurnEvent::ThinkingStop);
                }
                turn.text = self.notice.clone();
                return result;
            }
        }
        self.release_held(&mut state);
        result
    }
}

impl TurnObserver for EffectGuard<'_> {
    fn emit(&self, event: TurnEvent) {
        let mut state = self.lock();
        match &event {
            TurnEvent::ToolCallFinished { name, action, ok, .. } if is_effect_call(name, action.as_deref()) => {
                state.effect_calls += 1;
                state.effect_successes += usize::from(*ok);
            }
            TurnEvent::TextChunk(_) | TurnEvent::ThinkingStop if state.only_failures() => {
                state.held.push(event);
                return;
            }
            // Nova rodada de tool (o texto segurado era só um comentário no meio) ou um sucesso que
            // devolve a confiança: o que ficou segurado sai agora, na ordem.
            TurnEvent::ToolCallStarted { .. } => self.release_held(&mut state),
            _ => {}
        }
        if !state.only_failures() {
            self.release_held(&mut state);
        }
        drop(state);
        self.inner.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<TurnEvent>>);
    impl TurnObserver for Recorder {
        fn emit(&self, event: TurnEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    fn finished(name: &str, ok: bool) -> TurnEvent {
        finished_action(name, None, ok)
    }

    fn finished_action(name: &str, action: Option<&str>, ok: bool) -> TurnEvent {
        TurnEvent::ToolCallFinished { name: name.into(), action: action.map(Into::into), ok, result: json!({ "success": ok }), duration_ms: 1 }
    }

    fn turn(text: &str) -> Result<TurnResult, super::super::TurnError> {
        Ok(TurnResult { text: text.into(), usage: rig_core::completion::Usage::new(), served_by_failover: false })
    }

    /// Roda um turno sintético pelo guarda e devolve o que o observer de fora viu + o texto final.
    fn run(events: Vec<TurnEvent>, final_text: &str) -> (Vec<TurnEvent>, String) {
        let recorder = Arc::new(Recorder::default());
        let guard = EffectGuard::new(recorder.as_ref(), "NÃO CONSEGUI".to_string());
        for event in events {
            guard.emit(event);
        }
        let result = guard.finish(turn(final_text)).unwrap();
        let seen = recorder.0.lock().unwrap().clone();
        (seen, result.text)
    }

    fn chunk(text: &str) -> TurnEvent {
        TurnEvent::TextChunk(text.into())
    }

    /// O achado: o lembrete falhou e a Mika disse "pode deixar". Em QUALQUER idioma — o guarda não
    /// lê o texto, só o desfecho da tool.
    #[test]
    fn a_failed_effect_tool_replaces_the_claim_in_every_language() {
        for claim in ["Pronto, lembrete criado!", "Done, I set the reminder!", "リマインダーを設定したよ！", "Listo, recordatorio creado.", "好的，提醒已设置。"] {
            let (seen, text) = run(vec![TurnEvent::ThinkingStart, finished("reminder", false), chunk(claim), TurnEvent::ThinkingStop], claim);
            assert_eq!(text, "NÃO CONSEGUI", "{claim}");
            assert_eq!(
                seen,
                [TurnEvent::ThinkingStart, finished("reminder", false), chunk("NÃO CONSEGUI"), TurnEvent::ThinkingStop],
                "a fala falsa nunca chega ao observer (nem à voz): {claim}"
            );
        }
    }

    /// O achado M2: `person_catalog get` de alguém desconhecido (`not_found`) é uma LEITURA que não
    /// achou nada, não "a ação que o usuário pediu falhou": a resposta ("não conheço essa pessoa")
    /// é legítima e não pode virar "Não consegui fazer isso." (nem no histórico).
    #[test]
    fn a_failed_read_does_not_replace_the_answer() {
        let reads = [
            ("person_catalog", "get"), ("person_catalog", "search"), ("person_catalog", "list"), ("person_catalog", "relationships"), ("person_catalog", "path"),
            ("user_profile", "get"), ("user_profile", "query"), ("desktop", "window"), ("desktop", "process"),
            ("memory_console", "search_episodic"), ("memory_console", "snapshot"), ("memory_console", "facts"),
            ("reminder", "list"), ("computer", "find"),
        ];
        for (tool, action) in reads {
            let events = vec![TurnEvent::ThinkingStart, finished_action(tool, Some(action), false), chunk("Não conheço essa pessoa."), TurnEvent::ThinkingStop];
            let (seen, text) = run(events.clone(), "Não conheço essa pessoa.");
            assert_eq!(seen, events, "{tool} {action}");
            assert_eq!(text, "Não conheço essa pessoa.", "{tool} {action}");
        }
    }

    /// A outra metade: a escrita que falhou continua trocando a fala, e uma ação nova (ou ausente)
    /// de tool de efeito conta como efeito até alguém declará-la leitura.
    #[test]
    fn a_failed_write_or_unknown_action_still_replaces_the_claim() {
        let writes = [
            ("person_catalog", Some("upsert")), ("person_catalog", Some("delete")), ("person_catalog", Some("add_fact")), ("user_profile", Some("learn")),
            ("user_profile", Some("forget")), ("desktop", Some("launch")), ("reminder", Some("schedule")), ("reminder", Some("cancel")),
            ("memory_console", Some("add_fact")), ("computer", Some("click")), ("code", Some("run_js")), ("person_catalog", Some("acao_nova")), ("reminder", None),
        ];
        for (tool, action) in writes {
            let events = vec![TurnEvent::ThinkingStart, finished_action(tool, action, false), chunk("Pronto, feito!"), TurnEvent::ThinkingStop];
            let (_, text) = run(events, "Pronto, feito!");
            assert_eq!(text, "NÃO CONSEGUI", "{tool} {action:?}");
        }
    }

    #[test]
    fn a_successful_effect_or_a_non_effect_tool_passes_everything_through() {
        for (tool, ok) in [("reminder", true), ("get_system_info", false), ("web", false)] {
            let events = vec![TurnEvent::ThinkingStart, finished(tool, ok), chunk("fiz"), TurnEvent::ThinkingStop];
            let (seen, text) = run(events.clone(), "fiz");
            assert_eq!(seen, events, "{tool} ok={ok}");
            assert_eq!(text, "fiz");
        }
    }

    #[test]
    fn a_retry_that_succeeds_restores_trust_and_nothing_is_lost() {
        let events = vec![
            TurnEvent::ThinkingStart,
            finished("person_catalog", false),
            chunk("deixa eu tentar de novo. "),
            TurnEvent::ToolCallStarted { name: "person_catalog".into() },
            finished("person_catalog", true),
            chunk("pronto, salvei."),
            TurnEvent::ThinkingStop,
        ];
        let (seen, text) = run(events.clone(), "pronto, salvei.");
        assert_eq!(seen, events, "a conversa entre as tentativas e a resposta final saem intactas e em ordem");
        assert_eq!(text, "pronto, salvei.");
    }

    /// Falhou, o turno também falhou (provedor caiu): nada segurado se perde.
    #[test]
    fn an_errored_turn_releases_what_was_held() {
        let recorder = Arc::new(Recorder::default());
        let guard = EffectGuard::new(recorder.as_ref(), "x".into());
        guard.emit(finished("reminder", false));
        guard.emit(chunk("parcial"));
        guard.emit(TurnEvent::ThinkingStop);
        assert!(guard.finish(Err(super::super::TurnError::Aborted)).is_err());
        let seen = recorder.0.lock().unwrap().clone();
        assert_eq!(seen, [finished("reminder", false), chunk("parcial"), TurnEvent::ThinkingStop]);
    }
}
