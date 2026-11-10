//! "Joga com" — specs/08-gamer-companion.md ("Pessoas no jogo"): promove uma pessoa vista
//! repetidamente para `RelationshipType::PlaysWith` no catálogo, quando essa recorrência acontece
//! com uma sessão de jogo aberta. Antes desta rodada o enum não tinha essa variante nenhuma — a
//! visão já auto-cadastrava parceiros de jogo (`vision::watcher::upsert_detected_people`), mas todo
//! mundo ficava com o rótulo genérico (`stranger`/escada de afinidade), nunca "parceiro de jogo".
//!
//! **Sinal: menções vistas JOGANDO, não o total.** A rodada anterior usava
//! `PersonRecord::mention_count` (TODA menção — chat, voz, visão) como piso de recorrência,
//! documentando a aproximação ("esse total já passou do piso + sessão aberta agora" em vez de "N das
//! menções aconteceram jogando"). [`PartnerMentionTracker`] fecha esse gap: conta, por pessoa, só as
//! vezes em que `PersonMentionAdded` chegou com uma sessão de jogo JÁ aberta — uma menção no chat
//! sobre alguém fora de uma sessão nunca soma para "joga com". O contador é em memória (por processo,
//! como [`super::gaming_state::GamingState`]): reiniciar o app reseta a contagem, aceitável para uma
//! heurística de recorrência que só decide um rótulo automático, não apaga histórico nenhum.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::events::{bus, EventKey};
use crate::modules::features::person_catalog::types::RelationshipType;
use crate::modules::features::person_catalog::CatalogStore;

use super::session_memory::SessionMemory;

/// Piso de recorrência — mesma ordem de grandeza que outros limiares de "não é a primeira vez"
/// deste módulo (`gamer-companion.momentCooldowns`, `session_stats::streak`), sem exigir config
/// nova para uma heurística que ainda não tem sinal para calibrar melhor.
const RECURRING_MENTION_THRESHOLD: usize = 3;

/// Contagem de "vezes vista jogando" por pessoa — só cresce quando há sessão aberta no momento da
/// menção (ver o cabeçalho do arquivo). `spawn_partner_watcher` mantém UMA instância pelo processo,
/// mesmo padrão de outro estado de processo único deste módulo (`SessionMemory`).
#[derive(Default)]
pub struct PartnerMentionTracker {
    counts: Mutex<HashMap<String, usize>>,
}

impl PartnerMentionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Devolve a contagem JÁ incrementada para `person_id`.
    fn record_mention_while_playing(&self, person_id: &str) -> usize {
        let mut counts = self.counts.lock().unwrap_or_else(|poison| poison.into_inner());
        let count = counts.entry(person_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }
}

/// Porte testável, sem I/O de bus — [`spawn_partner_watcher`] é a única chamadora de produção.
/// `mention_count` do catálogo (total, qualquer fonte) NÃO entra mais aqui: o piso é só sobre
/// menções feitas com sessão aberta, contadas por `tracker`.
pub(super) fn promote_recurring_partner(
    catalog: &CatalogStore,
    session_memory: &SessionMemory,
    tracker: &PartnerMentionTracker,
    person_id: &str,
    now_ms: i64,
) {
    if session_memory.current_session().is_none() {
        return;
    }
    if tracker.record_mention_while_playing(person_id) < RECURRING_MENTION_THRESHOLD {
        return;
    }
    if let Err(error) = catalog.set_relationship_type(person_id, RelationshipType::PlaysWith, now_ms) {
        log::warn!("gamer: falha ao promover parceiro de jogo recorrente {person_id}: {error}");
    }
}

/// Assina `EventKey::PersonMentionAdded` (mesmo evento que `hud_bridge` já encaminha à UI) e aplica
/// [`promote_recurring_partner`] a cada um. Chamador de produção: `modules/mod.rs`, com o MESMO
/// `CatalogStore` (`features::person_catalog::catalog_store`) e `SessionMemory`
/// (`gamer::game_watcher::start`) que o resto do processo usa.
pub fn spawn_partner_watcher(catalog: &'static CatalogStore, session_memory: Arc<SessionMemory>) -> tauri::async_runtime::JoinHandle<()> {
    let mut sub = bus().subscribe();
    let tracker = PartnerMentionTracker::new();
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if envelope.key != EventKey::PersonMentionAdded {
                continue;
            }
            let Some(person_id) = envelope.payload.get("personId").and_then(|v| v.as_str()) else { continue };
            if catalog.get(person_id).is_none() {
                continue;
            }
            promote_recurring_partner(catalog, &session_memory, &tracker, person_id, super::iso_time::now_ms() as i64);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::features::person_catalog::types::UpsertInput;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-gamer-game-partners-{tag}-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn catalog() -> (CatalogStore, TempDir) {
        let dir = TempDir::new("catalog");
        (CatalogStore::open(&dir.0.join("catalog.json")).unwrap(), dir)
    }

    fn session_memory_with_an_open_session() -> Arc<SessionMemory> {
        let sm = Arc::new(SessionMemory::new(super::super::session_db::SessionDb::open_in_memory().unwrap()));
        sm.start_session("570", "Dota 2").unwrap();
        sm
    }

    #[test]
    fn promotes_a_recurring_partner_seen_while_a_game_session_is_open() {
        let (catalog, _dir) = catalog();
        let person = catalog.upsert(UpsertInput { name: "Duo Partner".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        let sm = session_memory_with_an_open_session();
        let tracker = PartnerMentionTracker::new();

        for _ in 0..RECURRING_MENTION_THRESHOLD {
            promote_recurring_partner(&catalog, &sm, &tracker, &person.id, 2000);
        }

        let reloaded = catalog.get(&person.id).unwrap();
        assert_eq!(reloaded.relationship_type, Some(RelationshipType::PlaysWith));
    }

    #[test]
    fn does_not_promote_below_the_recurrence_threshold() {
        let (catalog, _dir) = catalog();
        let person = catalog.upsert(UpsertInput { name: "New Face".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        let sm = session_memory_with_an_open_session();
        let tracker = PartnerMentionTracker::new();

        for _ in 0..(RECURRING_MENTION_THRESHOLD - 1) {
            promote_recurring_partner(&catalog, &sm, &tracker, &person.id, 2000);
        }

        let reloaded = catalog.get(&person.id).unwrap();
        assert_eq!(reloaded.relationship_type, Some(RelationshipType::Stranger), "não deveria ter sido promovido a PlaysWith");
    }

    #[test]
    fn does_not_promote_without_an_open_game_session() {
        let (catalog, _dir) = catalog();
        let person = catalog.upsert(UpsertInput { name: "Chat Only".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        let sm = Arc::new(SessionMemory::new(super::super::session_db::SessionDb::open_in_memory().unwrap()));
        let tracker = PartnerMentionTracker::new();

        for _ in 0..RECURRING_MENTION_THRESHOLD {
            promote_recurring_partner(&catalog, &sm, &tracker, &person.id, 2000);
        }

        let reloaded = catalog.get(&person.id).unwrap();
        assert_eq!(reloaded.relationship_type, Some(RelationshipType::Stranger), "não deveria ter sido promovido a PlaysWith");
    }

    /// A MUST fechada nesta rodada: menção total (fora de sessão) NUNCA soma para o piso, só as
    /// feitas com sessão aberta — mesmo se o total já bateu o piso de recorrência.
    #[test]
    fn mentions_made_without_a_session_never_count_toward_the_threshold() {
        let (catalog, _dir) = catalog();
        let person = catalog.upsert(UpsertInput { name: "Chat Regular".into(), ..Default::default() }, 1000, 100).unwrap().person.unwrap();
        let idle = Arc::new(SessionMemory::new(super::super::session_db::SessionDb::open_in_memory().unwrap()));
        let tracker = PartnerMentionTracker::new();

        // Muitas menções fora de sessão — nenhuma deve creditar o tracker.
        for _ in 0..10 {
            promote_recurring_partner(&catalog, &idle, &tracker, &person.id, 2000);
        }
        assert_eq!(catalog.get(&person.id).unwrap().relationship_type, Some(RelationshipType::Stranger));

        // Agora com sessão aberta, precisa do piso inteiro de novo — o histórico fora de sessão não
        // deixou crédito nenhum no tracker.
        let sm = session_memory_with_an_open_session();
        for _ in 0..(RECURRING_MENTION_THRESHOLD - 1) {
            promote_recurring_partner(&catalog, &sm, &tracker, &person.id, 2000);
        }
        assert_eq!(catalog.get(&person.id).unwrap().relationship_type, Some(RelationshipType::Stranger), "ainda faltava uma menção jogando");

        promote_recurring_partner(&catalog, &sm, &tracker, &person.id, 2000);
        assert_eq!(catalog.get(&person.id).unwrap().relationship_type, Some(RelationshipType::PlaysWith));
    }
}
