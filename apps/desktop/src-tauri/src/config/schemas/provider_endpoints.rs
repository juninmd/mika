//! Endpoints dos provedores remotos — fonte ÚNICA. Cada origem aparecia em dois a quatro arquivos
//! (catálogo de modelos, perfil do modelo, chat, embeddings, TTS, botão "Testar"), e uma mudança
//! de versão de API (`v1beta` → `v1`) atualizava um caminho e deixava o outro batendo na antiga.
//! Também é a lista auditável de hosts para onde a Mika sai — todos passam por
//! `privacy::assert_network_allowed` em quem chama.

pub(crate) const GOOGLE_AI_ORIGIN: &str = "https://generativelanguage.googleapis.com";
pub(crate) const GOOGLE_AI_V1BETA: &str = "https://generativelanguage.googleapis.com/v1beta";
pub(crate) const GOOGLE_AI_MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models?pageSize=200";
pub(crate) const GOOGLE_AI_MODEL_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models/";

pub(crate) const OPENROUTER_ORIGIN: &str = "https://openrouter.ai";
pub(crate) const OPENROUTER_API_V1: &str = "https://openrouter.ai/api/v1";
pub(crate) const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
/// Só modelos com endpoint de retenção zero (o par do `provider.zdr` em `brain::provider_params`).
pub(crate) const OPENROUTER_ZDR_MODELS_URL: &str = "https://openrouter.ai/api/v1/models?zdr=true";
pub(crate) const OPENROUTER_KEY_URL: &str = "https://openrouter.ai/api/v1/key";

pub(crate) const ELEVENLABS_ORIGIN: &str = "https://api.elevenlabs.io";
pub(crate) const ELEVENLABS_VOICES_URL: &str = "https://api.elevenlabs.io/v1/voices";

pub(crate) const GOOGLE_TTS_ORIGIN: &str = "https://texttospeech.googleapis.com";
pub(crate) const GOOGLE_TTS_VOICES_URL: &str = "https://texttospeech.googleapis.com/v1/voices";

#[cfg(test)]
mod tests {
    use super::*;

    /// Literal completo (const não concatena), então o teste é quem garante que nenhum caminho
    /// derivado divergiu da origem do seu provedor.
    #[test]
    fn every_derived_url_starts_at_its_provider_origin() {
        let families: [(&str, &[&str]); 4] = [
            (GOOGLE_AI_ORIGIN, &[GOOGLE_AI_V1BETA, GOOGLE_AI_MODELS_URL, GOOGLE_AI_MODEL_BASE]),
            (OPENROUTER_ORIGIN, &[OPENROUTER_API_V1, OPENROUTER_MODELS_URL, OPENROUTER_ZDR_MODELS_URL, OPENROUTER_KEY_URL]),
            (ELEVENLABS_ORIGIN, &[ELEVENLABS_VOICES_URL]),
            (GOOGLE_TTS_ORIGIN, &[GOOGLE_TTS_VOICES_URL]),
        ];
        for (origin, urls) in families {
            for url in urls {
                assert!(url.starts_with(&format!("{origin}/")), "{url} fora de {origin}");
            }
        }
        assert!(GOOGLE_AI_MODELS_URL.starts_with(GOOGLE_AI_V1BETA) && GOOGLE_AI_MODEL_BASE.starts_with(GOOGLE_AI_V1BETA));
        assert!(OPENROUTER_MODELS_URL.starts_with(OPENROUTER_API_V1) && OPENROUTER_KEY_URL.starts_with(OPENROUTER_API_V1));
    }
}
