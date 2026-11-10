//! `voice.asrLanguage` → the encoder's `lang_id` prompt index (the export's prompt dictionary).

/// Automatic language detection.
pub const AUTO: i64 = 101;

// Bare "pt" goes to pt-BR, not pt-PT as in the model's own table: this app's users are Brazilian.
const PROMPTS: &[(&str, i64)] = &[
    ("pt", 12), ("pt-br", 12), ("pt-pt", 13), ("en", 0), ("en-us", 0), ("en-gb", 1),
    ("es", 3), ("es-us", 3), ("es-es", 2), ("fr", 8), ("fr-fr", 8), ("fr-ca", 100),
    ("it", 15), ("it-it", 15), ("de", 9), ("de-de", 9), ("nl", 16), ("nl-nl", 16),
    ("tr", 18), ("tr-tr", 18), ("ru", 11), ("ru-ru", 11), ("ar", 7), ("ar-ar", 7),
    ("hi", 6), ("hi-in", 6), ("ja", 10), ("ja-jp", 10), ("ko", 14), ("ko-kr", 14),
    ("vi", 33), ("vi-vn", 33), ("uk", 19), ("uk-ua", 19), ("pl", 17), ("pl-pl", 17),
    ("sv", 24), ("sv-se", 24), ("cs", 22), ("cs-cz", 22), ("nb", 103), ("nb-no", 103),
    ("da", 25), ("da-dk", 25), ("bg", 30), ("bg-bg", 30), ("fi", 26), ("fi-fi", 26),
    ("hr", 29), ("hr-hr", 29), ("sk", 28), ("sk-sk", 28), ("zh", 4), ("zh-cn", 4),
    ("hu", 23), ("hu-hu", 23), ("ro", 20), ("ro-ro", 20), ("et", 60), ("et-ee", 60),
];

/// Unknown or empty languages fall back to detection instead of forcing a wrong prompt.
pub fn prompt_id(language: Option<&str>) -> i64 {
    let Some(code) = language.map(|l| l.trim().replace('_', "-").to_lowercase()) else {
        return AUTO;
    };
    PROMPTS.iter().find(|(name, _)| *name == code).map_or(AUTO, |(_, id)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portuguese_defaults_to_brazil() {
        assert_eq!(prompt_id(Some("pt")), 12);
        assert_eq!(prompt_id(Some("pt_BR")), 12);
        assert_eq!(prompt_id(Some("pt-PT")), 13);
    }

    #[test]
    fn unknown_or_missing_language_detects_automatically() {
        assert_eq!(prompt_id(None), AUTO);
        assert_eq!(prompt_id(Some("")), AUTO);
        assert_eq!(prompt_id(Some("klingon")), AUTO);
    }
}
