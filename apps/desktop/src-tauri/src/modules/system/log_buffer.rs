//! Ring buffer em memória para os `log::` do shell — alimenta `system:getRecentLogs` (`mod.rs`) e
//! o lote periódico de `log_stream.rs`, os dois lados do painel de Logs de Configurações
//! (`apps/desktop/src/renderer/src/components/settings/use-log-stream.ts`) que antes desta peça
//! só recebia `{ logs: [] }` — ver o "EM ABERTO" que este arquivo fecha em `mod.rs::get_recent_logs`.
//!
//! **`LogRingBuffer` é uma struct, não um `OnceLock` cru exposto** — os testes deste arquivo
//! criam a própria instância (`LogRingBuffer::new()`) em vez de compartilhar o singleton global.
//! `cargo test` roda todo teste do binário na mesma thread pool; um singleton batido por vários
//! testes de ring/redação/lote ao mesmo tempo seria o mesmo tipo de contaminação cross-teste que
//! `hud_bridge.rs`/`events::bus` já documentaram noutro contexto. Só [`push_from_log_record`],
//! [`snapshot`] e [`drain_pending`] (as três funções livres no fim do arquivo) tocam o singleton —
//! e só a fiação de produção (`lib.rs`, `log_stream.rs`) chama essas três.
//!
//! Segredo é redigido em [`super::log_redact::redact`] ANTES de entrar no buffer — nunca depois,
//! nunca condicional: um log já gravado com a chave em texto puro não vira seguro com um filtro na
//! saída, porque a exportação (`use-log-stream.ts::exportLogs`) copia `formatted` direto pro
//! arquivo `.txt` que o usuário salva.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::log_redact::redact;
use super::log_trace;

/// Mesmo teto que `logs-panel-filters.ts::MAX_LOGS` — o painel já trunca no cliente; o lado Rust
/// nunca guarda mais que isso, então a truncagem do cliente nunca escondeu um vazamento de memória
/// do lado nativo.
pub const MAX_LOGS: usize = 500;

/// Forma exata de `LogItem` (`packages/sdk/src/core/logger-format.ts`). `meta` fica de fora: é
/// payload estruturado livre do logger do renderer, sem equivalente nos `log::` do shell Rust — o
/// contrato aceita `logs: unknown[]`, então omitir esse campo opcional é válido. `traceId` (abaixo)
/// tem, sim, equivalente: [`super::log_trace::current`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LogRecord {
    pub timestamp: String,
    pub at: i64,
    pub level: String,
    pub scope: String,
    pub msg: String,
    pub formatted: String,
    pub audience: &'static str,
    #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// SHOULD de `specs/10-observability.md` ("Repetição"): linhas idênticas consecutivas ganham
    /// contador em vez de virar entrada nova. Campo extra e opcional — `logs: z.array(z.unknown())`
    /// em `desktop-ipc.ts` aceita qualquer forma, então nada do lado TS quebra por ele existir.
    /// Omitido quando `1` (a maioria dos logs nunca repete) para não inflar todo registro à toa.
    #[serde(rename = "repeatCount", skip_serializing_if = "is_one")]
    pub repeat_count: u32,
}

fn is_one(count: &u32) -> bool {
    *count == 1
}

fn level_name(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "error",
        log::Level::Warn => "warn",
        log::Level::Info => "info",
        // `LogLevel` do contrato TS só tem 4 níveis; `Trace` cai em `debug`, o mais próximo.
        log::Level::Debug | log::Level::Trace => "debug",
    }
}

/// `debug` é detalhe de máquina por convenção do próprio nível (ver "Níveis, com critério" em
/// `specs/10-observability.md`) — o único público que se importa com uma chamada IPC ou um cache
/// hit é quem vai ler o NDJSON para diagnosticar, não a pessoa. `info`/`warn`/`error` são o que a
/// spec chama de tier humano: evento que o usuário se importa, anomalia recuperável, falha real.
fn audience_for(level: log::Level) -> &'static str {
    if level >= log::Level::Debug {
        "ai"
    } else {
        "human"
    }
}

pub(super) fn build_record(level: log::Level, scope: &str, raw_msg: &str) -> LogRecord {
    let now = chrono::Local::now();
    let timestamp = now.format("%H:%M:%S%.3f").to_string();
    let level_str = level_name(level);
    let msg = redact(raw_msg);
    let formatted = format!("[{timestamp}][{scope}][{level_str}] {msg}");
    LogRecord {
        timestamp,
        at: now.timestamp_millis(),
        level: level_str.to_string(),
        scope: scope.to_string(),
        msg,
        formatted,
        audience: audience_for(level),
        trace_id: log_trace::current(),
        repeat_count: 1,
    }
}

/// Mesma chave que decide "é repetição": nível + escopo + mensagem (já redigida) iguais aos do
/// último registro. `trace_id` fica fora de propósito — duas chamadas idênticas em turnos
/// diferentes não são a mesma repetição estrutural, mas isso é refinamento futuro; o SHOULD pede
/// "linhas idênticas consecutivas", e a linha visível ao usuário é `formatted`/`msg`, não o turno.
fn same_line(a: &LogRecord, b: &LogRecord) -> bool {
    a.level == b.level && a.scope == b.scope && a.msg == b.msg
}

struct Inner {
    ring: VecDeque<LogRecord>,
    pending: VecDeque<LogRecord>,
}

pub struct LogRingBuffer {
    inner: Mutex<Inner>,
}

impl LogRingBuffer {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Inner { ring: VecDeque::new(), pending: VecDeque::new() }) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Redige, empurra no ring (teto [`MAX_LOGS`], descarta o mais antigo) e enfileira no lote
    /// pendente (mesmo teto — nunca ilimitado, mesmo se `log_stream::spawn` nunca drenar).
    ///
    /// Repetição consecutiva (`specs/10-observability.md`, "Repetição", SHOULD) não vira entrada
    /// nova: incrementa `repeat_count` na última entrada do ring E do lote pendente, em vez de
    /// duplicar a linha nos dois. Sem isto um provedor travado (`vision/watcher.rs` em loop, por
    /// exemplo) enche as 500 vagas do ring com a MESMA linha, expulsando histórico útil.
    /// Devolve o registro CRU recém-construído (antes de qualquer colapso de repetição) — quem
    /// chama para alimentar um sink que quer "um evento, uma linha" (`log_files::enqueue`, o tier
    /// `ai`, que quer TUDO por inteiro) usa o retorno; o ring/lote internos continuam colapsando
    /// repetição consecutiva do jeito que já colapsavam.
    pub fn push(&self, level: log::Level, scope: &str, raw_msg: &str) -> LogRecord {
        let record = build_record(level, scope, raw_msg);
        let raw = record.clone();
        let mut guard = self.lock();

        let is_repeat = guard.ring.back().is_some_and(|last| same_line(last, &record));
        if is_repeat {
            let updated = {
                let last = guard.ring.back_mut().expect("checado acima");
                last.repeat_count += 1;
                last.at = record.at;
                last.formatted = format!("{} (x{})", record.formatted, last.repeat_count);
                last.clone()
            };

            let pending_matches = guard.pending.back().is_some_and(|pending_last| same_line(pending_last, &record));
            if pending_matches {
                let pending_last = guard.pending.back_mut().expect("checado acima");
                pending_last.repeat_count = updated.repeat_count;
                pending_last.at = updated.at;
                pending_last.formatted = updated.formatted.clone();
            } else {
                // O lote pendente já foi drenado (`log_stream::spawn` rodou entre um push e o
                // outro) — sem entrada correspondente lá, o contador atualizado ainda precisa
                // chegar ao painel, então entra como registro próprio no lote.
                guard.pending.push_back(updated);
                if guard.pending.len() > MAX_LOGS {
                    guard.pending.pop_front();
                }
            }
            return raw;
        }

        guard.ring.push_back(record.clone());
        if guard.ring.len() > MAX_LOGS {
            guard.ring.pop_front();
        }
        guard.pending.push_back(record);
        if guard.pending.len() > MAX_LOGS {
            guard.pending.pop_front();
        }
        raw
    }

    pub fn push_from_log_record(&self, record: &log::Record) -> LogRecord {
        self.push(record.level(), record.target(), &record.args().to_string())
    }

    /// Conteúdo atual do ring, do mais antigo ao mais novo — é o que `system:getRecentLogs`
    /// devolve na carga inicial do painel.
    pub fn snapshot(&self) -> Vec<LogRecord> {
        self.lock().ring.iter().cloned().collect()
    }

    /// Esvazia e devolve o que chegou desde o último drain — chamada só pelo timer de
    /// `log_stream.rs`. Idempotente: chamar duas vezes seguidas sem `push` no meio devolve `[]` na
    /// segunda.
    pub fn drain_pending(&self) -> Vec<LogRecord> {
        self.lock().pending.drain(..).collect()
    }
}

impl Default for LogRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn global() -> &'static LogRingBuffer {
    static BUFFER: OnceLock<LogRingBuffer> = OnceLock::new();
    BUFFER.get_or_init(LogRingBuffer::new)
}

/// Chamada pelo `fern::Output::call` registrado em `lib.rs` — todo `log::log!` do processo passa
/// por aqui uma vez que o logger real está montado. Devolve o registro cru para o mesmo chamador
/// repassar a `log_files::enqueue` (tier `ai` em disco) sem reconstruir nada.
pub fn push_from_log_record(record: &log::Record) -> LogRecord {
    global().push_from_log_record(record)
}

/// Lido por `system::get_recent_logs`.
pub fn snapshot() -> Vec<LogRecord> {
    global().snapshot()
}

/// Lido pelo timer de `log_stream::spawn`.
pub fn drain_pending() -> Vec<LogRecord> {
    global().drain_pending()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_bound_evicts_the_oldest_entry_past_the_cap() {
        let buf = LogRingBuffer::new();
        for i in 0..(MAX_LOGS + 10) {
            buf.push(log::Level::Info, "scope", &format!("msg-{i}"));
        }
        let snap = buf.snapshot();
        assert_eq!(snap.len(), MAX_LOGS);
        // O mais antigo sobrevivente é o 11º empurrado (índices 0..10 foram descartados).
        assert_eq!(snap.first().unwrap().msg, "msg-10");
        assert_eq!(snap.last().unwrap().msg, format!("msg-{}", MAX_LOGS + 9));
    }

    /// SHOULD de "Repetição" (`specs/10-observability.md`): 5 linhas idênticas consecutivas viram
    /// UMA entrada no ring com `repeat_count == 5`, não cinco. Falhava contra o `push` antigo
    /// (`snap.len()` dava `5`, não `1`).
    #[test]
    fn identical_consecutive_lines_collapse_with_a_counter() {
        let buf = LogRingBuffer::new();
        for _ in 0..5 {
            buf.push(log::Level::Warn, "vision", "densidade abaixo do limiar");
        }
        let snap = buf.snapshot();
        assert_eq!(snap.len(), 1, "5 linhas idênticas deveriam colapsar em 1 entrada");
        assert_eq!(snap[0].repeat_count, 5);
        assert!(
            snap[0].formatted.ends_with("(x5)"),
            "formatted deveria carregar o contador visível: {}",
            snap[0].formatted
        );
    }

    /// Uma linha diferente entre duas repetições quebra a sequência — cada trecho colapsa
    /// separado, sem contaminar contador do outro.
    #[test]
    fn a_different_line_breaks_the_repeat_sequence() {
        let buf = LogRingBuffer::new();
        buf.push(log::Level::Info, "scope", "a");
        buf.push(log::Level::Info, "scope", "a");
        buf.push(log::Level::Info, "scope", "b");
        buf.push(log::Level::Info, "scope", "a");
        let snap = buf.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].msg, "a");
        assert_eq!(snap[0].repeat_count, 2);
        assert_eq!(snap[1].msg, "b");
        assert_eq!(snap[1].repeat_count, 1);
        assert_eq!(snap[2].msg, "a");
        assert_eq!(snap[2].repeat_count, 1);
    }

    /// O lote pendente (o que `log_stream::spawn` drena) também recebe o contador atualizado, não
    /// só o ring — senão o painel em tempo real mostraria a linha sem repetição nenhuma até o
    /// próximo `getRecentLogs`.
    #[test]
    fn repeat_counter_also_updates_in_the_pending_batch() {
        let buf = LogRingBuffer::new();
        buf.push(log::Level::Error, "llm", "provedor indisponível");
        buf.push(log::Level::Error, "llm", "provedor indisponível");
        buf.push(log::Level::Error, "llm", "provedor indisponível");
        let pending = buf.drain_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].repeat_count, 3);
    }

    #[test]
    fn pending_queue_is_also_bounded() {
        let buf = LogRingBuffer::new();
        for i in 0..(MAX_LOGS + 10) {
            buf.push(log::Level::Info, "scope", &format!("msg-{i}"));
        }
        assert_eq!(buf.drain_pending().len(), MAX_LOGS);
    }

    #[test]
    fn push_redacts_the_message_before_it_reaches_the_buffer() {
        let buf = LogRingBuffer::new();
        let key = format!("sk-ant-{}", "a".repeat(40));
        buf.push(log::Level::Error, "llm", &format!("falha ao autenticar com {key}"));
        let snap = buf.snapshot();
        assert_eq!(snap[0].msg, "falha ao autenticar com [KEY]");
        assert!(!snap[0].formatted.contains(&key));
    }

    #[test]
    fn record_matches_the_log_item_shape_the_renderer_expects() {
        let buf = LogRingBuffer::new();
        buf.push(log::Level::Warn, "voice", "buffer de áudio atrasado");
        let value = serde_json::to_value(&buf.snapshot()[0]).unwrap();
        let obj = value.as_object().unwrap();
        for field in ["timestamp", "at", "level", "scope", "msg", "formatted", "audience"] {
            assert!(obj.contains_key(field), "campo '{field}' ausente no LogRecord serializado");
        }
        assert_eq!(obj["level"], "warn");
        assert_eq!(obj["scope"], "voice");
        assert_eq!(obj["audience"], "human");
    }

    #[test]
    fn drain_pending_empties_the_queue_but_leaves_the_ring_intact() {
        let buf = LogRingBuffer::new();
        buf.push(log::Level::Info, "scope", "primeiro");
        buf.push(log::Level::Info, "scope", "segundo");

        let batch = buf.drain_pending();
        assert_eq!(batch.len(), 2);
        assert!(buf.drain_pending().is_empty(), "um segundo drain sem push no meio deveria vir vazio");
        assert_eq!(buf.snapshot().len(), 2, "drain do lote não pode apagar o ring");
    }

    #[test]
    fn push_from_log_record_builds_the_same_shape_as_push() {
        let buf = LogRingBuffer::new();
        let record = log::Record::builder()
            .level(log::Level::Debug)
            .target("chat")
            .args(format_args!("turno concluído"))
            .build();
        buf.push_from_log_record(&record);
        let snap = buf.snapshot();
        assert_eq!(snap[0].scope, "chat");
        assert_eq!(snap[0].level, "debug");
        assert_eq!(snap[0].msg, "turno concluído");
    }

    #[test]
    fn trace_level_folds_into_debug_the_only_matching_ts_level() {
        assert_eq!(level_name(log::Level::Trace), "debug");
    }

    #[test]
    fn debug_and_trace_are_the_ai_audience_the_rest_is_human() {
        let buf = LogRingBuffer::new();
        buf.push(log::Level::Debug, "vision", "screenshot capturado");
        buf.push(log::Level::Trace, "vision", "poll de ciclo");
        buf.push(log::Level::Info, "chat", "resposta recebida");
        buf.push(log::Level::Warn, "llm", "retry após falha");
        buf.push(log::Level::Error, "llm", "provedor desativado");
        let snap = buf.snapshot();
        assert_eq!(snap[0].audience, "ai");
        assert_eq!(snap[1].audience, "ai");
        assert_eq!(snap[2].audience, "human");
        assert_eq!(snap[3].audience, "human");
        assert_eq!(snap[4].audience, "human");
    }

    #[tokio::test]
    async fn trace_id_rides_along_with_every_record_emitted_inside_a_turn_scope() {
        let buf = LogRingBuffer::new();
        log_trace::scoped("t-turn-1".to_string(), async {
            buf.push(log::Level::Info, "chat", "turno concluído");
        })
        .await;
        buf.push(log::Level::Info, "chat", "fora de qualquer turno");
        let snap = buf.snapshot();
        assert_eq!(snap[0].trace_id.as_deref(), Some("t-turn-1"));
        assert_eq!(snap[1].trace_id, None);
    }
}
