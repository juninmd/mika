//! Sinks de arquivo dos dois tiers pedidos por `specs/10-observability.md` ("Dois públicos, dois
//! tiers"/"Persistência"): `logs/mika-ai.ndjson` (tier `ai`, TUDO, uma linha JSON por evento) e
//! `logs/mika-human.log` (tier `human`, só o subconjunto que `log_buffer::audience_for` já marca
//! como `"human"`). Os dois arquivos eram do processo Node e saíram por inteiro no Stage C da
//! migração Tauri (2026-09-22); este módulo os reconstrói do lado do shell Rust a partir do MESMO
//! [`LogRecord`] que já alimenta o ring buffer — nenhuma lógica de redação/nível/`traceId`
//! duplicada, e o MUST "o tier humano entra TAMBÉM no arquivo de IA" cai de graça: as duas escritas
//! partem do mesmo registro.
//!
//! **Fora do caminho quente.** [`enqueue`] só empurra numa fila LIMITADA (descarta o mais antigo e
//! conta) — nunca escreve em disco na thread que chamou `log::log!`. [`spawn`] inicia uma thread
//! dedicada que mantém os arquivos abertos e escreve; não ocupa worker do runtime async.
//! No encerramento, [`flush`] espera o escritor esvaziar a fila (com teto) — o lote final não morre
//! com o processo.
//! Um evento perdido entre o boot e o primeiro `spawn` (ou se `spawn` nunca for chamado, como em
//! teste) é aceitável — o ring buffer em memória (`log_buffer`) já cobre esse instante, e nenhum
//! `log::log!` do processo passa a depender de disco para não travar.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

use super::log_buffer::LogRecord;

const HUMAN_FILE: &str = "mika-human.log";
const AI_FILE: &str = "mika-ai.ndjson";

/// Teto por arquivo antes de rotacionar: o arquivo cheio vira `<nome>.1` (sobrescrevendo a rotação
/// anterior) e um novo começa — o último trecho de log sobrevive, em vez de ser zerado justo quando
/// alguém vai investigar um problema. Dois arquivos de 10 MB por tier é o histórico máximo em disco.
const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Fila entre `log::log!` e o escritor. Limitada: um provedor em laço de erro gera log mais rápido
/// que o disco e uma fila sem limite viraria vazamento de memória. Cheia, descarta o MAIS ANTIGO e
/// conta (o escritor registra quantos se perderam).
const QUEUE_CAP: usize = 4096;

struct Queue {
    state: Mutex<State>,
    /// Acorda o escritor (há registro) e quem espera no `flush` (o escritor terminou o lote).
    ready: Condvar,
    idle: Condvar,
    dropped: AtomicU64,
    cap: usize,
}

struct State {
    items: VecDeque<LogRecord>,
    /// O escritor já tirou um lote da fila e ainda não terminou de gravá-lo: fila vazia NÃO significa
    /// "em disco" enquanto isto for `true` — é por isso que o `flush` espera as duas coisas.
    writing: bool,
}

impl Queue {
    fn new(cap: usize) -> Self {
        Self {
            state: Mutex::new(State { items: VecDeque::new(), writing: false }),
            ready: Condvar::new(),
            idle: Condvar::new(),
            dropped: AtomicU64::new(0),
            cap,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn push(&self, record: LogRecord) {
        let mut state = self.lock();
        if state.items.len() >= self.cap {
            state.items.pop_front();
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        state.items.push_back(record);
        self.ready.notify_one();
    }

    /// Bloqueia até haver registro e devolve o lote inteiro (marcando o escritor como ocupado até
    /// [`Self::finish_batch`]).
    fn wait_batch(&self) -> Vec<LogRecord> {
        let mut state = self.lock();
        while state.items.is_empty() {
            state = self.ready.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        state.writing = true;
        state.items.drain(..).collect()
    }

    fn finish_batch(&self) {
        self.lock().writing = false;
        self.idle.notify_all();
    }

    /// Espera (no máximo `timeout`) a fila esvaziar E o último lote chegar ao disco. Chamado no
    /// encerramento: sem isto o que foi logado nos últimos milissegundos — a razão da saída, um
    /// panic — morria na fila junto com o processo.
    fn flush(&self, timeout: std::time::Duration) {
        let deadline = std::time::Instant::now() + timeout;
        let mut state = self.lock();
        while !state.items.is_empty() || state.writing {
            let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) else { return };
            state = self.idle.wait_timeout(state, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

static QUEUE: OnceLock<Queue> = OnceLock::new();

/// Arquivo de log com o handle ABERTO (abrir/fechar a cada linha era um `open` por evento) e rotação
/// por tamanho.
struct Sink {
    path: PathBuf,
    file: Option<File>,
    size: u64,
    max: u64,
}

impl Sink {
    fn new(path: PathBuf, max: u64) -> Self {
        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        Self { path, file: None, size, max }
    }

    /// "Apagar meus dados" remove `logs/` com o handle aberto: no Linux/macOS o `Sink` seguiria
    /// escrevendo no inode desvinculado (o log da sessão some), no Windows toma ACCESS_DENIED. Um
    /// `stat` por lote basta para notar e reabrir (recriando a pasta) no lote seguinte.
    fn revalidate(&mut self) {
        if self.file.is_some() && !self.path.exists() {
            self.file = None;
            self.size = 0;
        }
    }

    fn append(&mut self, line: &str) {
        if self.size > self.max {
            self.rotate();
        }
        if self.file.is_none() {
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            self.file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        let Some(file) = self.file.as_mut() else { return };
        if writeln!(file, "{line}").is_ok() {
            self.size += line.len() as u64 + 1;
        } else {
            self.file = None;
        }
    }

    fn rotate(&mut self) {
        self.file = None;
        let mut rotated = self.path.as_os_str().to_owned();
        rotated.push(".1");
        let rotated = PathBuf::from(rotated);
        let _ = std::fs::remove_file(&rotated);
        if std::fs::rename(&self.path, &rotated).is_err() {
            let _ = std::fs::remove_file(&self.path);
        }
        self.size = 0;
    }
}

/// Chamado no encerramento (`RunEvent::Exit`/saída por panic): bloqueia até 1,5 s esperando o escritor
/// gravar o que já está na fila. Sem `spawn` rodado é no-op.
pub fn flush() {
    if let Some(queue) = QUEUE.get() {
        queue.flush(std::time::Duration::from_millis(1500));
    }
}

/// Chamado uma vez, no `setup` de `lib.rs`, com o diretório de dados JÁ resolvido
/// (`state::app_data_dir`, que respeita `MIKA_PROFILE_DIR` — nunca o caminho real do SO direto, a
/// mesma regra que todo outro sink de arquivo deste processo segue). Uma segunda chamada (hot
/// reload, teste) é ignorada em vez de panicar: perder o segundo sink de arquivo é melhor do que
/// derrubar o processo por causa de log.
///
/// O escritor é uma THREAD dedicada, não uma task async: `write` em disco é bloqueante e não pode
/// ocupar um worker do runtime (nem a task que resolve `mika_invoke`).
pub fn spawn(data_dir: PathBuf) {
    if QUEUE.set(Queue::new(QUEUE_CAP)).is_err() {
        return;
    }
    let dir = data_dir.join("logs");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::error!(target: "system", "logs: não consegui criar {} ({error})", dir.display());
        return;
    }
    let spawned = std::thread::Builder::new().name("log-files".into()).spawn(move || {
        let queue = QUEUE.get().expect("acabou de ser inicializada");
        let mut ai = Sink::new(dir.join(AI_FILE), MAX_FILE_BYTES);
        let mut human = Sink::new(dir.join(HUMAN_FILE), MAX_FILE_BYTES);
        loop {
            write_batch(queue, &mut ai, &mut human);
        }
    });
    if let Err(error) = spawned {
        log::error!(target: "system", "logs: não consegui iniciar o escritor ({error})");
    }
}

/// Enfileira um registro para os dois arquivos — chamado pelo MESMO hook `fern::Output::call`
/// (`lib.rs`) que já alimenta `log_buffer::push_from_log_record`. Nunca bloqueia quem chamou
/// `log::log!` (só um lock curto numa fila limitada); sem [`spawn`] ainda rodado (ou nunca, em
/// teste), o registro é descartado sem erro — o ring buffer em memória cobre esse instante.
pub fn enqueue(record: LogRecord) {
    if let Some(queue) = QUEUE.get() {
        queue.push(record);
    }
}

fn write_batch(queue: &Queue, ai: &mut Sink, human: &mut Sink) {
    let batch = queue.wait_batch();
    ai.revalidate();
    human.revalidate();
    let dropped = queue.dropped.swap(0, Ordering::Relaxed);
    if dropped > 0 {
        let notice = super::log_buffer::build_record(log::Level::Warn, "system", &format!("logs: {dropped} registros descartados (fila cheia)"));
        write_record(ai, human, &notice);
    }
    for record in &batch {
        write_record(ai, human, record);
    }
    queue.finish_batch();
}

/// Ponto único de escrita, testável sem thread/`AppHandle`: o `ai` sempre recebe a linha; o
/// `human` só quando `record.audience == "human"` — MUST de "Dois públicos, dois tiers" ("o tier
/// humano entra TAMBÉM no arquivo de IA") satisfeito por escrever os dois a partir do mesmo record,
/// nunca por sincronizar dois caminhos separados depois.
fn write_record(ai: &mut Sink, human: &mut Sink, record: &LogRecord) {
    if let Ok(line) = serde_json::to_string(record) {
        ai.append(&line);
    }
    if record.audience == "human" {
        human.append(&record.formatted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-log-files-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sinks(dir: &TempDir) -> (Sink, Sink) {
        (Sink::new(dir.0.join("ai.ndjson"), MAX_FILE_BYTES), Sink::new(dir.0.join("human.log"), MAX_FILE_BYTES))
    }

    fn record(level: &str, audience: &'static str, msg: &str) -> LogRecord {
        LogRecord {
            timestamp: "00:00:00.000".to_string(),
            at: 0,
            level: level.to_string(),
            scope: "test".to_string(),
            msg: msg.to_string(),
            formatted: format!("[00:00:00.000][test][{level}] {msg}"),
            audience,
            trace_id: None,
            repeat_count: 1,
        }
    }

    /// MUST ("Dois públicos, dois tiers"): tier humano entra TAMBÉM no arquivo de IA — o oposto
    /// (`ai` sem o `human`) esvaziaria o NDJSON toda vez que uma linha fosse `human`.
    #[test]
    fn a_human_record_lands_in_both_files() {
        let dir = TempDir::new();
        let (mut ai_sink, mut human_sink) = sinks(&dir);
        let (ai, human) = (dir.0.join("ai.ndjson"), dir.0.join("human.log"));
        write_record(&mut ai_sink, &mut human_sink, &record("warn", "human", "caí para o Ollama local"));

        let ai_content = std::fs::read_to_string(&ai).unwrap();
        let human_content = std::fs::read_to_string(&human).unwrap();
        assert!(ai_content.contains("caí para o Ollama local"), "ai.ndjson: {ai_content}");
        assert!(human_content.contains("caí para o Ollama local"), "human.log: {human_content}");
    }

    /// Um evento `ai`-only (debug/trace) não pertence ao tier humano — só o NDJSON recebe.
    #[test]
    fn an_ai_only_record_never_reaches_the_human_file() {
        let dir = TempDir::new();
        let (mut ai_sink, mut human_sink) = sinks(&dir);
        let (ai, human) = (dir.0.join("ai.ndjson"), dir.0.join("human.log"));
        write_record(&mut ai_sink, &mut human_sink, &record("debug", "ai", "chamada IPC de rotina"));

        let ai_content = std::fs::read_to_string(&ai).unwrap();
        assert!(ai_content.contains("chamada IPC de rotina"));
        assert!(!human.exists(), "arquivo humano não deveria existir sem nenhuma linha humana");
    }

    /// Cada evento é UMA linha JSON válida — o painel/o agente de IA leem `mika-ai.ndjson` linha a
    /// linha, sem parser de streaming multi-linha.
    #[test]
    fn ndjson_lines_parse_independently() {
        let dir = TempDir::new();
        let (mut ai_sink, mut human_sink) = sinks(&dir);
        let ai = dir.0.join("ai.ndjson");
        for i in 0..3 {
            write_record(&mut ai_sink, &mut human_sink, &record("info", "human", &format!("evento {i}")));
        }
        let content = std::fs::read_to_string(&ai).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("cada linha é JSON válido isolado");
            assert!(parsed["msg"].is_string());
        }
    }

    /// PRIVACY.md §2/§6 prometem chave e token redigidos nos arquivos de log: o registro que chega
    /// a `write_record` sai de `LogRingBuffer::push` (que redige em `build_record`), e é por esse
    /// caminho — o mesmo do `fern` em `lib.rs` — que o segredo tem de sumir dos DOIS arquivos.
    #[test]
    fn a_secret_logged_through_the_pipeline_never_reaches_either_file() {
        let dir = TempDir::new();
        let (mut ai_sink, mut human_sink) = sinks(&dir);
        let (ai, human) = (dir.0.join("ai.ndjson"), dir.0.join("human.log"));
        let key = format!("sk-or-v1-{}", "a1b2c3d4".repeat(8));
        let buffer = super::super::log_buffer::LogRingBuffer::new();

        let raw = buffer.push(log::Level::Warn, "llm", &format!("openrouter: 401 chave {key}; Authorization: Bearer tok_123.abc"));
        write_record(&mut ai_sink, &mut human_sink, &raw);

        for (name, path) in [("mika-ai.ndjson", &ai), ("mika-human.log", &human)] {
            let content = std::fs::read_to_string(path).unwrap();
            assert!(!content.contains(&key) && !content.contains("tok_123"), "{name} vazou segredo: {content}");
            assert!(content.contains("[KEY]"), "{name}: {content}");
        }
    }

    /// Passar do teto ROTACIONA: o conteúdo antigo vira `.1` e o arquivo novo só tem a linha nova.
    /// (Antes zerava o arquivo — o trecho de log de antes do problema sumia junto.)
    #[test]
    fn a_file_past_the_size_cap_rotates_to_dot_one_instead_of_being_zeroed() {
        let dir = TempDir::new();
        let path = dir.0.join("capped.log");
        let mut sink = Sink::new(path.clone(), 100);
        sink.append(&"a".repeat(150));
        sink.append("linha nova");

        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "linha nova");
        assert_eq!(std::fs::read_to_string(dir.0.join("capped.log.1")).unwrap().trim(), "a".repeat(150));

        sink.append(&"b".repeat(150));
        sink.append("outra");
        assert_eq!(std::fs::read_to_string(dir.0.join("capped.log.1")).unwrap().trim(), format!("linha nova\n{}", "b".repeat(150)), ".1 guarda só a rotação mais recente");
    }

    /// Fila cheia descarta o MAIS ANTIGO e conta — nunca cresce sem limite nem bloqueia quem loga.
    #[test]
    fn a_full_queue_drops_the_oldest_and_counts() {
        let queue = Queue::new(3);
        for i in 0..5 {
            queue.push(record("info", "ai", &format!("evento {i}")));
        }
        assert_eq!(queue.dropped.load(Ordering::Relaxed), 2);
        let msgs: Vec<String> = queue.wait_batch().into_iter().map(|r| r.msg).collect();
        assert_eq!(msgs, ["evento 2", "evento 3", "evento 4"]);
    }

    /// O descarte aparece no log em vez de sumir em silêncio.
    #[test]
    fn the_writer_reports_how_many_records_were_dropped() {
        let dir = TempDir::new();
        let (mut ai, mut human) = sinks(&dir);
        let queue = Queue::new(1);
        queue.push(record("info", "ai", "um"));
        queue.push(record("info", "ai", "dois"));
        write_batch(&queue, &mut ai, &mut human);
        let content = std::fs::read_to_string(dir.0.join("ai.ndjson")).unwrap();
        assert!(content.contains("1 registros descartados"), "{content}");
        assert!(content.contains("dois") && !content.contains("\"um\""), "{content}");
    }

    /// "Apagar meus dados" apaga `logs/` com o `Sink` ainda de arquivo aberto: no Linux/macOS ele
    /// seguia escrevendo no inode desvinculado (o log da sessão sumia), no Windows dava ACCESS_DENIED.
    /// O lote seguinte tem de recriar o arquivo e continuar escrevendo nele.
    #[test]
    fn a_log_file_wiped_while_open_is_recreated_by_the_next_batch() {
        let dir = TempDir::new();
        let logs = dir.0.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let mut ai = Sink::new(logs.join("ai.ndjson"), MAX_FILE_BYTES);
        let mut human = Sink::new(logs.join("human.log"), MAX_FILE_BYTES);
        let queue = Queue::new(8);
        queue.push(record("info", "human", "antes do wipe"));
        write_batch(&queue, &mut ai, &mut human);

        std::fs::remove_dir_all(&logs).unwrap();
        queue.push(record("info", "human", "depois do wipe"));
        write_batch(&queue, &mut ai, &mut human);

        let ai_content = std::fs::read_to_string(logs.join("ai.ndjson")).expect("o arquivo de log tem de voltar a existir");
        assert!(ai_content.contains("depois do wipe") && !ai_content.contains("antes do wipe"), "{ai_content}");
        assert!(std::fs::read_to_string(logs.join("human.log")).unwrap().contains("depois do wipe"));
    }

    /// `flush` é o que o encerramento chama: o que já estava na fila quando o app começou a sair
    /// tem de estar em disco quando ele volta, mesmo com o escritor ainda no meio do lote.
    #[test]
    fn flush_returns_only_after_the_writer_put_the_queued_records_on_disk() {
        let dir = TempDir::new();
        let (mut ai, mut human) = sinks(&dir);
        let queue = std::sync::Arc::new(Queue::new(8));
        let writer = {
            let queue = std::sync::Arc::clone(&queue);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(150));
                write_batch(&queue, &mut ai, &mut human);
            })
        };
        queue.push(record("info", "ai", "lote final"));

        queue.flush(std::time::Duration::from_secs(5));

        let content = std::fs::read_to_string(dir.0.join("ai.ndjson")).unwrap_or_default();
        assert!(content.contains("lote final"), "flush voltou antes de o lote ir ao disco: {content:?}");
        writer.join().unwrap();
    }

    /// `enqueue` sem `spawn` (cenário de teste/boot cedo) não bloqueia nem entra em pânico — o
    /// canal simplesmente não tem receptor ainda.
    #[test]
    fn enqueue_without_spawn_does_not_panic() {
        enqueue(record("info", "human", "sem sink ainda"));
    }
}
