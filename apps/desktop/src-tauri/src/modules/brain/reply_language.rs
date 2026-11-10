//! Responder no idioma em que o usuário ESCREVEU, não só no da interface (`app.locale`).
//!
//! O `system.md` fixa `{{language}}` = idioma do app, e isso continua sendo o default: quem tem a
//! interface em português e escreve em português não muda nada. Mas quem escreve uma mensagem
//! inteira em inglês num app em português recebia resposta em português. Aqui o idioma da
//! MENSAGEM é detectado offline (`whatlang`, trigramas, microssegundos — roda no caminho do turno) e,
//! quando ele é confiável e diferente do app, uma nota "Reply language" vai ao fim do prompt.
//!
//! **Só confiável.** Mensagem curta ("ok", "gg", "kkk") não tem trigrama suficiente e nunca troca o
//! idioma: [`MIN_CHARS`] e [`MIN_CONFIDENCE`]. O detector só concorre entre os 10 idiomas da
//! interface ([`SUPPORTED`]): idioma fora deles não tem persona/gírias afinadas e não troca nada.
//!
//! **A voz não muda.** O TTS segue a voz configurada (do idioma do app); a nota troca o texto.
//! Desligável em Configurações → LLM (`llm.matchUserLanguage`).

use std::collections::HashMap;

use whatlang::Lang;

use crate::config::locale::{app_locale, language_name};
use crate::config::store::ConfigStore;

/// Abaixo disso o detector chuta: "bora jogar" sai como qualquer coisa.
const MIN_CHARS: usize = 16;
/// Medido nos testes abaixo: frase real de cada idioma sai entre 0,8 e 1,0; frase curta ou ambígua
/// ("bora jogar uma partida agora" → 0,38; "kkkk…" → 0) fica bem abaixo. O `is_reliable()` do
/// crate recusava espanhol legítimo (0,805), por isso o corte é nosso.
const MIN_CONFIDENCE: f64 = 0.8;

/// Os idiomas da interface e o locale de cada um.
const SUPPORTED: [(Lang, &str); 10] = [
    (Lang::Eng, "en-US"),
    (Lang::Por, "pt-BR"),
    (Lang::Spa, "es-ES"),
    (Lang::Fra, "fr-FR"),
    (Lang::Ita, "it-IT"),
    (Lang::Deu, "de-DE"),
    (Lang::Rus, "ru-RU"),
    (Lang::Jpn, "ja-JP"),
    (Lang::Kor, "ko-KR"),
    (Lang::Cmn, "zh-CN"),
];

/// Locale da interface em que `text` foi escrito, quando a detecção é confiável.
pub(crate) fn detect_locale(text: &str) -> Option<&'static str> {
    let text = text.trim();
    if text.chars().count() < MIN_CHARS {
        return None;
    }
    let detector = whatlang::Detector::with_allowlist(SUPPORTED.iter().map(|(lang, _)| *lang).collect());
    let info = detector.detect(text).filter(|info| info.confidence() >= MIN_CONFIDENCE)?;
    SUPPORTED.iter().find(|(lang, _)| *lang == info.lang()).map(|(_, locale)| *locale)
}

fn enabled(store: &ConfigStore) -> bool {
    store.get("llm.matchUserLanguage").and_then(|v| v.as_bool()).unwrap_or(true)
}

/// A nota do turno (`prompts/brain/reply-language.md`), ou `None` quando nada muda.
pub(crate) fn note(store: &ConfigStore, user_content: &str) -> Option<String> {
    if !enabled(store) {
        return None;
    }
    let detected = detect_locale(user_content)?;
    if detected == app_locale(store) {
        return None;
    }
    let vars = HashMap::from([("language".to_string(), language_name(detected).to_string())]);
    match crate::modules::chat::load_prompt_no_handle("brain", "reply-language", &vars) {
        Ok(text) => Some(text),
        Err(error) => {
            log::warn!("reply-language: prompt indisponível: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(locale: &str, enabled: Option<bool>) -> (ConfigStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mika-reply-language-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap();
        store.set("app.locale", serde_json::json!(locale)).unwrap();
        if let Some(enabled) = enabled {
            store.set("llm.matchUserLanguage", serde_json::json!(enabled)).unwrap();
        }
        (store, dir)
    }

    fn rand_suffix() -> u32 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        N.fetch_add(1, Ordering::Relaxed)
    }

    #[test]
    fn detects_each_interface_language_from_a_real_sentence() {
        let cases = [
            ("Can you help me find a good build for my warrior in this game?", "en-US"),
            ("Você pode me ajudar a achar uma build boa pro meu guerreiro nesse jogo?", "pt-BR"),
            ("¿Puedes ayudarme a encontrar una buena build para mi guerrero en este juego?", "es-ES"),
            ("Kannst du mir helfen, einen guten Build für meinen Krieger in diesem Spiel zu finden?", "de-DE"),
            ("このゲームで戦士の良いビルドを見つけるのを手伝ってくれますか？", "ja-JP"),
            ("Ты можешь помочь мне найти хороший билд для моего воина в этой игре?", "ru-RU"),
        ];
        for (text, locale) in cases {
            assert_eq!(detect_locale(text), Some(locale), "{text}");
        }
    }

    /// Mensagem curta não dá trigrama suficiente: "ok", "kkk", "gg" não podem trocar o idioma.
    #[test]
    fn a_short_message_never_switches_the_language() {
        for text in ["ok", "gg", "kkkkk", "bora jogar", "lol nice", "bora jogar uma partida agora", "kkkkkkkkkkkkkkkkkk"] {
            assert_eq!(detect_locale(text), None, "{text}");
        }
    }

    #[test]
    fn a_message_in_another_language_adds_a_reply_language_note() {
        let (store, dir) = store("pt-BR", None);
        let note = note(&store, "Can you help me find a good build for my warrior in this game?");
        let _ = std::fs::remove_dir_all(dir);
        let note = note.expect("inglês num app em português pede a nota");
        assert!(note.contains("English (US)"), "{note}");
    }

    #[test]
    fn same_language_as_the_interface_adds_nothing() {
        let (store, dir) = store("pt-BR", None);
        let out = note(&store, "Você pode me ajudar a achar uma build boa pro meu guerreiro nesse jogo?");
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(out, None);
    }

    #[test]
    fn the_toggle_off_keeps_the_interface_language() {
        let (store, dir) = store("pt-BR", Some(false));
        let out = note(&store, "Can you help me find a good build for my warrior in this game?");
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(out, None);
    }
}
