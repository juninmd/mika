//! Roda um script JS num `boa_engine::Context` isolado, sem bridge de host nenhuma — só
//! `console` (`logger.rs`) entra no global object. Duas defesas independentes contra script
//! hostil ou quebrado, nenhuma delas o usuário aprovando algo:
//!
//! 1. **Limites de runtime do próprio `boa`** (`RuntimeLimits`, `MAX_*` abaixo): iteração de loop,
//!    profundidade de recursão, tamanho de pilha da VM. `IncrementLoopIteration` (`boa_engine`
//!    v0.22, `vm/opcode/iteration/loop_ops.rs`) soma no MESMO contador de frame a cada iteração de
//!    QUALQUER loop dentro da mesma chamada de função — loops aninhados no mesmo frame dividem um
//!    teto, não multiplicam. Recursão gera um frame novo por chamada, então o pior caso de OPCODE é
//!    `MAX_RECURSION_DEPTH × MAX_LOOP_ITERATIONS` operações — finito, sempre, mesmo sob abuso.
//!    NÃO cobre memória: builtins como `Array.prototype.join`/`toLocaleString` alocam
//!    `Vec::with_capacity` direto do `.length` do array (até `2^32-1`) num laço nativo que nunca
//!    passa por `IncrementLoopIteration` — invisível a este teto. Ver `mod.rs` (§"Por que roda COM
//!    aprovação") para o risco residual e por que isso NÃO é um "sempre finito" de verdade.
//!    **O teto de iteração é POR FRAME, não global**: `frame.loop_iteration_count` zera a cada
//!    chamada de função (provado em `the_iteration_budget_resets_on_every_function_call`), então
//!    `while(true){ g(); }` com `g` fazendo 900k voltas roda ~10^11 operações sem nunca estourar
//!    nada. O teto não limita o trabalho total — quem limita é o item 2.
//! 2. **Cancelamento cooperativo** (`run_with_timeout`): o script roda por
//!    `Script::evaluate_async_with_budget`, que devolve o controle a cada `YIELD_BUDGET` ciclos de
//!    VM. A cada devolução a própria thread confere o prazo e a bandeira de abandono e, se
//!    estourou, larga o `Context` e termina — a thread MORRE, não fica órfã. O chamador espera
//!    `timeout + GRACE`; só depois disso (código que não devolve o controle) ele desiste, marca a
//!    execução como abandonada e conta uma ÓRFÃ em `ORPHANS`: enquanto houver órfã, `run_js` recusa
//!    execução nova (`busy_refusal`), então um script hostil não acumula threads.
//!
//! **Limite que SOBRA e é conhecido**: laço dentro de builtin nativo (`Array.prototype.join` com
//! `length` enorme, `String.prototype.repeat`, regex com backtracking catastrófico, `sort`) nunca
//! devolve o controle à VM — nem o teto de iteração nem o cancelamento o alcançam. Esse caso vira
//! órfã (bloqueia novas execuções até a thread terminar sozinha) e, se alocar sem limite, ainda
//! pode abortar o processo por memória (ver `mod.rs`: é por isso que a tool exige aprovação).
//!
//! `Context` é `!Send` (interning via `Rc`) — por isso é construído E rodado inteiramente dentro
//! da própria thread spawnada, nunca fora dela.

use std::future::Future;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::task::Poll;
use std::thread;
use std::time::{Duration, Instant};

use boa_engine::{Context, JsResult, JsValue, Script, Source};
use boa_runtime::Console;

use super::logger::CapturingLogger;

const TIMEOUT_MS: u64 = 3_000;
/// Folga que o chamador dá à thread para ela mesma largar o script depois do prazo, antes de
/// abandoná-la como órfã. A thread confere o prazo a cada `YIELD_BUDGET` ciclos (sub-milissegundo).
const GRACE_MS: u64 = 750;
/// "Ciclos de relógio" de VM entre devoluções de controle — a granularidade do cancelamento.
const YIELD_BUDGET: u32 = 4_096;
/// Teto cumulativo de iterações de loop POR FRAME de chamada (ver o cabeçalho acima) — a defesa
/// primária contra `while(true){}`; o timeout de parede é o backstop do lado do chamador, não o
/// mecanismo que de fato para a thread.
const MAX_LOOP_ITERATIONS: u64 = 1_000_000;
/// Profundidade de chamada de função (mesmo default do próprio `boa`, nomeado aqui para que mudar
/// vire uma decisão, não uma herança silenciosa).
const MAX_RECURSION_DEPTH: usize = 512;
/// Slots da pilha de valores da VM do `boa` (unidade interna dela, não bytes de pilha do SO) —
/// mesmo default do crate.
const MAX_STACK_SIZE: usize = 1024 * 10;
/// Teto do texto combinado (stdout OU resultado OU erro, cada um por si) devolvido ao chamador —
/// um `console.log` dentro do teto de iteração acima ainda pode imprimir megabytes antes de
/// esbarrar nele.
const MAX_OUTPUT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunJsOutput {
    pub(super) stdout: String,
    pub(super) result: Option<String>,
    pub(super) error: Option<String>,
    pub(super) duration_ms: u64,
}

/// Threads de script que o chamador já abandonou e que ainda não terminaram.
static ORPHANS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
fn orphaned_executions() -> usize {
    ORPHANS.load(Ordering::SeqCst)
}

/// Estado compartilhado entre o chamador e a thread do script (uma transição só, por CAS).
const RUNNING: u8 = 0;
const ABANDONED: u8 = 1;
const FINISHED: u8 = 2;

/// Fecha a contabilidade da thread ao sair — por retorno OU por pânico (ver o uso em
/// [`run_on_thread`]). Terminou depois de abandonada: deixa de ser órfã.
struct SettleOnExit(Arc<AtomicU8>);

impl Drop for SettleOnExit {
    fn drop(&mut self) {
        if self.0.compare_exchange(RUNNING, FINISHED, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            ORPHANS.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn failed(error: String, duration_ms: u64) -> RunJsOutput {
    RunJsOutput { stdout: String::new(), result: None, error: Some(error), duration_ms }
}

/// Recusa execução nova enquanto houver órfã — função pura para o teste não precisar fabricar uma.
fn busy_refusal(orphans: usize) -> Option<RunJsOutput> {
    (orphans > 0).then(|| failed("sandbox ocupada: uma execução anterior ainda não terminou — tente de novo em instantes".to_string(), 0))
}

/// Executa `code` numa thread dedicada e devolve em até `TIMEOUT_MS` (+ `GRACE_MS` se a thread não
/// devolver o controle sozinha) — ver o cabeçalho do módulo.
pub(super) fn run_js(code: &str) -> RunJsOutput {
    run_with_timeout(code, Duration::from_millis(TIMEOUT_MS))
}

fn run_with_timeout(code: &str, timeout: Duration) -> RunJsOutput {
    let code = code.to_string();
    run_on_thread(timeout, move |deadline, state| eval_in_fresh_context(&code, deadline, state))
}

/// Roda `job` numa thread dedicada e espera `timeout + GRACE_MS`. Separado de
/// [`run_with_timeout`] para o contrato de ÓRFÃS (contagem, pânico) ser provado sem depender de um
/// script que quebre o `boa` de propósito.
fn run_on_thread(timeout: Duration, job: impl FnOnce(Instant, &AtomicU8) -> RunJsOutput + Send + 'static) -> RunJsOutput {
    if let Some(refused) = busy_refusal(ORPHANS.load(Ordering::SeqCst)) {
        return refused;
    }
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let state = Arc::new(AtomicU8::new(RUNNING));
    let thread_state = Arc::clone(&state);

    let spawned = thread::Builder::new().name("mika-code-sandbox".into()).spawn(move || {
        // `Drop` e não código depois do `send`: um pânico no `job` pula o que viria abaixo, e a
        // órfã contada pelo chamador nunca mais seria descontada (`code` recusado até reiniciar).
        let _settle = SettleOnExit(Arc::clone(&thread_state));
        let outcome = job(started + timeout, &thread_state);
        // Receptor pode já ter desistido — canal sem ninguém do outro lado só descarta o resultado.
        let _ = tx.send(outcome);
    });
    if let Err(error) = spawned {
        return failed(format!("não criou a thread da sandbox: {error}"), 0);
    }

    match rx.recv_timeout(timeout + Duration::from_millis(GRACE_MS)) {
        Ok(mut outcome) => {
            outcome.duration_ms = started.elapsed().as_millis() as u64;
            outcome
        }
        // Thread morreu sem responder (pânico no motor): não é órfã nem timeout.
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            failed("o motor de script falhou (pânico interno) — nada foi executado até o fim".to_string(), started.elapsed().as_millis() as u64)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Conta ANTES do CAS: a thread só decrementa se vir ABANDONED, que o CAS abaixo grava
            // depois deste incremento — o contador nunca fica negativo.
            ORPHANS.fetch_add(1, Ordering::SeqCst);
            if state.compare_exchange(RUNNING, ABANDONED, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                ORPHANS.fetch_sub(1, Ordering::SeqCst);
            }
            failed(format!("timeout: execução excedeu {}ms", timeout.as_millis()), started.elapsed().as_millis() as u64)
        }
    }
}

/// Dirige o script em fatias de `YIELD_BUDGET`: a cada fatia confere prazo e abandono. `None` =
/// cancelado (o `Context` é largado pelo chamador, a thread termina).
fn evaluate_cooperatively(context: &mut Context, code: &str, deadline: Instant, state: &AtomicU8) -> Option<JsResult<JsValue>> {
    let script = match Script::parse(Source::from_bytes(code), None, context) {
        Ok(script) => script,
        Err(error) => return Some(Err(error)),
    };
    let future = script.evaluate_async_with_budget(context, YIELD_BUDGET);
    let mut future = std::pin::pin!(future);
    let waker = futures::task::noop_waker();
    let mut task = std::task::Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut task) {
            Poll::Ready(result) => return Some(result),
            Poll::Pending if state.load(Ordering::SeqCst) == ABANDONED || Instant::now() >= deadline => return None,
            Poll::Pending => {}
        }
    }
}

fn eval_in_fresh_context(code: &str, deadline: Instant, state: &AtomicU8) -> RunJsOutput {
    let mut context = Context::default();
    context.runtime_limits_mut().set_loop_iteration_limit(MAX_LOOP_ITERATIONS);
    context.runtime_limits_mut().set_recursion_limit(MAX_RECURSION_DEPTH);
    context.runtime_limits_mut().set_stack_size_limit(MAX_STACK_SIZE);

    let logger = CapturingLogger::new();
    // Único global que este `Context` ganha além do que o próprio ECMAScript já define — nenhum
    // `require`/`fetch`/`process`/acesso a arquivo (ver o teste `no_access_to_host_objects` em
    // `tool.rs`).
    if let Err(e) = Console::register_with_logger(logger.clone(), &mut context) {
        return failed(e.to_string(), 0);
    }

    let eval_result = evaluate_cooperatively(&mut context, code, deadline, state);
    let stdout = cap(logger.take().join("\n"));

    match eval_result {
        None => RunJsOutput { stdout, result: None, error: Some("timeout: execução cancelada pelo prazo".to_string()), duration_ms: 0 },
        Some(Ok(value)) => {
            RunJsOutput { stdout, result: format_result(value, &mut context), error: None, duration_ms: 0 }
        }
        Some(Err(e)) => RunJsOutput { stdout, result: None, error: Some(cap(e.to_string())), duration_ms: 0 },
    }
}

/// `undefined`, uma função ou um valor não representável em JSON (BigInt, Symbol) viram `None` —
/// não há "resultado" útil para mostrar (ex.: um script que só faz atribuições).
fn format_result(value: JsValue, context: &mut Context) -> Option<String> {
    let json = value.to_json(context).ok().flatten()?;
    Some(cap(json.to_string()))
}

/// Trunca na fronteira de char válida mais próxima — nunca corta um byte no meio de um caractere
/// multi-byte (unicode é uma entrada esperada, não um caso extra).
fn cap(s: String) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s;
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... [truncado em {MAX_OUTPUT_BYTES} bytes]", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_capture_and_returned_value() {
        let out = run_js("console.log('hi'); console.info('there'); 42");
        assert!(out.stdout.contains("hi"));
        assert!(out.stdout.contains("[info] there"));
        assert_eq!(out.result, Some("42".to_string()));
        assert_eq!(out.error, None);
    }

    #[test]
    fn syntax_error_is_reported_not_panicked() {
        let out = run_js("function( {");
        assert!(out.error.is_some());
        assert_eq!(out.result, None);
    }

    #[test]
    fn infinite_loop_is_stopped_by_the_iteration_limit_well_under_the_wall_clock_timeout() {
        let out = run_js("while (true) {}");
        assert!(out.error.as_deref().is_some_and(|e| e.to_lowercase().contains("iteration")));
        // Prova que quem parou foi o teto de iteração, não o timeout de 3s (backstop diferente).
        assert!(out.duration_ms < TIMEOUT_MS, "duration_ms={}", out.duration_ms);
    }

    #[test]
    fn recursion_without_a_base_case_hits_the_recursion_limit() {
        let out = run_js("function f(n){ return f(n+1); } f(0);");
        assert!(out.error.as_deref().is_some_and(|e| e.to_lowercase().contains("recursive")));
    }

    #[test]
    fn console_output_is_capped_around_8kb() {
        let out = run_js("console.log('x'.repeat(50000));");
        assert!(out.stdout.len() < 50_000);
        assert!(out.stdout.ends_with("[truncado em 8192 bytes]"));
    }

    /// Contrato de segurança do módulo inteiro: nenhuma ponte de host, nem por acidente.
    #[test]
    fn no_access_to_host_objects() {
        let out = run_js("[typeof require, typeof process, typeof fetch, typeof __dirname].join(',')");
        assert_eq!(out.result, Some("\"undefined,undefined,undefined,undefined\"".to_string()));
        assert_eq!(out.error, None);
    }

    /// `RuntimeLimits::loop_iteration` é POR FRAME de chamada, não global: laços aninhados no mesmo
    /// frame dividem o teto...
    #[test]
    fn nested_loops_in_the_same_frame_share_one_iteration_budget() {
        let out = run_js("let n = 0; for (let i = 0; i < 2000; i++) { for (let j = 0; j < 2000; j++) { n++; } } n");
        assert!(out.error.as_deref().is_some_and(|e| e.to_lowercase().contains("iteration")), "{out:?}");
    }

    /// ...mas cada CHAMADA ganha um frame novo com o contador zerado: 5 chamadas de 900k iterações
    /// (4,5M no total, acima do teto) passam. É por isso que o teto de iteração NÃO limita o trabalho
    /// total, e o cancelamento cooperativo abaixo é que garante o fim da thread.
    #[test]
    fn the_iteration_budget_resets_on_every_function_call() {
        // Prazo folgado de propósito: o que se afirma aqui é o ORÇAMENTO DE ITERAÇÃO, não o prazo de
        // parede (3 s em produção), e um runner compartilhado e carregado estoura 3 s em 4,5M de iterações.
        let out = run_with_timeout(
            "function g(){ let k = 0; for (let i = 0; i < 900000; i++) { k++; } return k; } let t = 0; for (let j = 0; j < 5; j++) { t += g(); } t",
            Duration::from_secs(60),
        );
        assert_eq!(out.error, None, "{out:?}");
        assert_eq!(out.result, Some("4500000".to_string()));
    }

    /// Trabalho que o teto de iteração NÃO pega (laço externo chama função, cada chamada zera o
    /// contador): o timeout tem de PARAR a thread, não só abandoná-la.
    #[test]
    fn a_timed_out_script_that_evades_the_loop_limit_is_actually_stopped() {
        let out = run_with_timeout(
            "function g(){ for (let i = 0; i < 900000; i++) {} } while (true) { g(); }",
            Duration::from_millis(300),
        );
        assert!(out.error.as_deref().is_some_and(|e| e.contains("timeout")), "{out:?}");
        wait_until_no_orphans();
        assert_eq!(orphaned_executions(), 0, "a thread do script continuou rodando depois do timeout");
    }

    /// Enquanto houver thread órfã (script que o chamador abandonou e ainda não parou), nada novo roda.
    #[test]
    fn a_new_execution_is_refused_while_an_orphan_is_still_running() {
        let refused = busy_refusal(1).expect("com órfã tem de recusar");
        assert!(refused.error.as_deref().is_some_and(|e| e.contains("ocupada")), "{refused:?}");
        assert_eq!(refused.result, None);
        assert!(busy_refusal(0).is_none());
    }

    /// Pânico na thread do script (bug do motor): sem o guard, a órfã contada pelo chamador ficava
    /// em 1 para sempre e todo `code` seguinte era recusado ("sandbox ocupada") até reiniciar o app.
    #[test]
    fn a_panicking_script_thread_does_not_leave_an_orphan_that_blocks_later_runs() {
        let out = run_on_thread(Duration::from_millis(100), |_, _| panic!("falha injetada no motor"));
        assert!(out.error.is_some(), "{out:?}");
        wait_until_no_orphans();
        assert_eq!(orphaned_executions(), 0, "a thread que entrou em pânico ficou contada como órfã");
        let next = run_js("1 + 1");
        assert_eq!(next.result, Some("2".to_string()), "{next:?}");
    }

    fn wait_until_no_orphans() {
        for _ in 0..100 {
            if orphaned_executions() == 0 {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}
