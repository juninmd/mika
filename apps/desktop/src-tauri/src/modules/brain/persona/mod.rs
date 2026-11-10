//! Módulo `persona` — o `system.md` rico (15 placeholders) que `brain::turn::compose_system_prompt`
//! usava só parcialmente até esta rodada (ver o item "Sem `prompts/system.md`" no cabeçalho de
//! `turn.rs`, que este módulo fecha).
//!
//! - [`axes`] — port de `persona-axes.ts`/`persona-axes-render.ts`: os 15 eixos de calibragem
//!   (`{{personaAxes}}`).
//! - [`personality`] — port de `persona-personality.ts`: os 4 perfis (`{{personalityProfile}}`).
//! - [`user_context`] — port de `user-profile-context.ts`: o bloco "USER PROFILE AND HABITS".
//! - [`gaming_context`] — fatia jogo+duração de `brain/engine/gaming-context.ts` (pull-based, ver o
//!   cabeçalho do arquivo), [`screen_context`] — "o que está na tela agora" (pull-based sobre um
//!   holder alimentado por assinante de `EventKey::ScreenObservation`, fecha specs/08-gamer-
//!   companion.md §"Ajuda no jogo" e specs/06-vision.md; ver o cabeçalho do arquivo) e
//!   [`crate::modules::features::character::slang_context`] (gíria por modo) alimentam o
//!   `dynamic_context` de `system_prompt::build_full_system_prompt` — a lacuna que o cabeçalho de
//!   `system_prompt.rs` chamava de "sempre vazio" está fechada nesta rodada, na fachada
//!   [`compose_system_prompt`] abaixo (`brain::turn::compose_system_prompt` chama esta, não
//!   `system_prompt::build_full_system_prompt` direto).
//! - [`user_clock`] — o relógio do usuário (`user.timezone` IANA/deslocamento, senão o SO) usado por
//!   "## TIME AWARENESS" e pelas necessidades da Mika (`features::mood::needs`).
//! - [`routine_context`] — corpo da Mika (necessidades) + hábitos do usuário (top programas/jogos).
//! - [`system_prompt`] — a composição final (`buildSystemPrompt`/`buildSplitSystemPrompt`).
//!
//! `{{relationshipBlock}}`/`{{relationshipStatus}}` e o loader do `.md` ficam em `modules::chat`
//! (`persona_relationship.rs`/`prompt.rs`) — não duplicados aqui, reaproveitados via os re-exports
//! `pub(crate)` que `chat/mod.rs` já abre para isso, mesmo padrão de "porta única por módulo" que o
//! resto do `brain` segue para `chat`/`tools`.
//!
//! **`init_prompts_root` — wiring fechado nesta rodada.** `lib.rs::setup` agora chama
//! `crate::modules::chat::init_prompts_root(&handle)` junto das outras dependências de boot
//! (`gamer::game_watcher::start`, `features::mood::persistence::register`); sem ela, build
//! empacotado caía no fallback de dev (`out/prompts` ao lado do crate, inexistente fora da árvore de
//! source) e [`system_prompt::build_full_system_prompt`] falharia alto em vez de servir um
//! `system.md` vazio. Ver o cabeçalho de `chat/prompt.rs`.

pub mod axes;
mod facts_block;
pub mod gaming_context;
mod people_block;
pub mod personality;
mod routine_context;
pub mod screen_context;
pub mod system_prompt;
pub(crate) mod user_clock;
mod user_context;

use crate::config::store::ConfigStore;

/// Fachada que `brain::turn::compose_system_prompt` chama a cada turno de CHAT: monta o
/// `dynamic_context` (todos pull-based — sem `EventKey` novo, sem `ContextRegistry` genérico, cada
/// fonte concatenada direto aqui), delega a [`system_prompt::build_full_system_prompt`] e injeta
/// humor/vínculo por cima (`chat::inject_persona_state`) — a mesma ordem de
/// `turn-context.ts::composeSystemPrompt` (`buildSystemPrompt` → `injectPersonaState`). O caminho
/// "raw" não passa por aqui: no TS ele usa `buildSystemPrompt` sem injeção. Erro (`.md` ausente) sobe
/// honesto; `turn.rs` decide como degradar, não este módulo.
///
/// A ORDEM das sete fontes espelha a prioridade de "Recuperação para o prompt"
/// (04-memory.md): gíria/memória de sessão/jogo ativo/metas (pull-based, prioridade do TS —
/// `ContextRegistry.sort((a,b) => b.priority - a.priority)`: gíria 90, sessão 80, jogo 75, metas 40)
/// primeiro, e as três fontes que a rodada anterior fechou por último — fatos de longo prazo, pessoas
/// citadas na mensagem e episódios semânticos — porque no orçamento por prioridade da spec elas vêm
/// DEPOIS de humor/vínculo/janela recente, não antes (corte de baixo para cima quando o orçamento
/// estourar: as três primeiras a sair são justamente `dynamic_context` das últimas seções, nunca a
/// persona nem a conversa recente). [`screen_context`] entra logo depois de `gaming_context` (mesma
/// prioridade de contexto "agora" que o jogo ativo, sem `EventKey` própria do TS para se ancorar —
/// specs/08-gamer-companion.md §"Ajuda no jogo" não define prioridade numérica, só o SHOULD de que a
/// tela participa do chat regular). Das sete, só `screen_context` é alimentada por um assinante de
/// `EventKey` (`ScreenObservation`, ver o cabeçalho do arquivo); as outras seis continuam pull direto
/// sobre estado de processo. A ordem de concatenação aqui faz o mesmo papel do `sort` por prioridade
/// do registro real.
///
/// `user_content` é o texto do turno ATUAL — usado só para resolver pessoas citadas
/// ([`crate::modules::features::person_catalog::catalog_reader`]) e buscar episódios relacionados
/// (`episodic_reader().build_context`, semântico com fallback por palavra-chave). Nenhuma das duas
/// buscas chama LLM: são o caminho SÍNCRONO do turno (MUST de 04-memory.md — extração roda depois,
/// fora do caminho de resposta; isto aqui é LEITURA do que já foi extraído em turnos anteriores).
///
/// `async` só por causa dos episódios: o embedding da mensagem é a única ida à rede daqui. Ele sai
/// PRIMEIRO no `join!` (o primeiro poll já despacha o pedido) e as outras sete fontes, locais e
/// síncronas, são montadas enquanto ele está em voo — antes, o turno bloqueava o worker async
/// esperando o vetor e só então montava o resto.
pub async fn compose_system_prompt(store: &ConfigStore, user_content: &str) -> Result<String, String> {
    let (episodic, mut dynamic_context) = tokio::join!(build_episodic_context_block(user_content), async { local_context_blocks(store) });
    // After recall: the message's embedding is now cached, so ranking facts by it costs no wait.
    dynamic_context.push_str(&facts_block::build_long_term_facts_block(user_content));
    dynamic_context.push_str(&people_block::build_mentioned_people_block(user_content));
    dynamic_context.push_str(&episodic);
    let prompt = system_prompt::build_full_system_prompt(store, &dynamic_context)?;
    Ok(crate::modules::chat::inject_persona_state(&prompt, &crate::modules::chat::gather_persona_state(store)))
}

/// As fontes de `dynamic_context` que não dependem da mensagem, na ordem de prioridade descrita acima.
fn local_context_blocks(store: &ConfigStore) -> String {
    let mut dynamic_context = String::new();
    dynamic_context.push_str(&crate::modules::features::character::slang_context::build_slang_block(store));
    dynamic_context.push_str(&crate::modules::brain::post_turn::session_accumulator::accumulator().build_context());
    dynamic_context.push_str(&gaming_context::build_gaming_context_block());
    dynamic_context.push_str(&screen_context::build_screen_context_from_store(store));
    dynamic_context.push_str(&routine_context::build_routine_block(store));
    dynamic_context.push_str(&build_goals_context_block());
    dynamic_context
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    format!("{}…", text.chars().take(max_chars).collect::<String>())
}

/// Item 5 de "Recuperação para o prompt": episódios semanticamente próximos de `user_content`.
/// `EpisodicMemoryService::build_context` já resolve semântico-com-fallback-por-palavra-chave E o
/// teto (`rag_top_k`, default 5) — nenhuma lógica nova aqui, só o encaixe no turno de chat regular
/// que faltava (04-memory.md linha 151).
async fn build_episodic_context_block(user_content: &str) -> String {
    if user_content.trim().is_empty() {
        return String::new();
    }
    let Some(episodic) = crate::modules::memory::episodic_reader() else { return String::new() };
    episodic.build_context(user_content).await.unwrap_or_default()
}

/// Espelha `getBrainContext(goals)` (`brain-goals-query.ts`): as 3 metas ativas de maior prioridade
/// (já vêm ordenadas por `reflection::goals::get_active_goals`), mesmo formato do TS —
/// `"\n\n## Active Goals\n- [P{priority}] {description}"` por linha, string vazia sem metas.
/// Mesmo mecanismo pull-based de [`gaming_context::build_gaming_context_block`] (sem `EventKey`
/// novo, sem `ContextRegistry` — Rust não tem um registro de contexto genérico; cada fonte se
/// concatena direto aqui). `memory_store_reader()`/o `MemoryStore` podem não existir ainda (antes do
/// boot terminar de registrar `register_config_bound_tools`) — string vazia nesse caso, igual a
/// qualquer outra fonte pull-based deste arquivo quando a fonte está indisponível.
fn build_goals_context_block() -> String {
    let Some(mem) = crate::modules::memory::memory_store_reader() else { return String::new() };
    let Ok(goals) = crate::modules::brain::reflection::goals::get_active_goals(mem) else { return String::new() };
    format_goals_block(&goals)
}

/// Formatação pura, separada de [`build_goals_context_block`] só para ser testável sem depender do
/// `MemoryStore` de processo (populado só depois do boot real — ver `memory::memory_store_reader`,
/// mesma limitação já documentada em `episodic_reader`/`social_reader`).
fn format_goals_block(goals: &[crate::modules::brain::reflection::goals::GoalEntry]) -> String {
    if goals.is_empty() {
        return String::new();
    }
    let lines = goals.iter().take(3).map(|g| format!("- [P{}] {}", g.priority, g.description)).collect::<Vec<_>>().join("\n");
    format!("\n\n## Active Goals\n{lines}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::brain::reflection::goals::GoalEntry;

    fn goal(description: &str, priority: f64) -> GoalEntry {
        GoalEntry { id: description.to_string(), description: description.to_string(), priority, created_at: 0 }
    }

    #[test]
    fn format_goals_block_is_empty_without_any_active_goal() {
        assert_eq!(format_goals_block(&[]), "");
    }

    /// Mesmo formato de `getBrainContext` (`brain-goals-query.ts`): cabeçalho + até 3 linhas
    /// "- [P{priority}] {description}", a quarta meta (menor prioridade) NUNCA aparece.
    #[test]
    fn format_goals_block_matches_the_typescript_header_and_caps_at_three() {
        let goals = vec![goal("ser mais direta", 8.0), goal("falar menos de política", 6.0), goal("puxar mais assunto de games", 5.0), goal("nunca deveria aparecer", 1.0)];

        let block = format_goals_block(&goals);
        assert_eq!(block, "\n\n## Active Goals\n- [P8] ser mais direta\n- [P6] falar menos de política\n- [P5] puxar mais assunto de games");
        assert!(!block.contains("nunca deveria aparecer"));
    }

    /// Correção sobre a rodada anterior: `SessionMemoryAccumulator.asContextProvider()` é um
    /// provider do MESMO `ContextRegistry` que `getBrainContext()` (metas) usa — entra no turno de
    /// chat REGULAR, não no prompt raw. Usa o singleton de processo real (mesmo que
    /// `post_turn::listener` alimenta em produção); string distintiva para não colidir com nenhum
    /// outro teste que porventura toque o mesmo singleton.
    #[tokio::test]
    async fn compose_system_prompt_includes_accumulated_session_facts_in_the_regular_turn() {
        let dir = std::env::temp_dir().join(format!("mika-persona-compose-session-memory-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        crate::modules::brain::post_turn::session_accumulator::accumulator()
            .add_facts(&["prefere respostas curtas e diretas nesta rodada de teste de persona".to_string()], &[], &[]);

        let prompt = compose_system_prompt(&store, "").await.expect("system.md deveria existir via fallback de dev");
        let _ = std::fs::remove_dir_all(&dir);

        assert!(prompt.contains("## Established In This Conversation"));
        assert!(prompt.contains("prefere respostas curtas e diretas nesta rodada de teste de persona"));
    }

    /// Ponta a ponta com o `system.md`/`system-tail.md` reais (fallback de dev): o vínculo entra
    /// antes de "## CRITICAL STYLE RULES", a afinidade vem do placar persistido e nenhum placeholder
    /// sobra cru. Sem a injeção, o turno de chat perdia o bloco de vínculo que o TS sempre manda.
    #[tokio::test]
    async fn compose_system_prompt_injects_the_bond_block_before_the_critical_guidelines() {
        let dir = std::env::temp_dir().join(format!("mika-persona-compose-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        store.set("character.affinityScore", serde_json::json!(-20.0)).unwrap();

        let prompt = compose_system_prompt(&store, "").await.expect("system.md deveria existir via fallback de dev");
        let _ = std::fs::remove_dir_all(&dir);

        let bond_at = prompt.find("## Bond With The User").expect("bloco de vínculo ausente");
        let guidelines_at = prompt.find("## CRITICAL STYLE RULES").expect("system-tail ausente");
        assert!(bond_at < guidelines_at);
        assert!(prompt.contains("Current affinity: 40/100"), "(-20 + 100) / 2 = 40");
        assert!(!prompt.contains("{{"), "placeholder cru sobrou no prompt");
    }

    /// Prompts are English; only the reply follows `{{language}}` (brain/system.md). The blocks this
    /// module builds from code (profile, relationship, axes) must carry no Portuguese function
    /// words; the mood/bond blocks have the same guard in `chat::persona_composer`.
    #[test]
    fn code_built_persona_blocks_carry_no_portuguese_function_words() {
        let dir = std::env::temp_dir().join(format!("mika-persona-english-guard-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        let mut texts = vec![axes::build_axes_block(&store)];
        let _ = std::fs::remove_dir_all(&dir);
        for profile in ["professional", "friend", "tsundere", "sarcastic"] {
            texts.push(personality::build_profile_block(profile));
        }
        for level in ["acquaintance", "friend", "close-friend", "companion"] {
            texts.push(crate::modules::chat::build_relationship_block(level));
        }
        let portuguese = regex::Regex::new(r"(?i)\b(você|vocês|não|com o|com a|uma|está|sem|para o|para a|dele|dela|seu|sua|nunca|sempre|mesmas)\b").unwrap();
        for text in &texts {
            if let Some(hit) = portuguese.find(text) {
                panic!("Portuguese `{}` in a persona block:\n{text}", hit.as_str());
            }
        }
    }
}
