//! `CapturingLogger` — implementa `boa_runtime::Logger` empurrando cada `console.*` para um
//! buffer em memória em vez de `stdout`/`stderr` (`DefaultLogger`, o padrão do crate, escreve lá,
//! e não há terminal nenhum lendo o processo da Mika). `sandbox.rs` é o único chamador: registra
//! um clone no `Context` e lê o original depois que `eval` retorna.
//!
//! `Rc<RefCell<...>>`, não `Arc<Mutex<...>>`: o `Context` inteiro (`sandbox.rs`) vive e morre
//! dentro de uma única thread dedicada — nunca atravessa fronteira de thread — então o custo de
//! sincronização atômica não compra nada aqui. Mesmo padrão de captura que o próprio `boa` usa em
//! `cli/src/logger.rs` (`SharedExternalPrinterLogger`), só que com um `Vec<String>` no lugar do
//! terminal externo.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{Context, Finalize, JsResult, Trace};
use boa_runtime::{ConsoleState, Logger};

#[derive(Clone, Trace, Finalize)]
pub(super) struct CapturingLogger {
    // `Rc<RefCell<...>>` não é rastreado pelo GC do boa (não é um `Gc<T>`) — `unsafe_ignore_trace`
    // é seguro porque não há ponteiro de GC escondido dentro dele, só `String`s comuns.
    #[unsafe_ignore_trace]
    lines: Rc<RefCell<Vec<String>>>,
}

impl CapturingLogger {
    pub(super) fn new() -> Self {
        Self { lines: Rc::new(RefCell::new(Vec::new())) }
    }

    /// Drena as linhas capturadas. Chamado uma vez, depois que `context.eval(...)` retorna — ver
    /// `sandbox.rs::eval_in_fresh_context`.
    pub(super) fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.lines.borrow_mut())
    }

    fn push(&self, msg: String) {
        self.lines.borrow_mut().push(msg);
    }
}

impl Logger for CapturingLogger {
    fn log(&self, msg: String, _state: &ConsoleState, _context: &mut Context) -> JsResult<()> {
        self.push(msg);
        Ok(())
    }

    fn info(&self, msg: String, _state: &ConsoleState, _context: &mut Context) -> JsResult<()> {
        self.push(format!("[info] {msg}"));
        Ok(())
    }

    fn warn(&self, msg: String, _state: &ConsoleState, _context: &mut Context) -> JsResult<()> {
        self.push(format!("[warn] {msg}"));
        Ok(())
    }

    fn error(&self, msg: String, _state: &ConsoleState, _context: &mut Context) -> JsResult<()> {
        self.push(format!("[error] {msg}"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prova o buffer em si, isolado do motor JS de verdade — `sandbox.rs` já cobre a integração
    /// ponta a ponta (`console.log` de dentro de um script real).
    #[test]
    fn captures_each_level_with_its_own_prefix_and_drains_on_take() {
        let logger = CapturingLogger::new();
        let mut context = Context::default();
        logger.log("hi".into(), &ConsoleState::default(), &mut context).unwrap();
        logger.info("there".into(), &ConsoleState::default(), &mut context).unwrap();
        logger.warn("careful".into(), &ConsoleState::default(), &mut context).unwrap();
        logger.error("boom".into(), &ConsoleState::default(), &mut context).unwrap();

        assert_eq!(logger.take(), vec!["hi", "[info] there", "[warn] careful", "[error] boom"]);
        // `take` esvazia — uma segunda leitura não repete o que já saiu.
        assert!(logger.take().is_empty());
    }
}
