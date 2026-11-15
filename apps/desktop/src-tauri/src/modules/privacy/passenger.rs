//! Modo passageiro (`privacy.passengerMode`): companhia sem lembranças. Com ele ligado, a conversa
//! continua na memória do processo (a Mika ainda lembra do que foi dito NESTA sessão), mas nada vai
//! para o disco: nem o histórico do chat (`chat::HistoryStore::push`, `chat::append_message`), nem
//! memória de longo prazo, fatos, perfil, catálogo de pessoas ou diário visual — todos já passam por
//! `memory::memory_recording_allowed`, que passa a dizer "não" também aqui.
//!
//! Não mexe em `memory.enabled`: desligar o modo devolve exatamente as preferências de antes, sem
//! nada para restaurar. E dura só a sessão — [`start_clean`] desliga no boot, então um modo
//! esquecido ligado não apaga a memória de semanas em silêncio.

use std::sync::{Arc, OnceLock};

use crate::config::store::ConfigStore;

static CONFIG: OnceLock<Arc<ConfigStore>> = OnceLock::new();

pub fn is_passenger_mode(store: &ConfigStore) -> bool {
    store.get("privacy.passengerMode").and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Para os escritores sem `ConfigStore` na mão (o histórico do chat). `false` antes do boot.
pub fn passenger_mode() -> bool {
    CONFIG.get().is_some_and(|store| is_passenger_mode(store))
}

/// Chamado uma vez, onde o `ConfigStore` único do processo nasce (`state::config_store`).
pub fn start_clean(store: &Arc<ConfigStore>) {
    if is_passenger_mode(store) {
        if let Err(error) = store.set("privacy.passengerMode", serde_json::json!(false)) {
            log::warn!("modo passageiro não desligou no boot: {error}");
        }
    }
    let _ = CONFIG.set(Arc::clone(store));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passenger_mode_blocks_memory_recording_and_is_off_after_a_restart() {
        let dir = std::env::temp_dir().join(format!("mika-passenger-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap());
        assert!(crate::modules::memory::memory_recording_allowed(&store));

        store.set("privacy.passengerMode", serde_json::json!(true)).unwrap();
        assert!(!crate::modules::memory::memory_recording_allowed(&store), "passageiro não grava memória");
        assert_eq!(store.get("memory.enabled"), Some(serde_json::json!(true)), "a preferência do usuário fica intacta");

        start_clean(&store);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!is_passenger_mode(&store), "o modo dura só a sessão");
    }
}
