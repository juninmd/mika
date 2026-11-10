//! O que roda DEPOIS do turno: reflexão. Porte de
//! `apps/desktop/src/main/services/brain/reflection.ts` (`ReflectionService`). O resto do que
//! rodava no mesmo gatilho no TypeScript (`PostTurnPipeline`, `brain/chat/pipeline.ts`) tem dono
//! PRÓPRIO: [`super::post_turn`].
//!
//! Diretório, não arquivo único — a regra de 300 linhas do AGENTS.md não cabia na versão anterior
//! (arquivo único) depois que a chamada ao modelo e a análise da resposta entraram nesta rodada:
//! - [`cadence`] — o contador "só a cada 5º turno" (`ReflectionCadence`).
//! - [`prompts`] — monta o PEDIDO: o recorte da conversa e o carregamento dos dois prompts
//!   (`reflection-system`, `reflection-subconscious`, via
//!   [`crate::modules::brain::prompt_loader`], compartilhado com `post_turn::facts`).
//! - [`parsing`] — o que fazer com a RESPOSTA do modelo: interpretar o JSON (via
//!   [`crate::modules::brain::response_json`], compartilhado), aceitar ou descartar.
//! - [`goals`] — porte mínimo de `GoalsService.addGoal`, só o lado de ESCRITA (ver seção abaixo).
//! - Este arquivo — [`run_reflection`], a orquestração que amarra os três primeiros, chama o modelo
//!   via [`super::turn::run_turn_llm_raw`] (fachada tool-free, multi-provedor, já usada por
//!   `gamer::session_memory`/`game_knowledge_tool` para o mesmo tipo de geração fora do turno
//!   principal de chat), persiste o insight aceito em memória episódica e, quando a nota passa o
//!   corte, grava metas via [`goals::add_goal`].
//!
//! **Chamador real desde esta rodada**: [`super::post_turn::listener::spawn_post_turn_listener`].
//!
//! ## O que ainda ficou de fora
//! - **Lado de LEITURA do laço de metas** (`Events.REFLECTED` → `addReflectionGoals` grava; quem lê
//!   de volta é `getBrainContext()` → `context-registry` → o prompt). [`run_reflection`] agora
//!   grava metas de verdade via [`goals::add_goal`] (o lado de ESCRITA, pedido explicitamente nesta
//!   rodada), mas nada em Rust ainda injeta essas metas de volta no prompt — o ponto de injeção é
//!   `turn::build_raw_system_prompt`, privada a `turn.rs`, fora do escopo desta rodada. Ver o
//!   cabeçalho de `goals.rs`.
//! - **Divergência de composição do prompt**: [`super::turn::run_turn_llm_raw`] sempre injeta o
//!   `system_prompt` da PERSONA (`compose_system_prompt`) antes de anexar o prompt de reflexão sob
//!   `## CURRENT TASK` (`turn::build_raw_system_prompt`, privada a `turn.rs` — não editável nesta
//!   rodada). O TypeScript usa `reflection-system.md` PURO, sem persona. O resultado ainda é uma
//!   reflexão de verdade, só que com uma camada extra de contexto que o TS não tinha — documentado,
//!   não escondido.

mod cadence;
pub mod goals;
mod parsing;
mod prompts;

pub use cadence::ReflectionCadence;
// `ReflectionInsight`/`ReflectionRequest`/`build_conversation_text` continuam `pub` nos próprios
// arquivos (`parsing.rs`/`prompts.rs`) para o teste de cada um, mas não são reexportados aqui: nada
// FORA de `parsing`/`prompts` os nomeia sem qualificar (`run_reflection` só usa as VARIANTES de
// `ReflectionStep`/o resultado de `accept_insight`) — reexportar um nome que ninguém importa é
// warning de `unused_imports` neste crate binário (pub sem consumidor externo não conta como uso).
use parsing::accept_insight;
use prompts::{prepare_reflection_step, ReflectionStep};

use tauri::AppHandle;

use crate::config::store::ConfigStore;

use super::turn;

/// = `BRAIN_CONSTANTS.INSIGHT_RATING_THRESHOLD` (`constants.ts`): abaixo disso a reflexão não vira
/// meta — mesmo corte de `addReflectionGoals`.
const INSIGHT_RATING_THRESHOLD: f64 = 6.0;
/// = `BRAIN_CONSTANTS.MAX_INSIGHT_OBSERVATIONS`: só as N primeiras observações viram meta por ciclo.
const MAX_INSIGHT_OBSERVATIONS: usize = 2;

/// O que [`run_reflection`] fez desta vez — cada variante é um dos pontos de saída de `maybeRun`
/// no TypeScript, agora nomeados em vez de um `null` silencioso only.
#[derive(Debug)]
pub enum ReflectionOutcome {
    /// Não era a vez do ciclo (cadência).
    NotDue,
    /// `memory.enabled=false`: nem o modelo é chamado (o turno inteiro iria a ele para nada) e
    /// nenhuma reflexão ou meta é gravada.
    MemoryDisabled,
    /// `reflection-system`/`reflection-subconscious` ausente ou em branco.
    PromptUnavailable(String),
    /// O modelo falhou (rede, provedor, timeout, abort).
    ModelFailed(String),
    /// A resposta do modelo não era (ou não continha) o JSON esperado.
    ParseFailed(String),
    /// O modelo respondeu, mas `reflection` ficou vazio depois do `trim` — nada para guardar.
    Blank,
    /// Aceita e persistida em memória episódica. `memory_id` é `None` quando a escrita falhou
    /// (memória desabilitada, dedup, ou erro de disco) — a reflexão em si não se perde do log.
    Stored { memory_id: Option<String> },
}

/// Orquestra um ciclo de reflexão sobre um turno concluído: monta o pedido
/// ([`prepare_reflection_step`]), chama o modelo configurado ([`turn::run_turn_llm_raw`]),
/// interpreta o JSON ([`parsing::parse_reflection_json`], privada — só [`accept_insight`] é
/// pública), aceita ([`accept_insight`]) e persiste em memória episódica — mesmos `type`/`source`
/// que o TypeScript grava (`{ type: 'reflection', source: 'subconscious' }`).
///
/// Chamado por [`super::post_turn::listener::spawn_post_turn_listener`] a cada `ThinkingStop`
/// reconstruído do turno (ver o cabeçalho daquele módulo para o porquê da reconstrução).
pub async fn run_reflection(
    app: &AppHandle,
    store: &ConfigStore,
    cadence: &mut ReflectionCadence,
    user: &str,
    assistant: &str,
) -> ReflectionOutcome {
    if !crate::modules::memory::memory_recording_allowed(store) {
        return ReflectionOutcome::MemoryDisabled;
    }
    let request = match prepare_reflection_step(app, cadence, user, assistant) {
        ReflectionStep::NotDue => return ReflectionOutcome::NotDue,
        ReflectionStep::PromptUnavailable(error) => return ReflectionOutcome::PromptUnavailable(error),
        ReflectionStep::Ready(request) => request,
    };

    let (_handle, signal) = turn::TurnAbortHandle::new();
    let text = match turn::run_turn_llm_raw(store, Some(&request.system_prompt), &request.user_prompt, signal, &turn::NullObserver).await {
        Ok(result) => result.text,
        Err(error) => return ReflectionOutcome::ModelFailed(error.to_string()),
    };

    let insight = match parsing::parse_reflection_json(&text) {
        Ok(insight) => insight,
        Err(error) => return ReflectionOutcome::ParseFailed(error),
    };

    let Some(accepted) = accept_insight(insight) else {
        return ReflectionOutcome::Blank;
    };

    // Porte de `addReflectionGoals` (`brain-event-handlers.ts`): grava (lado de ESCRITA só — ver
    // cabeçalho de `goals.rs` para o porquê do lado de leitura ficar de fora desta rodada).
    if accepted.rating >= INSIGHT_RATING_THRESHOLD {
        for observation in accepted.observations.iter().take(MAX_INSIGHT_OBSERVATIONS) {
            let description = format!("Improve based on self-reflection: {observation}");
            if let Err(error) = store_reflection_goal(app, &description, accepted.rating) {
                log::warn!("meta de reflexão não persistida: {error}");
            }
        }
    }

    // Embedding + SQLite são síncronos (o embedding espera o provedor por até 20 s): fora do worker
    // async, como `post_turn::pipeline::store_turn_memory`.
    let (app_for_store, reflection) = (app.clone(), accepted.reflection.clone());
    let stored = tauri::async_runtime::spawn_blocking(move || store_reflection(&app_for_store, &reflection)).await;
    let memory_id = stored.map_err(|error| error.to_string()).and_then(|result| result).unwrap_or_else(|error| {
        log::warn!("reflexão gerada mas não persistida em memória episódica: {error}");
        None
    });

    ReflectionOutcome::Stored { memory_id }
}

/// Persiste a reflexão aceita em memória episódica — mesma chamada que o TypeScript faz
/// (`memory.store(reflection, undefined, { type: 'reflection', source: 'subconscious' })`).
/// `EpisodicMemoryService` é `pub(crate)` (ver `memory/episodic/mod.rs`), então este módulo
/// constrói a própria instância sobre o `MemoryStore` de processo em vez de o `memory` expor uma
/// porta nova só para isto — nenhuma lógica de dedup/TTL é duplicada, é a MESMA fachada que
/// `memory_console`/`list_memories` já usam para leitura.
fn store_reflection(app: &AppHandle, reflection: &str) -> Result<Option<String>, String> {
    let mem_store = crate::modules::memory::memory_store(app)?;
    let config = crate::state::config_store(app)?;
    let recording_allowed = crate::modules::memory::memory_recording_allowed(&config);

    let service = crate::modules::memory::episodic_configured(mem_store);
    let metadata = serde_json::json!({ "type": "reflection", "source": "subconscious" });
    service.store(recording_allowed, reflection, None, Some(&metadata))
}

/// Porte de `brain.addGoal(id, goal, insight.rating)` dentro de `addReflectionGoals`: mesmo formato
/// de id (`auto-reflect-{epoch_ms}-{5 chars base36}`). `GoalsService` do TS não é gateada por
/// `memory.enabled` (é estado do processo, não memória de conversa) — [`goals::add_goal`] segue o
/// mesmo comportamento, sem checar `memory_recording_allowed`.
fn store_reflection_goal(app: &AppHandle, description: &str, rating: f64) -> Result<(), String> {
    let mem_store = crate::modules::memory::memory_store(app)?;
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let id = format!("auto-reflect-{now_ms}-{}", random_suffix());
    goals::add_goal(mem_store, &id, description, rating, now_ms)
}

/// Porte de `Math.random().toString(36).slice(2, 7)`: 5 caracteres alfanuméricos minúsculos, só
/// para desempatar o id — não precisa de aleatoriedade criptográfica.
fn random_suffix() -> String {
    use rand::RngExt;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..5).map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char).collect()
}
