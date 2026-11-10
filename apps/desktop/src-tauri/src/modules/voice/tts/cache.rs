//! Porte de `apps/desktop/src/main/services/voice/tts/tts-cache.ts`. Só a chave do Supertonic é
//! montada aqui — as chaves dos outros provedores (ElevenLabs/OpenAI/Azure/Google/Piper/Kokoro)
//! ficam de fora junto com o resto do provedor (ver `dispatch.rs`); adicionar a chave sem o gerador
//! seria cache para um resultado que nunca existe.

use std::collections::HashMap;

pub const TTS_CACHE_MAX_ENTRIES: usize = 64;

#[derive(Clone)]
pub struct CachedAudio {
    pub audio: Vec<u8>,
    pub mime_type: &'static str,
}

/// Ordem de inserção preservada em `order` para o descarte FIFO do mais antigo — mesmo efeito de
/// `cache.keys().next()` sobre um `Map` do JS (que preserva ordem de inserção).
pub struct TtsCache {
    entries: HashMap<String, CachedAudio>,
    order: Vec<String>,
}

impl TtsCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), order: Vec::new() }
    }

    pub fn get(&self, key: &str) -> Option<CachedAudio> {
        self.entries.get(key).cloned()
    }

    /// Espelha `finishGenerate`: resultado de fallback nunca entra no cache do provedor pedido.
    pub fn insert(&mut self, key: String, audio: CachedAudio) {
        if self.entries.insert(key.clone(), audio).is_none() {
            self.order.push(key);
        }
        while self.order.len() > TTS_CACHE_MAX_ENTRIES {
            let oldest = self.order.remove(0);
            self.entries.remove(&oldest);
        }
    }

    pub fn clear(&mut self) -> usize {
        let cleared = self.entries.len();
        self.entries.clear();
        self.order.clear();
        cleared
    }
}

impl Default for TtsCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Chave do Supertonic — espelha o `case TTS_PROVIDERS.SUPERTONIC` de `buildCacheKey`.
pub fn supertonic_cache_key(text: &str, voice: &str, language: &str, steps: u32, speed: f32) -> String {
    format!("supertonic|{text}|{voice}|{language}|{steps}|{speed}")
}

#[cfg(test)]
impl TtsCache {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(tag: u8) -> CachedAudio {
        CachedAudio { audio: vec![tag], mime_type: "audio/wav" }
    }

    #[test]
    fn get_misses_on_an_unknown_key() {
        assert!(TtsCache::new().get("nope").is_none());
    }

    #[test]
    fn insert_then_get_round_trips_the_same_bytes() {
        let mut cache = TtsCache::new();
        cache.insert("k".into(), audio(7));
        assert_eq!(cache.get("k").unwrap().audio, vec![7]);
    }

    #[test]
    fn insert_evicts_the_oldest_entry_once_the_cap_is_exceeded() {
        let mut cache = TtsCache::new();
        for i in 0..TTS_CACHE_MAX_ENTRIES {
            cache.insert(format!("k{i}"), audio(0));
        }
        assert_eq!(cache.len(), TTS_CACHE_MAX_ENTRIES);

        cache.insert("k-new".into(), audio(1));
        assert_eq!(cache.len(), TTS_CACHE_MAX_ENTRIES, "cap não pode crescer sem limite");
        assert!(cache.get("k0").is_none(), "a entrada mais antiga tinha de sair primeiro");
        assert!(cache.get("k-new").is_some());
    }

    #[test]
    fn clear_empties_the_cache_and_reports_how_many_entries_it_removed() {
        let mut cache = TtsCache::new();
        cache.insert("a".into(), audio(1));
        cache.insert("b".into(), audio(2));
        assert_eq!(cache.clear(), 2);
        assert!(cache.is_empty());
    }

    #[test]
    fn supertonic_cache_key_differs_by_every_parameter_that_changes_the_audio() {
        let base = supertonic_cache_key("oi", "F3", "pt", 20, 1.0);
        assert_ne!(base, supertonic_cache_key("oi", "F1", "pt", 20, 1.0), "voz diferente");
        assert_ne!(base, supertonic_cache_key("oi", "F3", "en", 20, 1.0), "idioma diferente");
        assert_ne!(base, supertonic_cache_key("oi", "F3", "pt", 10, 1.0), "passos diferentes");
        assert_ne!(base, supertonic_cache_key("oi", "F3", "pt", 20, 1.5), "velocidade diferente");
    }
}
