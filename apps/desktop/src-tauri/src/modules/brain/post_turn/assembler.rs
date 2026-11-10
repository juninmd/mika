//! Reconstrução do par usuário/assistente de um turno a partir dos eventos que `chat::send`
//! JÁ publica no barramento (`EventKey::ThinkingStart`/`StreamChunk`/`ThinkingStop`, ver
//! `modules/chat/send.rs::turn_event_bus`). Não editamos `send.rs` nesta rodada (fora do nosso
//! escopo), então não existe um evento pronto com "usuário + resposta completa" — só o texto de
//! entrada (`ThinkingStart{userContent}`) e o stream token a token (`StreamChunk{text}`) que
//! `send.rs` já traduz do `TurnEvent::TextChunk` de `turn.rs`.
//!
//! [`TurnAssembler`] é o mesmo acúmulo que `IpcTurnObserver`/`turn::run_turn` já fazem para montar
//! `TurnResult.text` (concatenar `TextChunk` em ordem) — replicado aqui porque aquele acúmulo
//! morre dentro do turno, e este listener só enxerga o que passou pelo barramento.

/// Estado puro do acúmulo — sem `AppHandle`, sem barramento, testável isolado. Um turno de cada
/// vez: `ThinkingStart` reseta, `StreamChunk` acumula, `ThinkingStop` devolve o par pronto.
#[derive(Debug, Default)]
pub struct TurnAssembler {
    user_content: Option<String>,
    assistant_buffer: String,
    used_tools: bool,
}

impl TurnAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// `ThinkingStart{userContent}` — início de um turno novo. Um `ThinkingStart` sem o
    /// `ThinkingStop` anterior ter chegado (turno cancelado/abortado no meio) descarta o buffer
    /// órfão em vez de misturar dois turnos — mesma perda que o próprio TS aceita quando um turno
    /// é abortado (nenhum par incompleto vira memória).
    pub fn on_thinking_start(&mut self, user_content: &str) {
        self.user_content = Some(user_content.to_string());
        self.assistant_buffer.clear();
        self.used_tools = false;
    }

    /// `StreamChunk{text}` — um pedaço do texto da Mika, na ordem de chegada.
    pub fn on_stream_chunk(&mut self, text: &str) {
        self.assistant_buffer.push_str(text);
    }

    /// `ToolCall{}` — o turno atual chamou ao menos uma tool. `post_turn::pipeline` lê isto (via
    /// `on_thinking_stop`) para nunca marcar um turno com efeito colateral real como "claim sem
    /// tool" (ver `turn::action_claim`).
    pub fn on_tool_call(&mut self) {
        self.used_tools = true;
    }

    /// `ThinkingStop{}` — turno terminou. Devolve `Some((user, assistant, used_tools))` só quando
    /// um `ThinkingStart` abriu o par E o buffer de resposta não ficou vazio (turno que falhou
    /// antes de gerar texto, ou só usou tools sem texto final, não vira memória de conversa — nada
    /// para guardar). Sempre reseta o estado, sucesso ou não, para o próximo turno começar limpo.
    pub fn on_thinking_stop(&mut self) -> Option<(String, String, bool)> {
        let user = self.user_content.take()?;
        let assistant = std::mem::take(&mut self.assistant_buffer);
        let used_tools = std::mem::take(&mut self.used_tools);
        if assistant.trim().is_empty() {
            return None;
        }
        Some((user, assistant, used_tools))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_turn_produces_the_pair_in_order() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("oi mika");
        assembler.on_stream_chunk("Oi! ");
        assembler.on_stream_chunk("tudo bem?");
        let pair = assembler.on_thinking_stop();
        assert_eq!(pair, Some(("oi mika".to_string(), "Oi! tudo bem?".to_string(), false)));
    }

    /// O sinal que `post_turn::pipeline` precisa para nunca tratar um turno com tool real como
    /// "claim sem tool" (ver `turn::action_claim`) — `used_tools` viaja no par, não é perdido.
    #[test]
    fn a_turn_with_a_tool_call_carries_used_tools_true() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("abre a calculadora");
        assembler.on_tool_call();
        assembler.on_stream_chunk("Abri a calculadora!");
        let pair = assembler.on_thinking_stop();
        assert_eq!(pair, Some(("abre a calculadora".to_string(), "Abri a calculadora!".to_string(), true)));
    }

    /// `used_tools` reseta por turno, igual ao buffer de texto — um tool call do turno anterior
    /// não pode vazar para o próximo par.
    #[test]
    fn used_tools_resets_between_turns() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("um");
        assembler.on_tool_call();
        assembler.on_stream_chunk("resposta com tool");
        assembler.on_thinking_stop();

        assembler.on_thinking_start("dois");
        assembler.on_stream_chunk("resposta sem tool");
        let pair = assembler.on_thinking_stop();
        assert_eq!(pair, Some(("dois".to_string(), "resposta sem tool".to_string(), false)));
    }

    #[test]
    fn thinking_stop_without_a_prior_thinking_start_produces_nothing() {
        let mut assembler = TurnAssembler::new();
        assembler.on_stream_chunk("texto órfão, sem começo");
        assert_eq!(assembler.on_thinking_stop(), None);
    }

    /// Turno que só chamou tools, sem texto final (ou abortou antes do primeiro chunk) — nunca vira
    /// um par vazio na memória.
    #[test]
    fn an_empty_assistant_buffer_produces_nothing() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("oi");
        assert_eq!(assembler.on_thinking_stop(), None);
    }

    #[test]
    fn a_blank_only_assistant_buffer_also_produces_nothing() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("oi");
        assembler.on_stream_chunk("   \n  ");
        assert_eq!(assembler.on_thinking_stop(), None);
    }

    /// Um `ThinkingStart` novo antes do `ThinkingStop` do turno anterior (abort/cancelamento no
    /// meio) descarta o buffer órfão — não mistura dois turnos no mesmo par.
    #[test]
    fn a_new_thinking_start_discards_an_orphaned_buffer_from_an_aborted_turn() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("primeira pergunta");
        assembler.on_stream_chunk("resposta parcial que nunca fechou");
        assembler.on_thinking_start("segunda pergunta");
        assembler.on_stream_chunk("resposta completa");
        let pair = assembler.on_thinking_stop();
        assert_eq!(pair, Some(("segunda pergunta".to_string(), "resposta completa".to_string(), false)));
    }

    #[test]
    fn the_assembler_resets_between_turns_and_handles_the_next_one_independently() {
        let mut assembler = TurnAssembler::new();
        assembler.on_thinking_start("um");
        assembler.on_stream_chunk("resposta um");
        assembler.on_thinking_stop();

        assembler.on_thinking_start("dois");
        assembler.on_stream_chunk("resposta dois");
        let pair = assembler.on_thinking_stop();
        assert_eq!(pair, Some(("dois".to_string(), "resposta dois".to_string(), false)));
    }
}
