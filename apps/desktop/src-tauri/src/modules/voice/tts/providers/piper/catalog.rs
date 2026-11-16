//! Catálogo de vozes Piper que o app oferece: SÓ o que está aqui baixa. Cada voz carrega o SHA-256 do
//! `.onnx` e do `.onnx.json` na revisão FIXA do repositório (commit, não a tag móvel `v1.0.0`):
//! quem controlasse a ref trocaria os bytes que o modelo ONNX da próxima instalação executa.
//!
//! **Licença é critério de entrada, não detalhe.** O repositório `rhasspy/piper-voices` é MIT, mas a
//! licença do DATASET varia por voz (Ruslan e Pavoque são CC-BY-NC; Huayan, Irina e várias outras
//! não declaram nada). Cada voz abaixo teve o `MODEL_CARD` lido e a licença do dataset conferida na
//! origem em 2026-10-06 (e de novo em 2026-11-15, na revisão retreinada) — o registro por voz está em `LICENSE-ASSETS.md` e no inventário de
//! `THIRD-PARTY-NOTICES.md`. Voz nova só entra com SHA fixo E licença comercial verificada.
//!
//! **Idiomas sem voz aqui, de propósito:** `ja` e `ko` (o Piper não tem voz nesses idiomas), `zh`
//! (`zh_CN-huayan`: licença do dataset "Unknown") e `en` (o Supertonic fala inglês, então não há
//! fallback a alimentar; `en_US-amy-medium` saiu porque o `MODEL_CARD` só diz "See URL" — licença do
//! dataset NÃO verificada — e `en_US-lessac-high` por ter sido treinada direto no dataset Lessac,
//! só pesquisa). Para esses idiomas não existe fallback local — a UI avisa e sugere um provedor de
//! nuvem.

/// Revisão do `rhasspy/piper-voices` de onde todo SHA abaixo foi medido (2026-11-13).
///
/// **Vozes retreinadas.** faber, thorsten, paola e denis foram retreinadas em nov/2026 a partir das
/// bases `hifi_tts_92` (Hi-Fi TTS, CC BY 4.0) e `rdh` (CC0), com MRD e SDP — não mais do checkpoint
/// `en_US-lessac` (dataset só para pesquisa), o que encerra a pendência de linhagem registrada em
/// `LICENSE-ASSETS.md`. O `MODEL_CARD` delas exige **Piper 1.5+** (`vowel_clusters` no `.onnx.json`;
/// um Piper antigo ignora o campo e lê fonemas que a voz não conhece) — ver `piper.rs::check_version`.
/// edresson não mudou (mesmo SHA da `v1.0.0`).
pub(super) const REVISION: &str = "6b9cfaacb4fb2e02a9621227e3c1a48bec9710c3";
/// Versão mínima do binário para as vozes retreinadas.
pub(super) const MIN_PIPER_VERSION: (u32, u32) = (1, 5);

pub(crate) struct CatalogVoice {
    pub key: &'static str,
    /// Código de idioma de duas letras (o mesmo de `config::locale::language_code`).
    pub language: &'static str,
    pub onnx_sha256: &'static str,
    pub json_sha256: &'static str,
}

pub(crate) const VOICES: [CatalogVoice; 5] = [
    CatalogVoice {
        key: "pt_BR-faber-medium",
        language: "pt",
        onnx_sha256: "75e913eb463db0b6ac7ddf68d3e4c9ac19ace36dc3b710a81039b448352b8ffc",
        json_sha256: "fa8a8fecb01ae198bc416381e368d3206f85bad1449a0f11e1f6a9edf85a32f1",
    },
    CatalogVoice {
        key: "pt_BR-edresson-low",
        language: "pt",
        onnx_sha256: "de4cecee38b30bb1a6378a337af605d59f0c377df702c6a6752870db8991cd84",
        json_sha256: "f138992d2e777d1e3aa0bbb14c2d324307b0f342c1bcf20978765b3bea506c56",
    },
    // Dataset Thorsten-Voice, CC0; base `rdh` (CC0).
    CatalogVoice {
        key: "de_DE-thorsten-medium",
        language: "de",
        onnx_sha256: "fd270eb3d1ebf91d763f8a0f949b1b1bebdfefaea18c3aaf8577dd34536e064f",
        json_sha256: "cf245c91668e74cb6716f38647ae3d6d9bac42965872596f65f9ab2b7d7221ed",
    },
    // Dataset OHF-Voice/voice-datasets (CC0); base `hifi_tts_92` (CC BY 4.0).
    CatalogVoice {
        key: "it_IT-paola-medium",
        language: "it",
        onnx_sha256: "f9b8658aa8c01f8280aa88488f6e7643a639b6225a81514c40440a01f6364a20",
        json_sha256: "41c062de55b3410bcc01e456ba6d037d56669ecbb68849ab8b330e30fc475c6e",
    },
    // Dataset OHF-Voice/voice-datasets (CC0); base `rdh` (CC0).
    CatalogVoice {
        key: "ru_RU-denis-medium",
        language: "ru",
        onnx_sha256: "879bc365cb14d1380e3f5ed6044162ccf54c99f6161871569b7f830aec3ce841",
        json_sha256: "e4fb4ab60262d1ab802c8c39301976c4135e545bc834a33c6eedc1f764df3bd6",
    },
];

pub(crate) fn voice(key: &str) -> Option<&'static CatalogVoice> {
    VOICES.iter().find(|voice| voice.key == key)
}

/// Voz do catálogo para um idioma — o que o fallback do Supertonic usa quando o modelo não fala o
/// idioma da interface. `None` = nenhuma voz comercialmente segura nesse idioma.
pub(crate) fn voice_for_language(language: &str) -> Option<&'static CatalogVoice> {
    VOICES.iter().find(|voice| voice.language == language)
}
