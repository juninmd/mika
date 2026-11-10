//! Catálogo de vozes Piper que o app oferece: SÓ o que está aqui baixa. Cada voz carrega o SHA-256 do
//! `.onnx` e do `.onnx.json` na revisão FIXA do repositório (commit, não a tag móvel `v1.0.0`):
//! quem controlasse a ref trocaria os bytes que o modelo ONNX da próxima instalação executa.
//!
//! **Licença é critério de entrada, não detalhe.** O repositório `rhasspy/piper-voices` é MIT, mas a
//! licença do DATASET varia por voz (Ruslan e Pavoque são CC-BY-NC; Huayan, Irina e várias outras
//! não declaram nada). Cada voz abaixo teve o `MODEL_CARD` lido e a licença do dataset conferida na
//! origem em 2026-10-06 — o registro por voz está em `LICENSE-ASSETS.md` e no inventário de
//! `THIRD-PARTY-NOTICES.md`. Voz nova só entra com SHA fixo E licença comercial verificada.
//!
//! **Idiomas sem voz aqui, de propósito:** `ja` e `ko` (o Piper não tem voz nesses idiomas), `zh`
//! (`zh_CN-huayan`: licença do dataset "Unknown") e `en` (o Supertonic fala inglês, então não há
//! fallback a alimentar; `en_US-amy-medium` saiu porque o `MODEL_CARD` só diz "See URL" — licença do
//! dataset NÃO verificada — e `en_US-lessac-high` por ter sido treinada direto no dataset Lessac,
//! só pesquisa). Para esses idiomas não existe fallback local — a UI avisa e sugere um provedor de
//! nuvem.

/// Revisão do `rhasspy/piper-voices` de onde todo SHA abaixo foi medido (== tag `v1.0.0`).
pub(super) const REVISION: &str = "375a0fe641dea077c2a47b4e9a056d6da521eed3";

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
        onnx_sha256: "858555e3a064209c57088fe6bd70c4c3dc54d03eaa00c45d5ecaf43a33f95aa7",
        json_sha256: "7e694de195ae3fc36dd732c445eb04fb49b649854893cb5506b978f0d50a1d6f",
    },
    CatalogVoice {
        key: "pt_BR-edresson-low",
        language: "pt",
        onnx_sha256: "de4cecee38b30bb1a6378a337af605d59f0c377df702c6a6752870db8991cd84",
        json_sha256: "f138992d2e777d1e3aa0bbb14c2d324307b0f342c1bcf20978765b3bea506c56",
    },
    // Dataset Thorsten-Voice, CC0.
    CatalogVoice {
        key: "de_DE-thorsten-medium",
        language: "de",
        onnx_sha256: "7e64762d8e5118bb578f2eea6207e1a35a8e0c30595010b666f983fc87bb7819",
        json_sha256: "974adee790533adb273a1ac88f49027d2a1b8f0f2cf4905954a4791e79264e85",
    },
    // Dataset `paolapersico1/Voice-Dataset-Italian`, CC0-1.0 (cartão do dataset no Hugging Face).
    CatalogVoice {
        key: "it_IT-paola-medium",
        language: "it",
        onnx_sha256: "6fc918b5a0ea6137382833dddfa567bffbe6a5060c02043c87192ee59c04210c",
        json_sha256: "aea19c0a7fce29fbc359b93f10e7902854401e4c95ae2ea328ae516b15d296cf",
    },
    // Dataset NabuCasa/voice-datasets, CC0.
    CatalogVoice {
        key: "ru_RU-denis-medium",
        language: "ru",
        onnx_sha256: "15fab56e11a097858ee115545d0f697fc2a316c41a291a5362349fb870411b0a",
        json_sha256: "831c860dac0b5073eaa81610a0a638ec23d90a6cf8e5f871b4485c2cec3767c8",
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
