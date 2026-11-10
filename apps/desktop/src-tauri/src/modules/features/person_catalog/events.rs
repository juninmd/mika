//! Payloads of the person-catalog bus events. `hud_bridge` forwards them verbatim to the renderer,
//! whose contract is `{ personId, name? }` (`electron-api-events-types.ts`): publishing `id`
//! left `payload.personId` undefined, so the open detail never refreshed and a deleted person
//! stayed selected.

use serde_json::{json, Value};

use super::types::PersonRecord;
use crate::events::{bus, EventKey};

fn updated_payload(person: &PersonRecord, created: bool) -> Value {
    json!({ "personId": person.id, "name": person.primary_name, "created": created })
}

fn id_payload(id: &str) -> Value {
    json!({ "personId": id })
}

pub(crate) fn publish_updated(person: &PersonRecord, created: bool) {
    bus().publish(EventKey::PersonUpdated, updated_payload(person, created));
}

pub(crate) fn publish_mention_added(person: &PersonRecord) {
    bus().publish(EventKey::PersonMentionAdded, updated_payload(person, false));
}

pub(crate) fn publish_deleted(id: &str) {
    bus().publish(EventKey::PersonDeleted, id_payload(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person() -> PersonRecord {
        serde_json::from_value(json!({ "id": "ada-lovelace", "primaryName": "Ada Lovelace", "type": "real_person", "createdAt": 0, "updatedAt": 0 }))
            .expect("minimal PersonRecord")
    }

    #[test]
    fn updated_payload_carries_the_renderer_contract_fields() {
        let payload = updated_payload(&person(), true);
        assert_eq!(payload["personId"], "ada-lovelace");
        assert_eq!(payload["name"], "Ada Lovelace");
        assert!(payload.get("id").is_none(), "renderer reads personId, never id");
    }

    #[test]
    fn deleted_payload_carries_person_id() {
        assert_eq!(id_payload("x"), json!({ "personId": "x" }));
    }

    #[test]
    fn the_renderer_contract_still_names_person_id() {
        let ts = include_str!("../../../../../src/shared/electron-api-events-types.ts");
        assert!(ts.contains("onPersonCatalogUpdated: (callback: (payload: { personId: string; name?: string })"));
        assert!(ts.contains("onPersonCatalogDeleted: (callback: (payload: { personId: string })"));
    }
}
