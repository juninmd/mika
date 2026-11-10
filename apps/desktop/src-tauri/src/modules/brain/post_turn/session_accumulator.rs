//! === engine/session-memory.ts (`SessionMemoryAccumulator`) + a fatia de `chat/pipeline.ts` que a
//! alimenta (`localExtract`/`applyExtraction`) ======================================================
//!
//! Fecha o "alicerce sem laço" que `reflection::goals` documentava: TS tem DOIS consumidores reais
//! (`asContextProvider()` → prompt, `flush()` → memória episódica). **Correção sobre uma rodada
//! anterior:** o ponto de injeção no prompt NÃO é `turn::build_raw_system_prompt` — checado de novo,
//! `asContextProvider()` registra no MESMO `ContextRegistry` que `getBrainContext()` (metas) usa,
//! prioridade 80 (`session-memory.ts`), consumido só pelo turno de chat REGULAR
//! (`turn-context.ts::prepareTurnContext`). A injeção real mora em
//! [`super::super::persona::compose_system_prompt`] (`brain/persona/mod.rs`, minha propriedade),
//! junto de gíria/jogo/metas, na ordem de prioridade do `ContextRegistry` real — ver [`accumulator`]
//! para o singleton.
//!
//! ## Produtor: [`local_extract`], não `extractContext`
//!
//! TS tem DOIS extratores: `extractContext` (LLM, caminho preferido) com `localExtract` (regex) como
//! FALLBACK quando o primeiro falha/expira. Só [`local_extract`] é portado aqui — `extractContext`
//! duplicaria uma segunda chamada de LLM por turno só para preencher `sessionMemory`, quando
//! `post_turn::facts::run` já roda UMA extração via LLM a cada turno (prompt/schema próprios, outro
//! propósito). `userPreferences` fica sempre vazio, igual ao próprio `localExtract` do TS (só
//! `extractContext` preenche esse campo). Reportado, não escondido.
//!
//! ## Estado de processo, `flush` amarrado ao ENCERRAMENTO do app (não a um "fim de sessão")
//!
//! TS zera o acumulador (`flush` → memória episódica, depois `clear()`) no fim de uma sessão de chat
//! (`chat-lifecycle.service.ts`). Rust não tem esse conceito exposto a `post_turn` — só
//! `ThinkingStop` por turno, sem um evento de sessão fechando (decisão registrada em defects.md
//! "memory-relationship" P2: persistir, não apagar, o que foi acumulado). A saída escolhida foi o
//! único "fim" que o processo Rust TEM: [`SessionMemoryAccumulator::flush`] é chamado em
//! `lib.rs::run`, dentro do `RunEvent::ExitRequested`, ao lado de `modules::mcp::disconnect_all()` —
//! sem isso, "o que ficou estabelecido nessa conversa" (preferências/entidades/tópicos regex) SEMPRE
//! morria com o processo, mesmo depois de um app inteiro de uso. **Idle consolidation (short → long term).** After [`super::listener::IDLE_CONSOLIDATION`] with no
//! turn, the listener flushes the accumulator to episodic memory and `clear()`s it — a long day of
//! use no longer depends on a clean app exit to keep what was established.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use regex::Regex;

use crate::modules::memory::MemoryStore;

/// = `MAX_TOKENS_APPROX * CHARS_PER_TOKEN` (`session-memory.ts`).
const TRIM_LIMIT_CHARS: usize = 300 * 4;

struct SessionFact {
    fact: String,
    key: String,
}

#[derive(Default)]
struct Inner {
    facts: Vec<SessionFact>,
    key_set: HashSet<String>,
}

/// Porte de `SessionMemoryAccumulator`. Ver o cabeçalho do arquivo para o que NÃO tem chamador ainda.
pub struct SessionMemoryAccumulator(Mutex<Inner>);

/// Mesmo mapa de acentos latinos já aceito em `modules/memory/text.rs`/`modules/brain/guards.rs` —
/// duplicado, não reaproveitado: os dois vivem em módulos fora do meu escopo nesta rodada
/// (`memory`/`brain::guards`), e abrir a visibilidade deles só para esta função pediria editar um
/// arquivo que não é meu. Mesmo precedente já documentado em `chat/prompt.rs` sobre duplicar leitura
/// pequena através de um limite de módulo.
fn strip_diacritics(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Porte de `normalizeKey`: baixa caixa, tira diacrítico, mantém só alfanumérico/espaço, colapsa
/// espaços. Usado para deduplicar fatos por conteúdo normalizado, nunca para exibição.
fn normalize_key(text: &str) -> String {
    let stripped = strip_diacritics(&text.to_lowercase());
    let filtered: String = stripped.chars().filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace()).collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl SessionMemoryAccumulator {
    fn new() -> Self {
        Self(Mutex::new(Inner::default()))
    }

    /// Porte de `addFacts`: `"Preference: "`/`"Mentioned: "`/`"Topic: "`, os dois últimos só para
    /// itens com mais de 2 caracteres, deduplicados por [`normalize_key`], depois [`trim`].
    pub fn add_facts(&self, user_preferences: &[String], entities: &[String], topics: &[String]) {
        let mut candidates: Vec<String> = Vec::new();
        candidates.extend(user_preferences.iter().map(|p| format!("Preference: {p}")));
        candidates.extend(entities.iter().filter(|e| e.chars().count() > 2).map(|e| format!("Mentioned: {e}")));
        candidates.extend(topics.iter().filter(|t| t.chars().count() > 2).map(|t| format!("Topic: {t}")));

        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for fact in candidates {
            let key = normalize_key(&fact);
            if inner.key_set.insert(key.clone()) {
                inner.facts.push(SessionFact { fact, key });
            }
        }
        trim(&mut inner);
    }

    /// Called after an idle consolidation (`listener`): what was flushed to long-term memory leaves
    /// the short-term block, so the next session starts clean.
    pub fn clear(&self) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.facts.clear();
        inner.key_set.clear();
    }

    /// Porte de `flush`: grava os fatos acumulados como uma memória episódica só (`type:
    /// session_fact`), sem esvaziar o acumulador (mesmo contrato do TS — `clear()` é passo separado
    /// de quem chama). Chamado por `lib.rs::run` no `RunEvent::ExitRequested` — ver o cabeçalho do
    /// arquivo.
    pub fn flush(&self, mem: &MemoryStore, recording_allowed: bool) -> Result<(), String> {
        let content = {
            let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if inner.facts.is_empty() {
                return Ok(());
            }
            inner.facts.iter().map(|f| format!("- {}", f.fact)).collect::<Vec<_>>().join("\n")
        };
        let service = crate::modules::memory::episodic_configured(mem);
        let metadata = serde_json::json!({ "type": "session_fact" });
        service.store(recording_allowed, &content, None, Some(&metadata)).map(|_| ())
    }

    /// The accumulated lines, oldest first — the memory panel's short-term view.
    pub fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).facts.iter().map(|f| f.fact.clone()).collect()
    }

    /// Porte de `buildContext`: string vazia sem fato acumulado, senão o mesmo cabeçalho do TS —
    /// pronta para entrar direto no `dynamic_context` de `turn::build_raw_system_prompt`.
    pub fn build_context(&self) -> String {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if inner.facts.is_empty() {
            return String::new();
        }
        let lines = inner.facts.iter().map(|f| format!("- {}", f.fact)).collect::<Vec<_>>().join("\n");
        format!("\n\n## Established In This Conversation\n{lines}\n")
    }
}

/// Porte de `SessionMemoryAccumulator.trim`: mantém as entradas mais RECENTES até o teto de
/// caracteres, cortando as mais antigas do início — mesma varredura de trás para frente do TS.
fn trim(inner: &mut Inner) {
    let mut total_chars = 0usize;
    let mut cut_from = inner.facts.len();
    for i in (0..inner.facts.len()).rev() {
        let next = total_chars + inner.facts[i].fact.chars().count();
        if next > TRIM_LIMIT_CHARS {
            cut_from = i + 1;
            break;
        }
        total_chars = next;
        cut_from = i;
    }
    if cut_from > 0 {
        inner.facts.drain(0..cut_from);
        inner.key_set = inner.facts.iter().map(|f| f.key.clone()).collect();
    }
}

static ACCUMULATOR: OnceLock<SessionMemoryAccumulator> = OnceLock::new();

/// Singleton de processo — mesmo padrão de `tools::registry()`/`features::mood::engine()`. Ver o
/// item "Estado de processo" no cabeçalho do arquivo para o porquê de nunca ser zerado em produção.
pub fn accumulator() -> &'static SessionMemoryAccumulator {
    ACCUMULATOR.get_or_init(SessionMemoryAccumulator::new)
}

fn entity_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[A-Z][a-záéíóúãõç]{2,}\b").unwrap())
}

/// Porte de `localExtract` (`brain/chat/pipeline.ts`) — ver o cabeçalho do arquivo para o porquê de
/// `extractContext` (o caminho LLM preferido no TS) não ser portado. `user_preferences` sempre vazio,
/// igual ao original.
pub fn local_extract(user: &str, assistant: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let combined = format!("{user} {assistant}");

    let mut entities: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for m in entity_regex().find_iter(&combined) {
        let word = m.as_str().to_string();
        if seen.insert(word.clone()) {
            entities.push(word);
            if entities.len() == 5 {
                break;
            }
        }
    }

    let mut freq: HashMap<String, u32> = HashMap::new();
    for word in combined.to_lowercase().split_whitespace() {
        if word.chars().count() <= 4 {
            continue;
        }
        let clean = normalize_key(word);
        if clean.chars().count() > 4 {
            *freq.entry(clean).or_insert(0) += 1;
        }
    }
    let mut ranked: Vec<(String, u32)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let topics: Vec<String> = ranked.into_iter().take(3).map(|(word, _)| word).collect();

    (Vec::new(), entities, topics)
}

#[cfg(test)]
mod tests {
    use crate::modules::memory::episodic::EpisodicMemoryService;
    use super::*;

    // -- normalize_key / trim / add_facts (sobre uma instância local, sem o singleton) -------------

    #[test]
    fn normalize_key_strips_diacritics_punctuation_and_case() {
        assert_eq!(normalize_key("Mencionado: Rust!"), "mencionado rust");
    }

    #[test]
    fn add_facts_prefixes_by_category_and_filters_short_entities_and_topics() {
        let acc = SessionMemoryAccumulator::new();
        acc.add_facts(&["gosta de rpg".to_string()], &["Godot".to_string(), "Oi".to_string()], &["programacao".to_string(), "ok".to_string()]);

        let ctx = acc.build_context();
        assert!(ctx.contains("- Preference: gosta de rpg"));
        assert!(ctx.contains("- Mentioned: Godot"));
        assert!(!ctx.contains("Oi"), "entidade com 2 chars ou menos é descartada");
        assert!(ctx.contains("- Topic: programacao"));
        assert!(!ctx.contains("Topic: ok"), "tópico com 2 chars ou menos é descartado");
    }

    #[test]
    fn add_facts_deduplicates_by_normalized_key() {
        let acc = SessionMemoryAccumulator::new();
        acc.add_facts(&[], &["Rust".to_string()], &[]);
        acc.add_facts(&[], &["rust!".to_string()], &[]);

        let ctx = acc.build_context();
        assert_eq!(ctx.matches("Mentioned:").count(), 1, "mesma entidade normalizada não deveria duplicar");
    }

    #[test]
    fn build_context_is_empty_without_any_fact() {
        let acc = SessionMemoryAccumulator::new();
        assert_eq!(acc.build_context(), "");
    }

    #[test]
    fn build_context_matches_the_typescript_header_and_bullets() {
        let acc = SessionMemoryAccumulator::new();
        acc.add_facts(&["curte anime".to_string()], &[], &[]);
        assert_eq!(acc.build_context(), "\n\n## Established In This Conversation\n- Preference: curte anime\n");
    }

    /// NEGATIVO — o acumulador nunca cresce sem limite: fatos antigos caem quando o total passa do
    /// teto de ~1200 caracteres (300 tokens aproximados), mantendo os mais RECENTES.
    #[test]
    fn trim_drops_the_oldest_facts_once_the_character_budget_overflows() {
        let acc = SessionMemoryAccumulator::new();
        for i in 0..50 {
            acc.add_facts(&[format!("preferencia numero {i} com bastante texto para encher o orcamento de caracteres")], &[], &[]);
        }
        let ctx = acc.build_context();
        assert!(!ctx.contains("numero 0 "), "a entrada mais antiga deveria ter sido cortada");
        assert!(ctx.contains("numero 49 "), "a entrada mais recente deveria sobreviver");
        assert!(ctx.chars().count() < 50 * 90, "o orçamento deveria ter cortado a maioria das entradas");
    }

    #[test]
    fn clear_empties_the_accumulator() {
        let acc = SessionMemoryAccumulator::new();
        acc.add_facts(&["x".to_string()], &[], &[]);
        acc.clear();
        assert_eq!(acc.build_context(), "");
    }

    #[test]
    fn flush_writes_a_single_session_fact_memory_and_keeps_the_accumulator_filled() {
        let mem = MemoryStore::open_in_memory().unwrap();
        let acc = SessionMemoryAccumulator::new();
        acc.add_facts(&["gosta de rpg".to_string()], &[], &[]);

        acc.flush(&mem, true).unwrap();

        let service = EpisodicMemoryService::new(&mem);
        let all = service.get_all().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].content.contains("gosta de rpg"));
        assert_ne!(acc.build_context(), "", "flush não deveria esvaziar sozinho — clear() é passo separado");
    }

    #[test]
    fn flush_is_a_no_op_without_any_accumulated_fact() {
        let mem = MemoryStore::open_in_memory().unwrap();
        let acc = SessionMemoryAccumulator::new();
        acc.flush(&mem, true).unwrap();

        let service = EpisodicMemoryService::new(&mem);
        assert!(service.get_all().unwrap().is_empty());
    }

    // -- local_extract -------------------------------------------------------------------------

    #[test]
    fn local_extract_always_returns_empty_user_preferences() {
        let (preferences, _entities, _topics) = local_extract("oi Godot", "olá!");
        assert!(preferences.is_empty(), "mesmo o TS: só extractContext (LLM, não portado) preenche preferências");
    }

    #[test]
    fn local_extract_collects_up_to_five_unique_capitalized_entities_in_order() {
        let user = "Godot e Rust e Python e Java e Kotlin e Godot de novo e Elixir";
        let (_preferences, entities, _topics) = local_extract(user, "");
        assert_eq!(entities, vec!["Godot", "Rust", "Python", "Java", "Kotlin"], "seis nomes únicos, só os 5 primeiros, sem repetir Godot");
    }

    #[test]
    fn local_extract_ranks_topics_by_frequency_among_words_longer_than_four_chars() {
        let text = "programacao programacao programacao jogos jogos rpg";
        let (_preferences, _entities, topics) = local_extract(text, "");
        assert_eq!(topics.first().map(String::as_str), Some("programacao"), "palavra mais frequente com mais de 4 letras vence");
    }

    #[test]
    fn local_extract_ignores_short_words_for_topics() {
        let (_preferences, _entities, topics) = local_extract("oi tudo bem com voce hoje", "sim");
        assert!(topics.is_empty(), "nenhuma palavra tem mais de 4 caracteres depois de limpa");
    }
}
