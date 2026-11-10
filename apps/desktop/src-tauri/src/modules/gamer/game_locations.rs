//! CRUD de `game_locations` — porte de `services/game-locations-store.ts`. Reaproveita a MESMA
//! conexão SQLite já aberta por [`super::session_db::SessionDb`] (`session_db.rs::lock`, visibilidade
//! elevada para `pub(super)` só para este arquivo poder chamar) em vez de abrir uma segunda
//! `Connection` para o mesmo arquivo — a tabela `game_locations` já é criada por
//! `SessionDb::from_connection`.

use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use super::iso_time::{format_iso8601, now_ms};
use super::session_db::SessionDb;

/// Mesmo teto de `MAX_LOCATIONS_PER_GAME` (`game-locations-store.ts`): evita crescimento sem limite
/// por jogo, prunando as mais antigas a cada inserção.
const MAX_LOCATIONS_PER_GAME: i64 = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLocation {
    pub id: String,
    pub game_name: String,
    pub location_name: String,
    pub description: String,
    pub connections: Vec<String>,
    pub important_items: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: String,
}

pub fn save_location(
    db: &SessionDb,
    game_name: &str,
    location_name: &str,
    description: &str,
    connections: &[String],
    important_items: &[String],
    notes: Option<&str>,
) -> Result<GameLocation, String> {
    let location = GameLocation {
        id: Uuid::new_v4().to_string(),
        game_name: game_name.to_string(),
        location_name: location_name.to_string(),
        description: description.to_string(),
        connections: connections.to_vec(),
        important_items: important_items.to_vec(),
        notes: notes.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
        created_at: format_iso8601(now_ms()),
    };

    let connections_json =
        serde_json::to_string(&location.connections).map_err(|e| format!("serializar connections: {e}"))?;
    let items_json =
        serde_json::to_string(&location.important_items).map_err(|e| format!("serializar importantItems: {e}"))?;

    let conn = db.lock()?;
    conn.execute(
        "INSERT INTO game_locations (id, game_name, location_name, description, connections, important_items, notes, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            location.id,
            location.game_name,
            location.location_name,
            location.description,
            connections_json,
            items_json,
            location.notes,
            location.created_at,
        ],
    )
    .map_err(|e| format!("insert location: {e}"))?;

    // Poda a mais antiga do MESMO jogo além do teto — mesma DELETE...NOT IN de `saveGameLocation`,
    // com o MESMO desempate por `rowid DESC` que `session_db.rs::history` documenta: duas
    // localizações salvas no mesmo milissegundo deixavam `ORDER BY created_at DESC` livre para
    // escolher qualquer uma como "mais recente", podando a errada de forma intermitente.
    conn.execute(
        "DELETE FROM game_locations WHERE game_name = ?1 AND id NOT IN ( \
             SELECT id FROM game_locations WHERE game_name = ?1 ORDER BY created_at DESC, rowid DESC LIMIT ?2 \
         )",
        params![game_name, MAX_LOCATIONS_PER_GAME],
    )
    .map_err(|e| format!("prune locations: {e}"))?;

    Ok(location)
}

pub fn find_locations(db: &SessionDb, game_name: &str, query: Option<&str>) -> Result<Vec<GameLocation>, String> {
    const COLUMNS: &str =
        "id, game_name, location_name, description, connections, important_items, notes, created_at";
    let conn = db.lock()?;

    match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM game_locations WHERE game_name = ?1 \
                     AND (location_name LIKE ?2 OR description LIKE ?2 OR notes LIKE ?2) \
                     ORDER BY created_at DESC, rowid DESC"
                ))
                .map_err(|e| format!("find locations: {e}"))?;
            collect_locations(&mut stmt, params![game_name, format!("%{q}%")])
        }
        None => {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM game_locations WHERE game_name = ?1 ORDER BY created_at DESC, rowid DESC"
                ))
                .map_err(|e| format!("find locations: {e}"))?;
            collect_locations(&mut stmt, params![game_name])
        }
    }
}

fn collect_locations(stmt: &mut rusqlite::Statement<'_>, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<GameLocation>, String> {
    let rows = stmt
        .query_map(args, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|e| format!("query locations: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        let (id, game_name, location_name, description, connections_json, items_json, notes, created_at) =
            row.map_err(|e| format!("row locations: {e}"))?;
        out.push(GameLocation {
            id,
            game_name,
            location_name,
            description,
            connections: parse_string_array(&connections_json),
            important_items: parse_string_array(&items_json),
            notes: notes.filter(|s| !s.is_empty()),
            created_at,
        });
    }
    Ok(out)
}

fn parse_string_array(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gamer::test_support::ScratchDir;

    fn db() -> (SessionDb, ScratchDir) {
        let scratch = ScratchDir::new("game-locations");
        let db = SessionDb::open(&scratch.path().join("gamer-sessions.db")).unwrap();
        (db, scratch)
    }

    #[test]
    fn save_then_find_round_trips_all_fields() {
        let (db, _scratch) = db();
        let saved = save_location(
            &db,
            "Hollow Knight",
            "City of Tears",
            "Grande cidade afundada, cheia de inimigos voadores.",
            &["Resting Grounds".to_string()],
            &["Lumafly Lantern".to_string()],
            Some("Cuidado com os Husk Guards."),
        )
        .unwrap();
        assert_eq!(saved.game_name, "Hollow Knight");

        let found = find_locations(&db, "Hollow Knight", None).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location_name, "City of Tears");
        assert_eq!(found[0].connections, vec!["Resting Grounds"]);
        assert_eq!(found[0].notes.as_deref(), Some("Cuidado com os Husk Guards."));
    }

    #[test]
    fn find_filters_by_game_name_never_leaks_across_games() {
        let (db, _scratch) = db();
        save_location(&db, "Hades", "Tartarus", "desc", &[], &[], None).unwrap();
        save_location(&db, "Elden Ring", "Limgrave", "desc", &[], &[], None).unwrap();

        let found = find_locations(&db, "Hades", None).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].game_name, "Hades");
    }

    #[test]
    fn query_matches_location_name_description_or_notes() {
        let (db, _scratch) = db();
        save_location(&db, "Dota 2", "Roshan Pit", "onde o Roshan mora", &[], &[], Some("cuidado com wards")).unwrap();
        save_location(&db, "Dota 2", "Secret Shop", "vende itens raros", &[], &[], None).unwrap();

        let by_name = find_locations(&db, "Dota 2", Some("roshan")).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].location_name, "Roshan Pit");

        let by_notes = find_locations(&db, "Dota 2", Some("wards")).unwrap();
        assert_eq!(by_notes.len(), 1);
        assert_eq!(by_notes[0].location_name, "Roshan Pit");
    }

    #[test]
    fn staying_under_the_cap_prunes_nothing() {
        let (db, _scratch) = db();
        for i in 0..3 {
            save_location(&db, "Cap Test", &format!("Loc {i}"), "desc", &[], &[], None).unwrap();
        }
        let found = find_locations(&db, "Cap Test", None).unwrap();
        assert_eq!(found.len(), 3, "abaixo do teto, nada deveria ter sido podado");
    }

    /// Prova o pruning de verdade: insere acima do teto e confere que (a) o total nunca ultrapassa
    /// `MAX_LOCATIONS_PER_GAME` e (b) a localização mais RECENTE sobrevive — nunca a podada por
    /// engano. `rowid DESC` como desempate (ver o comentário em `save_location`) é o que garante (b)
    /// quando duas linhas caem no mesmo `created_at` de milissegundo.
    #[test]
    fn saving_past_the_cap_prunes_the_oldest_and_keeps_the_total_at_the_cap() {
        let (db, _scratch) = db();
        let extra = 5;
        for i in 0..(MAX_LOCATIONS_PER_GAME as usize + extra) {
            save_location(&db, "Cap Test", &format!("Loc {i}"), "desc", &[], &[], None).unwrap();
        }

        let found = find_locations(&db, "Cap Test", None).unwrap();
        assert_eq!(found.len(), MAX_LOCATIONS_PER_GAME as usize);
        assert_eq!(found[0].location_name, format!("Loc {}", MAX_LOCATIONS_PER_GAME as usize + extra - 1), "a mais recente tem de sobreviver ao pruning");
    }

    #[test]
    fn pruning_never_touches_a_different_game() {
        let (db, _scratch) = db();
        save_location(&db, "Other Game", "Keep Me", "desc", &[], &[], None).unwrap();
        for i in 0..(MAX_LOCATIONS_PER_GAME as usize + 1) {
            save_location(&db, "Cap Test", &format!("Loc {i}"), "desc", &[], &[], None).unwrap();
        }

        let other = find_locations(&db, "Other Game", None).unwrap();
        assert_eq!(other.len(), 1, "pruning de um jogo não pode apagar localização de outro jogo");
    }
}
