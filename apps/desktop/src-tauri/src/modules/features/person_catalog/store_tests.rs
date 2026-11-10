//! Suíte de `store.rs`, em arquivo separado só pela regra de 300 linhas do AGENTS.md (mesmo
//! truque de `modules::plugins::rpc.rs`/`rpc_tests.rs`).

use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mika-person-catalog-store-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn input(name: &str) -> UpsertInput {
    UpsertInput { name: name.to_string(), ..Default::default() }
}

#[test]
fn upsert_then_get_round_trips() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    let outcome = store.upsert(input("Ada Lovelace"), 1000, 100).unwrap();
    let id = outcome.person.unwrap().id;
    assert_eq!(store.get(&id).unwrap().primary_name, "Ada Lovelace");
}

#[test]
fn reopening_the_same_file_preserves_the_catalog_across_restarts() {
    let dir = TempDir::new();
    let path = dir.0.join("catalog.json");
    let id = {
        let store = CatalogStore::open(&path).unwrap();
        store.upsert(input("Ada"), 1000, 100).unwrap().person.unwrap().id
    };
    {
        let store = CatalogStore::open(&path).unwrap();
        assert_eq!(store.get(&id).unwrap().primary_name, "Ada");
    }
}

#[test]
fn a_missing_file_starts_with_an_empty_catalog_not_an_error() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("does-not-exist.json")).unwrap();
    assert!(store.list(None, None, 20).is_empty());
}

#[test]
fn a_corrupted_file_backs_up_and_starts_empty_instead_of_failing_the_open() {
    let dir = TempDir::new();
    let path = dir.0.join("catalog.json");
    std::fs::write(&path, b"isto nao e json").unwrap();

    let store = CatalogStore::open(&path).unwrap();
    assert!(store.list(None, None, 20).is_empty());
    store.upsert(input("Ada"), 1000, 100).unwrap();

    let backups: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("corrupt"))
        .collect();
    assert_eq!(backups.len(), 1, "arquivo corrompido devia ter sido preservado como backup");
}

#[test]
fn clear_all_empties_persons_and_alias_map_but_keeps_social_imported_at() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    let id = store.upsert(input("Ada"), 1000, 100).unwrap().person.unwrap().id;
    {
        let mut index = store.lock();
        index.social_imported_at = Some(4242);
    }

    let removed = store.clear_all().unwrap();
    assert_eq!(removed, 1);
    assert!(store.get(&id).is_none());
    assert!(store.list(None, None, 20).is_empty());
    assert_eq!(store.lock().social_imported_at, Some(4242), "socialImportedAt não pode ser apagado pelo wipe");

    // Sobrevive a um reopen — prova que foi persistido, não só limpo em memória.
    let reopened = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    assert!(reopened.list(None, None, 20).is_empty());
    assert_eq!(reopened.lock().social_imported_at, Some(4242));
}

#[test]
fn delete_then_get_finds_nothing() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    let id = store.upsert(input("Ada"), 1000, 100).unwrap().person.unwrap().id;
    store.delete(&id).unwrap();
    assert!(store.get(&id).is_none());
}

#[test]
fn search_and_list_are_bounded_by_the_requested_limit() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    for i in 0..5 {
        store.upsert(input(&format!("Fulano Repetido {i}")), 1000 + i as i64, 100).unwrap();
    }
    assert_eq!(store.search("fulano repetido", 2).len(), 2);
    assert_eq!(store.list(None, None, 3).len(), 3);
}

#[test]
fn writes_never_land_outside_the_directory_passed_to_open() {
    let dir = TempDir::new();
    let path = dir.0.join("nested").join("catalog.json");
    let store = CatalogStore::open(&path).unwrap();
    store.upsert(input("Ada"), 1000, 100).unwrap();
    assert!(path.exists(), "catalog.json devia existir exatamente no caminho pedido");
}

#[test]
fn block_removes_the_person_and_unblock_reopens_the_door() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    let id = store.upsert(input("Ada"), 1000, 100).unwrap().person.unwrap().id;

    let outcome = store.block("Ada").unwrap();
    assert!(outcome.removed, "a pessoa catalogada devia ter sido removida no bloqueio");
    assert!(store.get(&id).is_none());

    // Bloqueada, um upsert com o mesmo nome não recataloga.
    let refused = store.upsert(input("Ada"), 2000, 100).unwrap();
    assert!(refused.blocked);
    assert!(refused.person.is_none());

    let blocked = store.unblock("Ada").unwrap();
    assert!(blocked.is_empty(), "unblock devia ter tirado a única chave bloqueada");

    let allowed = store.upsert(input("Ada"), 3000, 100).unwrap();
    assert!(allowed.person.is_some(), "depois do unblock o mesmo nome volta a catalogar");
}

#[test]
fn unblock_an_identifier_that_was_never_blocked_is_a_named_error() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    assert!(store.unblock("ninguem").is_err());
}

#[test]
fn list_blocked_reflects_every_blocked_key() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    store.block("Ada").unwrap();
    store.block("Bea").unwrap();
    let mut blocked = store.list_blocked();
    blocked.sort();
    assert_eq!(blocked, vec!["ada".to_string(), "bea".to_string()]);
}

#[test]
fn merge_folds_the_source_person_into_the_target_and_removes_the_source() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    let target = store.upsert(input("Ada Lovelace"), 1000, 100).unwrap().person.unwrap();
    let source = store.upsert(input("Ada L"), 1000, 100).unwrap().person.unwrap();

    let outcome = store.merge(&target.id, &source.id, 2000, 100).unwrap();
    assert!(!outcome.already_merged);
    let merged = outcome.person.unwrap();
    assert_eq!(merged.id, target.id);
    assert!(merged.aliases.contains(&"Ada L".to_string()), "nome da fonte devia virar alias do alvo");

    assert!(store.get(&source.id).is_none(), "registro fonte devia ter sido apagado");
    assert!(store.get(&target.id).is_some());
}

#[test]
fn merge_with_a_missing_source_reports_already_merged_without_touching_the_target() {
    let dir = TempDir::new();
    let store = CatalogStore::open(&dir.0.join("catalog.json")).unwrap();
    let target = store.upsert(input("Ada"), 1000, 100).unwrap().person.unwrap();

    let outcome = store.merge(&target.id, "does-not-exist", 2000, 100).unwrap();
    assert!(outcome.already_merged);
    assert_eq!(outcome.person.unwrap().id, target.id);
}
