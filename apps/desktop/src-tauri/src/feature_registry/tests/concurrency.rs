use std::sync::Arc;
use std::thread;

use super::super::{FeatureRegistry, RegisteredFeature};
use super::{registration, settings};

/// O runtime real é multi-thread (Tokio) e `bridge.rs` invoca módulos concorrentemente. Este
/// teste não prova ausência de bug de concorrência (nenhum teste prova), mas prova que o design
/// escolhido — `RwLock` + clone em vez de referência emprestada — não deadlocka sob registro e
/// leitura simultâneos, e que nenhuma escrita se perde.
#[test]
fn concurrent_register_and_read_do_not_lose_writes_or_panic() {
    let registry = Arc::new(FeatureRegistry::new());
    let mut writers = Vec::new();

    for i in 0..8 {
        let registry = Arc::clone(&registry);
        writers.push(thread::spawn(move || {
            let plugin_id = format!("plugin-{i}");
            registry.register(plugin_id.clone(), registration(true, vec![settings(&plugin_id)]));
        }));
    }

    let mut readers = Vec::new();
    for _ in 0..8 {
        let registry = Arc::clone(&registry);
        readers.push(thread::spawn(move || {
            let _: Vec<RegisteredFeature> = registry.get_all();
            let _ = registry.get_ui_catalog();
        }));
    }

    for writer in writers {
        writer.join().expect("writer thread panicked");
    }
    for reader in readers {
        reader.join().expect("reader thread panicked");
    }

    assert_eq!(registry.get_all().len(), 8);
}
