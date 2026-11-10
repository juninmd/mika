//! Módulo `memory` em Rust — port do backend TypeScript correspondente.
//!
//! ## Isto é infraestrutura, não um canal de IPC
//! Ao contrário dos módulos vizinhos, `memory` quase não tem rota própria: uma busca em
//! `packages/sdk/src/contracts/desktop-ipc*.ts` (todo o arquivo, não só `desktop-ipc.ts`) não acha
//! nenhum canal `memory:*` — só o tipo `MemoryEntry` e os caminhos de config `memory.*`
//! (`packages/sdk/src/contracts/schemas/config-keys.enum.ts`). `modules()` continua `vec![]` de
//! propósito: não há nada aqui para `bridge.rs` encaminhar ou reivindicar. O valor desta frente é
//! a CAMADA que `chat`/`brain` vão chamar direto em Rust, sem passar por IPC nenhum — ver
//! `docs/tauri-rust-migration.md`, fase 3.
//!
//! ## O que foi portado
//! `EmbeddedMemoryStore` + `embedded-store-sql.ts`
//! (`apps/desktop/src/main/services/memory/{embedded-store,embedded-store-sql}.ts`): o backend
//! "embarcado" (sem Redis) da memória — uma única tabela `memory_entries(key, field, value,
//! score, expires_at)` que imita o subconjunto do Redis que `episodic`/`social` usam do lado
//! TypeScript (string, hash, set, sorted set, TTL, scan por cursor). É o alicerce citado na tabela
//! de fases ("banco; decide o armazenamento antes de quem escreve nele"): a lógica de cima (busca
//! por embedding em `episodic-search.ts`, grafo social em `social/*`, consolidação em
//! `consolidator.ts`) ainda mora no sidecar e migra numa fase futura, depois que `llm`/`chat`
//! existirem em Rust — ela não faz sentido isolada deles.
//!
//! `EmbeddedMemoryStore` foi escolhido, e não o backend Redis (`redis-client.ts`) nem as camadas
//! `episodic`/`social`, porque é o único pedaço AUTOCONTIDO: não depende de um servidor Redis
//! externo, não depende de embeddings, e do `ConfigStore` só precisa de dois campos de TTL.
//!
//! ## Decisão: `rusqlite` (bundled), não `libsql`
//! O `Cargo.toml` já declara os dois candidatos possíveis na tabela de equivalências
//! (`docs/tauri-rust-migration.md` §1) como decisão aberta; hoje só `rusqlite = "0.40"` (feature
//! `bundled`) está de fato listado nas dependências. A escolha, com evidência lida no TypeScript:
//!
//! 1. **O arquivo em disco já é SQLite puro, sem extensão proprietária.** O TypeScript abre o
//!    banco via `@libsql/client` em modo `file:` local (`packages/sdk/src/services/database.ts`),
//!    mas `embedded-store-sql.ts` só usa DDL/DML padrão (`TEXT`/`REAL`/`INTEGER`,
//!    `INSERT OR REPLACE`, `INSERT OR IGNORE`) — nenhum `F32_BLOB`, `vector_distance_cos` ou
//!    qualquer outra função só-libSQL (busquei em todo `services/memory/`: zero ocorrências fora
//!    de nomes de variável). O modo `file:` do libSQL é justamente o "SQLite local", byte
//!    compatível quando nenhuma extensão proprietária é usada — é por isso que abrir o MESMO
//!    arquivo com `rusqlite` não perde nem corrompe a memória de quem já tinha o app instalado.
//!    O mesmo padrão se repete fora deste módulo: `chat/history.service.ts` (outra frente da
//!    migração) usa `@libsql/client` só como camada de persistência por trás de DDL/DML padrão
//!    (`messages-table.ts`) — o histórico em si vive em memória no processo, e o SQL gravado é
//!    puro também. Nenhum consumidor de `@libsql/client` nesta árvore usa uma função só-libSQL.
//! 2. **`libsql` (crate Rust) está em pré-lançamento** (`0.10.0-pre.4`, conferido em
//!    `docs/tauri-rust-migration.md` §1) — API instável, risco desnecessário para uma tabela
//!    chave/campo/valor que não usa nada específico de libSQL.
//! 3. **`rusqlite` com `bundled` já compila** (usado por `token_usage.db` em `telemetry.rs`, sem
//!    depender de SQLite do sistema) — mesmo padrão já provado nesta árvore, multiplataforma.
//! 4. O tradeoff aceito: se um módulo futuro precisar de busca vetorial nativa (KNN em SQL, hoje
//!    feita em memória por `episodic-tier-scoring.ts`), `rusqlite` sozinho não oferece — nesse
//!    ponto listar `sqlite-vec` (extensão carregável, compatível com `rusqlite`) é a rota, não
//!    trocar para `libsql`.
//!
//! ## Compatibilidade com o arquivo existente do usuário
//! O caminho reproduzido é `<app_data_dir>/plugins/mika.brain/memory.db`, o mesmo que
//! `bootstrapBrainServices` abre hoje (`db.getDatabase('memory.db', 'plugins/mika.brain')` em
//! `apps/desktop/src/main/services/bootstrap/brain.ts`). `CREATE TABLE IF NOT EXISTS` e
//! `CREATE INDEX IF NOT EXISTS` tornam a abertura idempotente e reversível: reabrir um banco já
//! populado pelo TypeScript não apaga nem recria nada, e abrir de novo pelo TypeScript depois de
//! ter passado por aqui continua funcionando — nenhum dos dois lados migra o schema, os dois só
//! garantem que ele existe.
//!
//! ## O que ficou de fora (deliberado, não regressão silenciosa)
//! - `ping`/`quit`/`connect`/`on` do `RedisLike` do TypeScript existem só para o
//!   `EmbeddedMemoryStore` conseguir substituir o cliente Redis de verdade sem o chamador saber a
//!   diferença. Não há esse chamador em Rust ainda — fica de fora até existir.
//! - `sAdd`/`sMembers`/`scan`: sem chamador em Rust — mesmo critério do item acima.
//! - `keys`/`hGetAll` continuam SEM `LIMIT`, igual ao original: é o mesmo contrato do Redis
//!   (`KEYS`/`HGETALL` também não paginam). Quem varre volume alto usa `zRangeByScore` com
//!   `LIMIT`/`OFFSET`. Trocar o contrato aqui seria divergir do que `chat`/`brain` esperam chamar.
//! - Faixa de score em texto no estilo Redis (`"(5"` = exclusivo) não é modelada: nenhum chamador
//!   real em `services/memory/` passa isso (só `0`, `'-inf'`, `'+inf'` — conferido por busca);
//!   `ScoreBound` cobre exatamente esse uso real, com `Score`/`NegInf`/`PosInf` explícitos em vez
//!   de string mágica.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::AppHandle;

pub(crate) mod episodic;
pub mod experience;
mod live_config;
mod recording;
pub(crate) mod redact;
mod settings;
pub(crate) mod social;
mod text;

use crate::native::NativeModule;

/// = `recording-allowed.ts` — reexportado porque `recording` é um submódulo privado deste módulo
/// (só visível daqui e de descendentes). `features::person_catalog::tool.rs` NÃO é descendente de
/// `modules::memory`, mas precisa desta checagem antes de `add_fact`/`add_relationship` (as duas
/// escritas do grafo social que respeitam `memory.enabled` — ver `social::SocialMemoryService`).
pub use recording::memory_recording_allowed;

/// Continua sem canal de IPC (ver cabeçalho do módulo), mas passa a registrar a aba `mika.embeddings`
/// no `FeatureRegistry` como efeito colateral — mesmo padrão de `gamer::modules()`, sem
/// `ConfigStore`/`AppHandle` nenhum.
pub fn modules() -> Vec<Box<dyn NativeModule>> {
    settings::register();
    vec![]
}

// --- schema ---------------------------------------------------------------------------------
// Espelha `CREATE_TABLE`/`CREATE_INDEXES` de `embedded-store-sql.ts`, campo a campo.

const CREATE_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS memory_entries (
        key TEXT NOT NULL,
        field TEXT NOT NULL DEFAULT '',
        value TEXT NOT NULL DEFAULT '',
        score REAL,
        expires_at INTEGER,
        PRIMARY KEY (key, field)
    )
";

const CREATE_INDEXES: [&str; 3] = [
    "CREATE INDEX IF NOT EXISTS idx_memory_key ON memory_entries(key)",
    "CREATE INDEX IF NOT EXISTS idx_memory_expires ON memory_entries(expires_at)",
    "CREATE INDEX IF NOT EXISTS idx_memory_score ON memory_entries(key, score)",
];

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Porte de `globToLike`: escapa `\`, `%` e `_` (curinga literal do SQL) e só ENTÃO troca `*` por
/// `%` — a mesma ordem de duas passadas do TypeScript, feita num laço só porque `*` nunca é um dos
/// caracteres escapados na primeira passada (não há interação entre as duas regras).
fn glob_to_like(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    for ch in pattern.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            '*' => out.push('%'),
            other => out.push(other),
        }
    }
    out
}

/// Limite de um score de sorted-set. Cobre exatamente o que os chamadores reais usam no
/// TypeScript: um número, ou os marcadores `-inf`/`+inf` do Redis (ver comentário do módulo).
#[derive(Debug, Clone, Copy)]
pub enum ScoreBound {
    Score(f64),
    NegInf,
    PosInf,
}

impl From<f64> for ScoreBound {
    fn from(value: f64) -> Self {
        ScoreBound::Score(value)
    }
}

/// Porte de `toScoreBound`: `None` significa "sem limite" (marcador de infinito, ou score não
/// finito), e vira ausência da cláusula `AND score >= / <= ?` na consulta.
fn resolve_bound(bound: ScoreBound) -> Option<f64> {
    match bound {
        ScoreBound::Score(n) if n.is_finite() => Some(n),
        _ => None,
    }
}

/// `offset`/`count` de `zRangeByScore` e de `scan`.
#[derive(Debug, Clone, Copy)]
pub struct RangeLimit {
    pub offset: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ZRangeOpts {
    pub rev: bool,
    pub limit: Option<RangeLimit>,
}

// --- MemoryStore -----------------------------------------------------------------------------

/// Port de `EmbeddedMemoryStore`. Uma instância por arquivo `memory.db` (ou `:memory:` em teste);
/// `Mutex<Connection>` porque `rusqlite::Connection` não é `Sync` — o mesmo padrão que
/// `telemetry.rs` já usa para `token_usage.db`.
pub struct MemoryStore {
    conn: Mutex<Connection>,
}

impl MemoryStore {
    /// Abre (ou cria) o banco em `path`, criando o diretório pai se faltar, e garante o schema —
    /// idempotente e reversível: um arquivo já populado pelo TypeScript é preservado, não recriado.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("não criou {}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("não abriu {}: {e}", path.display()))?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self, String> {
        apply_pragmas(&conn)?;
        let migration_1 = std::iter::once(CREATE_TABLE).chain(CREATE_INDEXES).collect::<Vec<_>>().join(";\n");
        crate::modules::sqlite_migrate::migrate(&conn, &[&migration_1]).map_err(|e| format!("memory.db: {e}"))?;
        let store = Self { conn: Mutex::new(conn) };
        store.sweep_expired()?;
        Ok(store)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "memory store envenenado".to_string())
    }

    /// Porte de `sweepExpired`: remove só o que já venceu, nunca mexe em entrada sem TTL ou ainda
    /// viva. Roda uma vez na abertura (igual ao TypeScript) e pode ser chamado de novo à vontade.
    pub fn sweep_expired(&self) -> Result<usize, String> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM memory_entries WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            params![now_millis()],
        )
        .map_err(|e| format!("sweep_expired: {e}"))
    }

    /// Porte de `get`. `None` para chave nunca escrita — nunca confunde ausência com string vazia.
    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT value FROM memory_entries WHERE key = ?1 AND field = '' \
             AND (expires_at IS NULL OR expires_at > ?2)",
            params![key, now_millis()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("get: {e}"))
    }

    /// Porte de `set`. `ex_seconds` negativo (ou zero) expira a entrada imediatamente, sem exigir
    /// `sweep_expired` manual — o mesmo comportamento que o teste TS `set com EX expira` cobre.
    pub fn set(&self, key: &str, value: &str, ex_seconds: Option<i64>) -> Result<(), String> {
        let expires_at = ex_seconds.map(|secs| now_millis() + secs * 1000);
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO memory_entries (key, field, value, score, expires_at) \
             VALUES (?1, '', ?2, NULL, ?3)",
            params![key, value, expires_at],
        )
        .map_err(|e| format!("set: {e}"))?;
        Ok(())
    }

    /// Porte de `hSet`. O campo novo herda o TTL dos irmãos já gravados na mesma chave — é o que
    /// impede um hash já expirado de "ressuscitar" porque um campo isolado nasceu sem prazo.
    pub fn h_set(&self, key: &str, field: &str, value: &str) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO memory_entries (key, field, value, score, expires_at) \
             VALUES (?1, ?2, ?3, NULL, (SELECT MAX(expires_at) FROM memory_entries WHERE key = ?1))",
            params![key, field, value],
        )
        .map_err(|e| format!("h_set: {e}"))?;
        Ok(())
    }

    /// Porte de `hGetAll`. Chave sem hash devolve mapa vazio, nunca erro.
    pub fn h_get_all(&self, key: &str) -> Result<HashMap<String, String>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT field, value FROM memory_entries WHERE key = ?1 AND field != '' \
                 AND (expires_at IS NULL OR expires_at > ?2)",
            )
            .map_err(|e| format!("h_get_all: {e}"))?;
        let rows = stmt
            .query_map(params![key, now_millis()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("h_get_all: {e}"))?;

        let mut out = HashMap::new();
        for row in rows {
            let (field, value) = row.map_err(|e| format!("h_get_all: {e}"))?;
            out.insert(field, value);
        }
        Ok(out)
    }

    /// Porte de `hDel`. Devolve quantas linhas saíram (0 ou 1, como a PK `(key, field)` garante).
    pub fn h_del(&self, key: &str, field: &str) -> Result<usize, String> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM memory_entries WHERE key = ?1 AND field = ?2",
            params![key, field],
        )
        .map_err(|e| format!("h_del: {e}"))
    }

    /// Porte de `del`: 1 se apagou algo, 0 se a chave nunca existiu — nunca finge sucesso.
    pub fn del(&self, key: &str) -> Result<usize, String> {
        let conn = self.lock()?;
        let affected = conn
            .execute("DELETE FROM memory_entries WHERE key = ?1", params![key])
            .map_err(|e| format!("del: {e}"))?;
        Ok(if affected > 0 { 1 } else { 0 })
    }

    /// Porte de `expire`: só aplica TTL a uma chave que já existe; 0 se não existir.
    pub fn expire(&self, key: &str, seconds: i64) -> Result<usize, String> {
        let expires_at = now_millis() + seconds * 1000;
        let conn = self.lock()?;
        let affected = conn
            .execute(
                "UPDATE memory_entries SET expires_at = ?1 WHERE key = ?2",
                params![expires_at, key],
            )
            .map_err(|e| format!("expire: {e}"))?;
        Ok(if affected > 0 { 1 } else { 0 })
    }

    /// Porte de `zAdd`. `INSERT OR REPLACE` sobre a PK `(key, field=member)` faz o mesmo membro
    /// ATUALIZAR o score em vez de duplicar — sem isso um "ranking" cresceria sem limite.
    pub fn z_add(&self, key: &str, score: f64, member: &str) -> Result<usize, String> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO memory_entries (key, field, value, score, expires_at) \
             VALUES (?1, ?2, '', ?3, NULL)",
            params![key, member, score],
        )
        .map_err(|e| format!("z_add: {e}"))
    }

    /// Porte de `zRem`.
    pub fn z_rem(&self, key: &str, member: &str) -> Result<usize, String> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM memory_entries WHERE key = ?1 AND field = ?2",
            params![key, member],
        )
        .map_err(|e| format!("z_rem: {e}"))
    }

    /// Porte de `zRangeByScore`/`scoreRangeQuery`: intervalo de score com `LIMIT`/`OFFSET` e
    /// direção — a variante PAGINADA da leitura de sorted-set (ver nota de paginação no topo do
    /// arquivo).
    ///
    /// `rowid {direction}` desempata: o score da memória episódica é um timestamp em MILISSEGUNDOS
    /// (`episodic::types::now_millis`), e duas escritas no mesmo milissegundo empatavam — o SQLite
    /// ficava livre para devolver qualquer uma primeiro. MESMO bug, mesma correção, que
    /// `session_db.rs::history` já documenta: `rowid` é a ordem de inserção (linha nova = `INSERT OR
    /// REPLACE` sobre `(key, field)`, nunca reaproveita rowid de outra chave), que é exatamente o
    /// que "mais recente" significa num empate.
    pub fn z_range_by_score(
        &self,
        key: &str,
        min: ScoreBound,
        max: ScoreBound,
        opts: ZRangeOpts,
    ) -> Result<Vec<String>, String> {
        let lo = resolve_bound(min);
        let hi = resolve_bound(max);
        let direction = if opts.rev { "DESC" } else { "ASC" };
        let limit = opts.limit.map(|l| l.count).unwrap_or(-1);
        let offset = opts.limit.map(|l| l.offset).unwrap_or(0);

        let mut bounds = String::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(key.to_string())];
        if let Some(lo) = lo {
            bounds.push_str(" AND score >= ?");
            args.push(Box::new(lo));
        }
        if let Some(hi) = hi {
            bounds.push_str(" AND score <= ?");
            args.push(Box::new(hi));
        }
        args.push(Box::new(now_millis()));
        args.push(Box::new(limit));
        args.push(Box::new(offset));

        let sql = format!(
            "SELECT field FROM memory_entries WHERE key = ? AND score IS NOT NULL{bounds} \
             AND (expires_at IS NULL OR expires_at > ?) ORDER BY score {direction}, rowid {direction} \
             LIMIT ? OFFSET ?"
        );

        let conn = self.lock()?;
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("z_range_by_score: {e}"))?;
        let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(refs.as_slice(), |row| row.get::<_, String>(0))
            .map_err(|e| format!("z_range_by_score: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("z_range_by_score: {e}"))
    }

    /// Porte de `keys`. Sem `LIMIT` — ver nota de paginação no topo do arquivo.
    pub fn keys(&self, pattern: &str) -> Result<Vec<String>, String> {
        let like = glob_to_like(pattern);
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT key FROM memory_entries WHERE key LIKE ?1 ESCAPE '\\' \
                 AND (expires_at IS NULL OR expires_at > ?2)",
            )
            .map_err(|e| format!("keys: {e}"))?;
        let rows = stmt
            .query_map(params![like, now_millis()], |row| row.get::<_, String>(0))
            .map_err(|e| format!("keys: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("keys: {e}"))
    }

}

/// WAL + `synchronous=NORMAL` + `busy_timeout` + `foreign_keys=ON` — os mesmos pragmas que
/// `applyPragmas` aplica em `packages/sdk/src/services/database.ts`. Manter os dois lados na mesma
/// configuração importa para o arquivo continuar compatível se o usuário voltar numa versão
/// anterior do app (o `-wal`/`-shm` que o SQLite cria do lado ficam no formato que os dois
/// entendem).
fn apply_pragmas(conn: &Connection) -> Result<(), String> {
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("pragma journal_mode: {e}"))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| format!("pragma synchronous: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(|e| format!("busy_timeout: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("pragma foreign_keys: {e}"))?;
    Ok(())
}

// --- Singleton de processo ----------------------------------------------------------------------
// Mesmo padrão que `telemetry.rs` usa para `token_usage.db`: um `OnceLock` PRÓPRIO deste módulo,
// para um banco que não é `config.json` — a regra de instância única (`state::config_store`) vale
// só para o `ConfigStore`, não para bancos de domínio.
//
// Chamadores de produção hoje: `features::privacy::clear::register` (abre o store no boot, dentro
// de `register_config_bound_tools`, que TEM `AppHandle`) e, a partir desta rodada,
// `episodic_reader()` logo abaixo — para os dois chamadores de LEITURA que não têm `AppHandle` no
// próprio construtor (`memory_console`, registrado por `brain::tools::register_config_free_tools`
// SEM config; `list_memories`, registrado por `features::privacy::list_memories::register()`
// também sem argumento). Nenhum dos dois pode abrir o próprio `ConfigStore`
// (`state::single_config_store_per_process` proíbe), então `episodic_reader()` lê deste `OnceLock`
// em vez de pedir um `AppHandle` que não existe no ponto de chamada.

static MEMORY_STORE: OnceLock<MemoryStore> = OnceLock::new();

/// `<app_data_dir>/plugins/mika.brain/memory.db` — o mesmo caminho que
/// `bootstrapBrainServices` abre hoje no sidecar (ver comentário de compatibilidade no topo).
fn memory_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = crate::state::app_data_dir(app)
        .map_err(|e| format!("não resolveu app_data_dir: {e}"))?;
    Ok(data_dir.join("plugins").join("mika.brain").join("memory.db"))
}


/// `brain.rerankingEnabled`/`brain.emotionalTagging`/`brain.recencyBias`/`brain.ragTopK`/
/// `brain.minSimilarity`/`brain.lexicalRerankWeight` e `memory.shortTermTtlHours`/
/// `memory.longTermTtlDays`, lidos a cada uso por [`live_config`]. Antes desta rodada
/// `episodic_reader()` chamava `EpisodicMemoryService::new` sem NENHUM destes seis campos vindo do
/// `ConfigStore` — Settings > Brain mostrava os controles, mas rerank/top-k/limiar de similaridade
/// continuavam sempre nos defaults do TS embutidos no construtor, e `apply_retention_config(None,
/// None, ...)` sempre caía no default de 168h/365d mesmo com TTL configurado.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EpisodicScoringConfig {
    pub reranking_enabled: bool,
    pub emotional_tagging: bool,
    pub embedding_enabled: bool,
    pub recency_bias: f64,
    pub rag_top_k: usize,
    pub min_similarity: f64,
    pub lexical_rerank_weight: f64,
    pub short_term_ttl_hours: f64,
    pub long_term_ttl_days: f64,
}

/// Leitura pull-based sem `AppHandle`, `None` só antes do primeiro `memory_store(app)` do processo.
pub(crate) fn episodic_scoring_config_reader() -> Option<EpisodicScoringConfig> {
    live_config::scoring()
}

/// Config de embedding, lida a cada uso por [`live_config`] (trocar o provedor na UI vale na hora). Consumido
/// por `episodic::embedding::generate_embedding`, que não tem `ConfigStore` no próprio ponto de
/// chamada (é síncrono, chamado de dentro de `write.rs`/`service.rs` sem `AppHandle`). Os campos
/// espelham `llm.embeddingProvider`/`llm.embeddingModel`/`apiKeys.ollamaBaseUrl`/
/// `apiKeys.ollamaApiKey`/`apiKeys.googleAI`/`apiKeys.openrouter`/
/// `apiKeys.openaiCompatibleBaseUrl`/`apiKeys.openaiCompatibleApiKey` (mesmos paths que
/// `modules::llm` já usa para o catálogo de chat — cópia literal, não reexport: `memory` é núcleo e
/// não depende de `llm`). `base_url`/`api_key` seguem servindo só o Ollama (compatibilidade do campo
/// pré-existente); os outros três provedores têm campo próprio porque não compartilham forma de
/// credencial (Google não tem `baseUrl` configurável, OpenRouter é URL fixa, só OpenAI-compatible
/// tem os dois).
#[derive(Debug, Clone)]
pub(crate) struct EmbeddingSettings {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub google_api_key: String,
    pub openrouter_api_key: String,
    pub openai_compat_base_url: String,
    pub openai_compat_api_key: String,
    /// `apiKeys.unslothBaseUrl`/`apiKeys.unslothApiKey` — o Unsloth tem credencial própria.
    pub unsloth_base_url: String,
    pub unsloth_api_key: String,
    /// `llm.embeddingTaskType`/`llm.embeddingOutputDimensionality`, sent to Google only (0 = default).
    pub task_type: String,
    pub dimensions: i64,
    pub offline_mode: bool,
}

/// Leitura pull-based sem `AppHandle`, `None` só antes do primeiro `memory_store(app)` do processo.
pub(crate) fn embedding_settings_reader() -> Option<EmbeddingSettings> {
    live_config::embedding()
}

/// O `MemoryStore` do processo, aberto sob demanda na primeira chamada. Também prende o `ConfigStore`
/// do processo para [`live_config`] (se ele falhar, os leitores caem no default, nunca panicam).
pub fn memory_store(app: &AppHandle) -> Result<&'static MemoryStore, String> {
    if let Some(store) = MEMORY_STORE.get() {
        return Ok(store);
    }

    let path = memory_db_path(app)?;
    let store = MemoryStore::open(&path)?;
    let _ = MEMORY_STORE.set(store);

    if let Ok(config) = crate::state::config_store(app) {
        live_config::attach(config);
    }

    MEMORY_STORE
        .get()
        .ok_or_else(|| "memory store não inicializou".to_string())
}

/// `memory.enabled` lido na hora; `true` (mesmo default "aberto" de [`memory_recording_allowed`])
/// antes de `memory_store(app)` prender o `ConfigStore`.
pub fn recording_allowed_cached() -> bool {
    live_config::recording_allowed()
}

/// Esvazia os caches de vetor em RAM (consulta e fatos) — "apagar tudo" não pode deixá-los de pé.
pub fn clear_embedding_caches() {
    episodic::clear_embedding_caches();
}

/// O serviço episódico com a config viva (TTL, limite, scoring, `embeddingEnabled`). É o que TODO
/// caminho de escrita usa — `episodic_reader()` é só este sobre o `MemoryStore` de processo.
pub fn episodic_configured<'a>(store: &'a MemoryStore) -> episodic::EpisodicMemoryService<'a> {
    episodic::EpisodicMemoryService::with_config(store, episodic_scoring_config_reader().as_ref(), live_config::list_limit())
}


/// `EpisodicMemoryService` pronto para LEITURA — a porta estreita que `memory_console`
/// (`snapshot`/`search_episodic`/`clear_episodic`) e `list_memories` usam para alcançar
/// `episodic::queries::get_all`/`search`/`clear` sem duplicar a lógica de ordenação/TTL/poda
/// daquele submódulo PRIVADO (ver o cabeçalho de `episodic/mod.rs`).
///
/// `None` só antes de `memory_store(app)` ter rodado ao menos uma vez — na prática, nunca em
/// runtime normal de tool (`register_config_bound_tools` já chama `memory_store` via
/// `features::privacy::clear::register`, antes da janela aceitar qualquer IPC de tool-execute) e
/// sempre em teste de unidade (nenhum teste tem `AppHandle` para popular o `OnceLock` — por isso os
/// testes de `memory_console`/`list_memories` injetam um `EpisodicMemoryService` próprio em vez de
/// depender deste singleton; ver os cabeçalhos deles).
pub fn episodic_reader() -> Option<episodic::EpisodicMemoryService<'static>> {
    let store = MEMORY_STORE.get()?;
    Some(episodic_configured(store))
}

/// `social::SocialMemoryService` sobre o MESMO `MemoryStore` de processo que `episodic_reader()` já
/// usa — o grafo social não tem arquivo próprio (ver o cabeçalho de `social/mod.rs`). Usado por
/// `brain::tools::memory_console::add_fact`, que como `episodic_reader()` não tem `AppHandle` no
/// próprio ponto de chamada. `features::person_catalog::tool.rs` NÃO usa este getter: ele já resolve
/// `memory_store(app)` direto no registro da tool (tem `AppHandle` ali) e guarda a referência.
pub fn social_reader() -> Option<social::SocialMemoryService<'static>> {
    MEMORY_STORE.get().map(social::SocialMemoryService::new)
}

/// `MemoryStore` cru — mesma leitura sem `AppHandle` de [`episodic_reader`]/[`social_reader`], sem
/// embrulhar num serviço: `reflection::goals` (metas ativas) opera direto sobre `MemoryStore`
/// (`h_get_all`/`h_set`), sem uma camada de serviço própria. Adicionado para
/// `brain::persona::compose_system_prompt` injetar metas ativas no prompt — esse ponto de chamada
/// não tem `AppHandle` (é síncrono, chamado de dentro de `brain::turn::compose_system_prompt`).
pub fn memory_store_reader() -> Option<&'static MemoryStore> {
    MEMORY_STORE.get()
}

#[cfg(test)]
impl MemoryStore {
    /// Banco efêmero em memória — equivalente ao `createClient({ url: ':memory:' })` que os testes
    /// do TypeScript usam para não sujar disco.
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("não abriu :memory:: {e}"))?;
        Self::from_connection(conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// `modules()` continua sem canal de IPC (ver cabeçalho), mas passa a registrar a aba
    /// `mika.embeddings` no `FeatureRegistry` compartilhado como efeito colateral.
    #[test]
    fn modules_registers_the_embeddings_settings_tab_as_a_side_effect() {
        modules();
        let all = crate::feature_registry::shared().get_all_settings();
        assert!(all.iter().any(|d| d["tabId"] == json!("embeddings") && d["pluginId"] == json!("mika.embeddings")));
    }

    /// Diretório temporário mínimo, mesmo padrão usado em `config/store.rs` e `telemetry.rs`.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-memory-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // O store precisa ter sido fechado (guard fora de escopo) ANTES de apagar o diretório —
            // é o ponto que já mordeu este repo em outro módulo (save debounced depois do rmSync).
            // Aqui a escrita é síncrona e o `Connection` fecha no `Drop` do `MemoryStore`, então não
            // há gravação pendente quando este destrutor roda.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    // --- caso vazio -----------------------------------------------------------------------------

    #[test]
    fn get_devolve_none_para_chave_nunca_escrita() {
        let store = store();
        assert_eq!(store.get("nope").unwrap(), None);
    }

    #[test]
    fn h_get_all_de_hash_inexistente_devolve_mapa_vazio() {
        let store = store();
        assert!(store.h_get_all("fantasma").unwrap().is_empty());
    }

    #[test]
    fn keys_em_banco_vazio_nao_erra() {
        let store = store();
        assert!(store.keys("*").unwrap().is_empty());
    }

    // --- unicode ----------------------------------------------------------------------------------

    #[test]
    fn set_get_roundtrip_preserva_unicode() {
        let store = store();
        store.set("greeting", "olá 🎮 — テスト", None).unwrap();
        assert_eq!(store.get("greeting").unwrap(), Some("olá 🎮 — テスト".to_string()));
    }

    #[test]
    fn h_set_h_get_all_preservam_unicode_por_campo() {
        let store = store();
        store.h_set("person:1", "name", "Mika ⭐").unwrap();
        store.h_set("person:1", "mood", "feliz 😊").unwrap();
        let hash = store.h_get_all("person:1").unwrap();
        assert_eq!(hash.get("name"), Some(&"Mika ⭐".to_string()));
        assert_eq!(hash.get("mood"), Some(&"feliz 😊".to_string()));
    }

    // --- TTL / expiração ----------------------------------------------------------------------

    #[test]
    fn set_com_ex_negativo_expira_imediatamente() {
        let store = store();
        store.set("temp", "x", Some(-1)).unwrap();
        assert_eq!(store.get("temp").unwrap(), None);
    }

    #[test]
    fn campo_escrito_depois_herda_prazo_dos_irmaos_ja_expirados() {
        let store = store();
        store.h_set("h", "content", "x").unwrap();
        store.expire("h", -1).unwrap();
        store.h_set("h", "metadata", "{}").unwrap();
        assert!(store.h_get_all("h").unwrap().is_empty());
    }

    #[test]
    fn h_del_remove_so_o_campo_pedido_mantendo_os_demais() {
        let store = store();
        store.h_set("h", "a", "1").unwrap();
        store.h_set("h", "b", "2").unwrap();
        assert_eq!(store.h_del("h", "a").unwrap(), 1);
        let hash = store.h_get_all("h").unwrap();
        assert_eq!(hash.len(), 1);
        assert_eq!(hash.get("b"), Some(&"2".to_string()));
    }

    #[test]
    fn z_rem_tira_o_membro_do_ranking() {
        let store = store();
        store.z_add("rank", 1.0, "a").unwrap();
        assert_eq!(store.z_rem("rank", "a").unwrap(), 1);
        assert!(store
            .z_range_by_score("rank", ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts::default())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn del_devolve_1_quando_apagou_e_0_quando_a_chave_nunca_existiu() {
        let store = store();
        store.set("k", "v", None).unwrap();
        assert_eq!(store.del("k").unwrap(), 1);
        assert_eq!(store.del("k").unwrap(), 0);
    }

    #[test]
    fn expire_em_chave_inexistente_nao_finge_sucesso() {
        let store = store();
        assert_eq!(store.expire("ghost", 60).unwrap(), 0);
    }

    #[test]
    fn sweep_expired_remove_so_entradas_vencidas() {
        let store = store();
        store.set("dead", "x", Some(-1)).unwrap();
        store.set("alive", "x", None).unwrap();
        assert_eq!(store.sweep_expired().unwrap(), 1);
        assert_eq!(store.get("alive").unwrap(), Some("x".to_string()));
    }

    // --- retry idempotente -------------------------------------------------------------------

    #[test]
    fn z_add_no_mesmo_membro_atualiza_score_em_vez_de_duplicar() {
        let store = store();
        store.z_add("rank", 1.0, "a").unwrap();
        store.z_add("rank", 9.0, "a").unwrap();
        assert_eq!(
            store
                .z_range_by_score("rank", ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts::default())
                .unwrap(),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn reabrir_o_mesmo_arquivo_e_idempotente_e_preserva_dados() {
        let dir = TempDir::new();
        let path = dir.0.join("memory.db");

        {
            let store = MemoryStore::open(&path).unwrap();
            store.set("k", "sobrevive", None).unwrap();
        }
        // Reabertura simula o app reiniciando sobre um banco que já existe — CREATE TABLE/INDEX
        // IF NOT EXISTS não pode apagar nem falhar sobre schema já presente.
        {
            let store = MemoryStore::open(&path).unwrap();
            assert_eq!(store.get("k").unwrap(), Some("sobrevive".to_string()));
        }
    }

    // --- limite / paginação -------------------------------------------------------------------

    #[test]
    fn z_range_by_score_respeita_min_max_e_ordem_padrao() {
        let store = store();
        store.z_add("rank", 3.0, "c").unwrap();
        store.z_add("rank", 1.0, "a").unwrap();
        store.z_add("rank", 2.0, "b").unwrap();

        assert_eq!(
            store
                .z_range_by_score("rank", ScoreBound::Score(1.0), ScoreBound::Score(2.0), ZRangeOpts::default())
                .unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            store
                .z_range_by_score(
                    "rank",
                    ScoreBound::NegInf,
                    ScoreBound::PosInf,
                    ZRangeOpts { rev: true, limit: None },
                )
                .unwrap(),
            vec!["c".to_string(), "b".to_string(), "a".to_string()]
        );
    }

    #[test]
    fn z_range_by_score_limit_recorta_e_offset_pula_mantendo_ordem() {
        let store = store();
        store.z_add("rank", 1.0, "a").unwrap();
        store.z_add("rank", 2.0, "b").unwrap();
        store.z_add("rank", 3.0, "c").unwrap();

        let top_2 = store
            .z_range_by_score(
                "rank",
                ScoreBound::NegInf,
                ScoreBound::PosInf,
                ZRangeOpts { rev: true, limit: Some(RangeLimit { offset: 0, count: 2 }) },
            )
            .unwrap();
        assert_eq!(top_2, vec!["c".to_string(), "b".to_string()]);

        let middle = store
            .z_range_by_score(
                "rank",
                ScoreBound::NegInf,
                ScoreBound::PosInf,
                ZRangeOpts { rev: false, limit: Some(RangeLimit { offset: 1, count: 1 }) },
            )
            .unwrap();
        assert_eq!(middle, vec!["b".to_string()]);
    }

    #[test]
    fn z_range_by_score_breaks_ties_on_rowid_so_same_millisecond_writes_stay_deterministic() {
        let store = store();
        // Mesmo score (mesmo milissegundo de timestamp, na prática) — sem o desempate por rowid,
        // o SQLite é livre para devolver "first"/"second" em qualquer ordem entre si. Mesma classe
        // de bug que `session_db.rs::history` documenta.
        store.z_add("rank", 100.0, "first").unwrap();
        store.z_add("rank", 100.0, "second").unwrap();

        let newest_first = store
            .z_range_by_score("rank", ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts { rev: true, limit: None })
            .unwrap();
        assert_eq!(
            newest_first,
            vec!["second".to_string(), "first".to_string()],
            "empate de score desempata pela ordem de inserção (rowid), não por acaso"
        );

        let oldest_first = store
            .z_range_by_score("rank", ScoreBound::NegInf, ScoreBound::PosInf, ZRangeOpts::default())
            .unwrap();
        assert_eq!(oldest_first, vec!["first".to_string(), "second".to_string()], "ASC também desempata de forma consistente, não só DESC");
    }

    // --- glob → LIKE ----------------------------------------------------------------------------

    #[test]
    fn keys_traduz_glob_sem_deixar_caracteres_especiais_escaparem_o_padrao() {
        let store = store();
        store.set("user:1", "v", None).unwrap();
        store.set("user:2", "v", None).unwrap();
        store.set("other", "v", None).unwrap();

        let mut matched = store.keys("user:*").unwrap();
        matched.sort();
        assert_eq!(matched, vec!["user:1".to_string(), "user:2".to_string()]);
    }

    #[test]
    fn underscore_no_padrao_e_literal_nao_curinga_de_um_caractere() {
        let store = store();
        store.set("a_b", "certo", None).unwrap();
        store.set("axb", "errado", None).unwrap();
        assert_eq!(store.keys("a_b").unwrap(), vec!["a_b".to_string()]);
    }

    #[test]
    fn glob_to_like_escapa_literais_antes_de_traduzir_o_curinga() {
        assert_eq!(glob_to_like("100%_off*"), "100\\%\\_off%");
        assert_eq!(glob_to_like("a\\b"), "a\\\\b");
    }
}
