//! Vozes de nuvem que as opções de `azureTtsVoice`/`googleTtsVoice` oferecem — uma tabela por
//! provedor, as MESMAS vozes de `renderer/settings/voice/tts-provider-constants.ts` (um teste em
//! `voice.rs` lê o TS e confere). Os nomes saem da documentação oficial (Azure: "Language and voice
//! support for the Speech service"; Google: "Supported voices and languages"), conferida em
//! 2026-10-06: o nome é o identificador que a API recebe, então um nome inventado é erro 400.
//!
//! Dentro de cada idioma a PRIMEIRA voz é a padrão: `remote/azure.rs` e `remote/google.rs` a usam
//! quando a voz salva ainda é o default português e a interface está em outro idioma — a tabela é
//! a única fonte do default e da lista, não duas listas para divergir.

use crate::config::schema::UiOption;

/// Primeira voz da tabela cujo locale é `voice_locale` (`ja-JP`; no Google, `cmn-CN` e não `zh-CN`).
pub(crate) fn default_voice(voices: &'static [UiOption], voice_locale: &str) -> Option<&'static str> {
    let prefix = format!("{voice_locale}-");
    voices.iter().find(|voice| voice.value.starts_with(&prefix)).map(|voice| voice.value)
}

pub(crate) const AZURE_VOICES: &[UiOption] = &[
    UiOption { label: "Jenny (en-US)", value: "en-US-JennyNeural" },
    UiOption { label: "Aria (en-US)", value: "en-US-AriaNeural" },
    UiOption { label: "Guy (en-US)", value: "en-US-GuyNeural" },
    UiOption { label: "Davis (en-US)", value: "en-US-DavisNeural" },
    UiOption { label: "Francisca (pt-BR)", value: "pt-BR-FranciscaNeural" },
    UiOption { label: "Thalita (pt-BR)", value: "pt-BR-ThalitaNeural" },
    UiOption { label: "Antonio (pt-BR)", value: "pt-BR-AntonioNeural" },
    UiOption { label: "Donato (pt-BR)", value: "pt-BR-DonatoNeural" },
    UiOption { label: "Elvira (es-ES)", value: "es-ES-ElviraNeural" },
    UiOption { label: "Abril (es-ES)", value: "es-ES-AbrilNeural" },
    UiOption { label: "Alvaro (es-ES)", value: "es-ES-AlvaroNeural" },
    UiOption { label: "Arnau (es-ES)", value: "es-ES-ArnauNeural" },
    UiOption { label: "Denise (fr-FR)", value: "fr-FR-DeniseNeural" },
    UiOption { label: "Brigitte (fr-FR)", value: "fr-FR-BrigitteNeural" },
    UiOption { label: "Henri (fr-FR)", value: "fr-FR-HenriNeural" },
    UiOption { label: "Alain (fr-FR)", value: "fr-FR-AlainNeural" },
    UiOption { label: "Elsa (it-IT)", value: "it-IT-ElsaNeural" },
    UiOption { label: "Isabella (it-IT)", value: "it-IT-IsabellaNeural" },
    UiOption { label: "Diego (it-IT)", value: "it-IT-DiegoNeural" },
    UiOption { label: "Benigno (it-IT)", value: "it-IT-BenignoNeural" },
    UiOption { label: "Nanami (ja-JP)", value: "ja-JP-NanamiNeural" },
    UiOption { label: "Aoi (ja-JP)", value: "ja-JP-AoiNeural" },
    UiOption { label: "Keita (ja-JP)", value: "ja-JP-KeitaNeural" },
    UiOption { label: "Daichi (ja-JP)", value: "ja-JP-DaichiNeural" },
    UiOption { label: "Xiaoxiao (zh-CN)", value: "zh-CN-XiaoxiaoNeural" },
    UiOption { label: "Xiaoyi (zh-CN)", value: "zh-CN-XiaoyiNeural" },
    UiOption { label: "Yunxi (zh-CN)", value: "zh-CN-YunxiNeural" },
    UiOption { label: "Yunjian (zh-CN)", value: "zh-CN-YunjianNeural" },
    UiOption { label: "SunHi (ko-KR)", value: "ko-KR-SunHiNeural" },
    UiOption { label: "JiMin (ko-KR)", value: "ko-KR-JiMinNeural" },
    UiOption { label: "InJoon (ko-KR)", value: "ko-KR-InJoonNeural" },
    UiOption { label: "BongJin (ko-KR)", value: "ko-KR-BongJinNeural" },
    UiOption { label: "Svetlana (ru-RU)", value: "ru-RU-SvetlanaNeural" },
    UiOption { label: "Dariya (ru-RU)", value: "ru-RU-DariyaNeural" },
    UiOption { label: "Dmitry (ru-RU)", value: "ru-RU-DmitryNeural" },
    UiOption { label: "Katja (de-DE)", value: "de-DE-KatjaNeural" },
    UiOption { label: "Amala (de-DE)", value: "de-DE-AmalaNeural" },
    UiOption { label: "Conrad (de-DE)", value: "de-DE-ConradNeural" },
    UiOption { label: "Bernd (de-DE)", value: "de-DE-BerndNeural" },
];

/// Só WaveNet (os Standard são de qualidade inferior); o nome inclui o prefixo do idioma — a API recusa `Wavenet-A` sem ele.
pub(crate) const GOOGLE_VOICES: &[UiOption] = &[
    UiOption { label: "Wavenet-C (en-US)", value: "en-US-Wavenet-C" },
    UiOption { label: "Wavenet-F (en-US)", value: "en-US-Wavenet-F" },
    UiOption { label: "Wavenet-D (en-US)", value: "en-US-Wavenet-D" },
    UiOption { label: "Wavenet-B (en-US)", value: "en-US-Wavenet-B" },
    UiOption { label: "Wavenet-A (pt-BR)", value: "pt-BR-Wavenet-A" },
    UiOption { label: "Wavenet-C (pt-BR)", value: "pt-BR-Wavenet-C" },
    UiOption { label: "Wavenet-B (pt-BR)", value: "pt-BR-Wavenet-B" },
    UiOption { label: "Wavenet-E (pt-BR)", value: "pt-BR-Wavenet-E" },
    UiOption { label: "Wavenet-F (es-ES)", value: "es-ES-Wavenet-F" },
    UiOption { label: "Wavenet-H (es-ES)", value: "es-ES-Wavenet-H" },
    UiOption { label: "Wavenet-E (es-ES)", value: "es-ES-Wavenet-E" },
    UiOption { label: "Wavenet-G (es-ES)", value: "es-ES-Wavenet-G" },
    UiOption { label: "Wavenet-F (fr-FR)", value: "fr-FR-Wavenet-F" },
    UiOption { label: "Wavenet-G (fr-FR)", value: "fr-FR-Wavenet-G" },
    UiOption { label: "Wavenet-E (it-IT)", value: "it-IT-Wavenet-E" },
    UiOption { label: "Wavenet-F (it-IT)", value: "it-IT-Wavenet-F" },
    UiOption { label: "Wavenet-A (ja-JP)", value: "ja-JP-Wavenet-A" },
    UiOption { label: "Wavenet-B (ja-JP)", value: "ja-JP-Wavenet-B" },
    UiOption { label: "Wavenet-C (ja-JP)", value: "ja-JP-Wavenet-C" },
    UiOption { label: "Wavenet-D (ja-JP)", value: "ja-JP-Wavenet-D" },
    UiOption { label: "Wavenet-A (cmn-CN)", value: "cmn-CN-Wavenet-A" },
    UiOption { label: "Wavenet-D (cmn-CN)", value: "cmn-CN-Wavenet-D" },
    UiOption { label: "Wavenet-B (cmn-CN)", value: "cmn-CN-Wavenet-B" },
    UiOption { label: "Wavenet-C (cmn-CN)", value: "cmn-CN-Wavenet-C" },
    UiOption { label: "Wavenet-A (ko-KR)", value: "ko-KR-Wavenet-A" },
    UiOption { label: "Wavenet-B (ko-KR)", value: "ko-KR-Wavenet-B" },
    UiOption { label: "Wavenet-C (ko-KR)", value: "ko-KR-Wavenet-C" },
    UiOption { label: "Wavenet-D (ko-KR)", value: "ko-KR-Wavenet-D" },
    UiOption { label: "Wavenet-A (ru-RU)", value: "ru-RU-Wavenet-A" },
    UiOption { label: "Wavenet-C (ru-RU)", value: "ru-RU-Wavenet-C" },
    UiOption { label: "Wavenet-B (ru-RU)", value: "ru-RU-Wavenet-B" },
    UiOption { label: "Wavenet-D (ru-RU)", value: "ru-RU-Wavenet-D" },
    UiOption { label: "Wavenet-G (de-DE)", value: "de-DE-Wavenet-G" },
    UiOption { label: "Wavenet-H (de-DE)", value: "de-DE-Wavenet-H" },
];
