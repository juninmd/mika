//! O que este porte do `PostTurnPipeline` (`brain/chat/pipeline.ts`) de fato faz por turno, e o
//! porquê explícito de cada passo do TypeScript que NÃO está aqui — ver "O que ficou de fora"
//! abaixo antes de assumir que algo foi esquecido.
//!
//! ## O que roda
//! [`run`] amarra, na mesma ordem de `PostTurnPipeline.run`:
//! 1. **Memória episódica** — grava o turno (usuário + Mika) sempre, como no TS
//!    (`storeTurnMemory` roda em TODO turno, não só nos "heavy"). Mesma fachada que
//!    `memory_console`/`list_memories` já leem — `EpisodicMemoryService::store`.
//! 2. **Fatos + ingestão social** ([`super::facts::run`], `mod facts;`) — porte de
//!    `Promise.all([ingestSocialMemory(user), runFactExtraction(user, assistant, llm, turnId)])`,
//!    também sempre, nunca só em turno pesado (ver o cabeçalho de `facts.rs`).
//! 3. **Cadência pesada** ([`super::cadence::HeavyCadence`]) — decide se ESTE turno tenta
//!    consolidação (`SHORT_MSG_THRESHOLD`/`HEAVY_INTERVAL_LONG`/`HEAVY_INTERVAL_SHORT`, mesmos
//!    valores de `pipeline.ts`).
//! 4. **Consolidação** ([`super::consolidation::maybe_consolidate`], só em turno pesado) — porte de
//!    `runConsolidation` → `consolidateMemoryIfNeeded`, agora real: `consolidate_old` ficou `async`
//!    nesta rodada (ver `memory::episodic::consolidation`), o bloqueio do runtime que impedia isto
//!    no round anterior não existe mais.
//!
//! O `turn_id` dos fatos é o id do EPISÓDIO que guarda este turno (`store_turn_memory`); antes ia
//! `None` sempre: `chat::send` publica `ThinkingStart{userContent}`/`StreamChunk{text}`/
//! `ThinkingStop{}` (confirmado lendo `chat/send.rs`, fora do escopo desta rodada) — nenhum dos três
//! carrega um id de turno, então [`super::assembler::TurnAssembler`] não tem como reconstruir um. O
//! TS também tolera isso (`turnId?: string`, `sourceTurnId` fica `undefined`).
//!
//! ## O que ainda ficou de fora, e por quê
//! - **`TopicTracker`/`extractContext`**. `TopicTracker.getActiveTopics()` não tem NENHUM leitor em
//!   todo o TypeScript (grep exaustivo em `tool-loop/*.ts`, `context/*.ts`, `brain/index.ts`) — é
//!   estado morto mesmo na origem, não portar é a decisão certa, não uma lacuna. `extractContext`
//!   (o extrator via LLM, caminho preferido de `localExtract`) também não é portado — ver o
//!   cabeçalho de `session_accumulator.rs` para o porquê.
//! - **`SessionMemoryAccumulator` — RESOLVIDO em rodada posterior.** [`super::listener`] agora
//!   alimenta [`super::session_accumulator::accumulator`] a cada turno fechado
//!   (`session_accumulator::local_extract`, a metade REGEX de `localExtract`); a injeção no prompt
//!   mora em `persona::compose_system_prompt` (fora deste diretório, mesmo lugar que as metas).
//! - **`applyGamingInfo`/`getGamingContext`**. Depende de `extractContext` (linha acima).

use tauri::AppHandle;

use crate::config::store::ConfigStore;

use super::cadence::HeavyCadence;

/// Grava o turno em memória episódica. Não usa `build_conversation_text` de `reflection.rs` (corta
/// em 400 caracteres, pensado para caber num PROMPT) — aqui o conteúdo grava a conversa inteira,
/// igual ao TS (`storeConversationMemory` não trunca).
fn conversation_content(user: &str, assistant: &str) -> String {
    let spoken = crate::modules::voice::strip_emotion_tags(assistant);
    format!("User: {user}\nMika: {spoken}")
}

/// Devolve o id do episódio gravado: é a "origem rastreável" dos fatos extraídos deste turno
/// (`source_turn_id`) — `chat::send` não publica id de turno, mas o episódio guarda a conversa.
async fn store_turn_memory(app: &AppHandle, config: &ConfigStore, user: &str, assistant: &str) -> Option<String> {
    let mem_store = match crate::modules::memory::memory_store(app) {
        Ok(store) => store,
        Err(error) => {
            log::warn!("post_turn: memória episódica indisponível, turno não gravado: {error}");
            return None;
        }
    };
    let recording_allowed = crate::modules::memory::memory_recording_allowed(config);
    let content = conversation_content(user, assistant);
    let metadata = serde_json::json!({ "type": "conversation", "source": "chat" });
    // Embedding + SQLite são síncronos: fora do worker async (que também serve `mika_invoke`).
    let stored = tauri::async_runtime::spawn_blocking(move || {
        crate::modules::memory::episodic_configured(mem_store).store(recording_allowed, &content, None, Some(&metadata))
    })
    .await;
    match stored {
        Ok(Ok(id)) => id,
        Ok(Err(error)) => {
            log::warn!("post_turn: falha ao gravar turno em memória episódica: {error}");
            None
        }
        Err(error) => {
            log::warn!("post_turn: gravação do turno em memória episódica abortou: {error}");
            None
        }
    }
}

/// Achado real (smoke item 6): um turno sem tool call cujo texto final afirma uma ação de PC feita
/// ("abri", "cliquei"...) virou memória episódica, e um turno FUTURO recitou o mesmo claim
/// alucinado de volta — a memória envenenada perpetuava o defeito em vez de só errar uma vez.
/// `used_tools` vem de [`super::assembler::TurnAssembler`] (via o bus, `EventKey::ToolCall`); só
/// pula gravação quando o texto claim SEM nenhuma tool ter rodado — o mesmo turno com tool real
/// que também diz "abri" nunca é bloqueado (o efeito colateral aconteceu de verdade).
fn should_record_turn(user: &str, assistant: &str, used_tools: bool) -> bool {
    used_tools || !crate::modules::brain::turn::action_claim::claims_untooled_action(user, assistant)
}

/// Roda o pipeline pós-turno inteiro para este par usuário/assistente — ver o cabeçalho do módulo
/// para a ordem e o que falta. Nunca propaga erro ao chamador: uma falha num passo não pode
/// derrubar o listener que processa os PRÓXIMOS turnos (mesma política do TS em cada etapa,
/// individualmente `try/catch` + `log.warn`).
pub async fn run(app: &AppHandle, config: &ConfigStore, cadence: &mut HeavyCadence, user: &str, assistant: &str, used_tools: bool) {
    if should_record_turn(user, assistant, used_tools) {
        let episode_id = store_turn_memory(app, config, user, assistant).await;
        super::facts::run(app, config, user, assistant, episode_id.as_deref()).await;
    } else {
        log::warn!("post_turn: resposta afirma ação sem tool call, pulando memória episódica/fatos (achado real: memória envenenada)");
    }

    if cadence.should_run_heavy(user, assistant) {
        super::consolidation::maybe_consolidate(app, config).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_content_mirrors_the_ts_template() {
        assert_eq!(conversation_content("oi", "olá!"), "User: oi\nMika: olá!");
    }

    // Raw `<|LAUGH|>` tags reached the memory list and came back into the prompt as recalled text.
    #[test]
    fn a_remembered_turn_keeps_what_mika_said_not_her_emotion_tags() {
        assert_eq!(conversation_content("oi", "<|LAUGH|> olá! <|SMUG:0.8|>"), "User: oi
Mika: olá!");
    }

    /// O caso central da tarefa: zero tool call + claim de ação feita nunca é gravado.
    #[test]
    fn should_record_turn_skips_an_unresolved_action_claim() {
        assert!(!should_record_turn("abre a calculadora", "Abrindo a calculadora agora!", false));
    }

    /// O MESMO texto, mas com uma tool tendo rodado de verdade — o efeito colateral aconteceu, não
    /// é memória envenenada, tem de ser gravado igual a qualquer outro turno.
    #[test]
    fn should_record_turn_keeps_the_same_claim_when_a_tool_actually_ran() {
        assert!(should_record_turn("abre a calculadora", "Abrindo a calculadora agora!", true));
    }

    /// Conversa comum, sem pedido de ação nem claim — sempre gravada.
    #[test]
    fn should_record_turn_keeps_plain_conversation() {
        assert!(should_record_turn("oi, tudo bem?", "Oi! Tudo ótimo, e você?", false));
    }

    #[test]
    fn conversation_content_never_truncates_unlike_the_reflection_snippet() {
        let long_user = "a".repeat(1000);
        let content = conversation_content(&long_user, "ok");
        assert!(content.contains(&long_user), "o pipeline pós-turno grava a conversa inteira, não um recorte de prompt");
    }

    // `run` precisa de `AppHandle`/`ConfigStore` reais (via `memory_store`/`config_store`), que não
    // dá para forjar num teste de unidade sem subir o Tauri — mesma limitação documentada em
    // `reflection/prompts.rs`. A cobertura de cada passo mora no próprio módulo: `cadence.rs`
    // (gatilho de turno pesado), `facts.rs` (parsing/ingestão), `consolidation.rs` (formatação do
    // lote), `memory::episodic::consolidation`/`service` (a consolidação em si).
}
