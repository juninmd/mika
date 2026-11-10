//! O idioma da interface (`app.locale`): o único default, o único leitor e a detecção do primeiro
//! boot.
//!
//! Antes deste arquivo cada módulo que precisava do idioma repetia
//! `store.get("app.locale")...unwrap_or("pt-BR")` — dez cópias do mesmo literal. Adicionar um
//! idioma ou trocar o fallback significava achar todas; esquecer uma fazia a Mika falar português
//! numa interface em coreano, sem erro.

use super::store::ConfigStore;

/// Valor de `app.locale` para quem nunca escolheu idioma e já tinha perfil antes da detecção do
/// SO existir — mantém o comportamento de sempre para usuários existentes. Perfil NOVO não cai
/// aqui: [`seed_first_run`] grava o idioma do SO.
pub const DEFAULT_LOCALE: &str = "pt-BR";

/// Idioma de fallback quando o SO fala algo que a Mika não traduz.
const UNSUPPORTED_OS_LOCALE: &str = "en-US";

/// `app.locale` corrente; vazio ou ausente vira [`DEFAULT_LOCALE`].
pub fn app_locale(store: &ConfigStore) -> String {
    store
        .get("app.locale")
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|locale| !locale.is_empty())
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string())
}

/// Nome do idioma para dentro de um prompt (`{{language}}`). O código (`ko-KR`) o LLM entende pior
/// que o nome, e "Always reply in pt-BR" já deixou modelo pequeno responder em inglês; o nome
/// nativo entre parênteses ancora o script certo (hangul, cirílico).
pub fn language_name(locale: &str) -> &str {
    match locale {
        "en-US" => "English (US)",
        "pt-BR" => "Brazilian Portuguese (Português do Brasil)",
        "es-ES" => "Spanish (Español)",
        "fr-FR" => "French (Français)",
        "it-IT" => "Italian (Italiano)",
        "ja-JP" => "Japanese (日本語)",
        "zh-CN" => "Simplified Chinese (简体中文)",
        "ko-KR" => "Korean (한국어)",
        "ru-RU" => "Russian (Русский)",
        "de-DE" => "German (Deutsch)",
        other => other,
    }
}

/// Código de idioma de duas letras (`ko-KR` → `ko`): o que o Whisper, o Supertonic e as APIs
/// de nuvem pedem.
pub fn language_code(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}

/// Um valor de voz que o usuário nunca mexeu ainda é o default português do schema (todo
/// `config.json` grava o snapshot inteiro, então "nunca mexeu" e "salvo como pt" são o mesmo
/// byte). Esse valor num app em coreano é sempre engano: segue o idioma da interface. Valor
/// diferente do default português é escolha do usuário e fica como está.
pub fn follow_locale(stored: String, pt_default: &str, locale: &str, for_locale: impl FnOnce(&str) -> String) -> String {
    if stored == pt_default && locale != DEFAULT_LOCALE { for_locale(locale) } else { stored }
}

/// [`language_name`] do `app.locale` corrente.
pub fn app_language(store: &ConfigStore) -> String {
    language_name(&app_locale(store)).to_string()
}

/// Tag BCP 47 ou POSIX (`ko-KR`, `pt_BR.UTF-8`, `zh-Hans-CN`, `de`) → locale suportado. Só a língua
/// decide: `es-MX` fala espanhol, e a Mika só tem um `es-*`. Língua sem tradução → `en-US`.
pub fn locale_for_os_tag(tag: &str) -> &'static str {
    let language = tag.split(['-', '_', '.', '@']).next().unwrap_or("").to_ascii_lowercase();
    match language.as_str() {
        "ko" => "ko-KR",
        "ru" => "ru-RU",
        "de" => "de-DE",
        "zh" => "zh-CN",
        "ja" => "ja-JP",
        "es" => "es-ES",
        "fr" => "fr-FR",
        "it" => "it-IT",
        "pt" => "pt-BR",
        _ => UNSUPPORTED_OS_LOCALE,
    }
}

/// Grava o idioma do SO como `app.locale` de um perfil novo. Quem chama decide o que é "novo"
/// (`state::config_store`: não havia `config.json`); gravar já no primeiro boot, e não só derivar
/// na leitura, mantém o idioma estável se o SO mudar depois e vale também para o renderer, que lê
/// `app.locale` pelo mesmo `ui-prefs` de sempre.
pub fn seed_first_run(store: &ConfigStore, os_tag: Option<&str>) -> Result<(), String> {
    let locale = os_tag.map_or(UNSUPPORTED_OS_LOCALE, locale_for_os_tag);
    store.set("app.locale", serde_json::json!(locale))
}

/// Idioma preferido do SO (`None` quando o SO não informa — contêiner com `LANG=C`).
pub fn os_locale_tag() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schemas;
    use serde_json::json;

    #[test]
    fn os_tags_map_to_the_supported_locale_of_the_same_language() {
        for (tag, expected) in [
            ("ko-KR", "ko-KR"),
            ("ko_KR.UTF-8", "ko-KR"),
            ("ru", "ru-RU"),
            ("ru_UA.UTF-8", "ru-RU"),
            ("de-AT", "de-DE"),
            ("zh-Hans-CN", "zh-CN"),
            ("zh_TW", "zh-CN"),
            ("ja_JP.eucJP", "ja-JP"),
            ("es-MX", "es-ES"),
            ("fr-CA", "fr-FR"),
            ("it", "it-IT"),
            ("pt-PT", "pt-BR"),
            ("PT_br@euro", "pt-BR"),
            ("en-GB", "en-US"),
            ("sv-SE", "en-US"),
            ("C", "en-US"),
            ("", "en-US"),
        ] {
            assert_eq!(locale_for_os_tag(tag), expected, "{tag}");
        }
    }

    #[test]
    fn prompts_get_the_language_name_not_the_code() {
        assert_eq!(language_name("ko-KR"), "Korean (한국어)");
        assert_eq!(language_name("de-DE"), "German (Deutsch)");
        assert!(language_name("pt-BR").starts_with("Brazilian Portuguese"));
        assert_eq!(language_name("xx-XX"), "xx-XX");
        for locale in ["en-US", "pt-BR", "es-ES", "fr-FR", "it-IT", "ja-JP", "zh-CN", "ko-KR", "ru-RU", "de-DE"] {
            assert_ne!(language_name(locale), locale, "{locale} sem nome de idioma");
        }
    }

    #[test]
    fn untouched_pt_defaults_follow_the_ui_language_but_choices_stay() {
        let ko = |l: &str| language_code(l).to_string();
        assert_eq!(follow_locale("pt".into(), "pt", "ko-KR", ko), "ko");
        assert_eq!(follow_locale("pt".into(), "pt", "pt-BR", ko), "pt");
        assert_eq!(follow_locale("en".into(), "pt", "ko-KR", ko), "en");
    }

    #[test]
    fn every_mapped_locale_is_accepted_by_the_config_schema() {
        let registry = schemas::registry();
        for tag in ["ko", "ru", "de", "zh", "ja", "es", "fr", "it", "pt", "en"] {
            let locale = locale_for_os_tag(tag);
            assert!(registry.validate("app.locale", &json!(locale)).is_ok(), "{locale}");
        }
    }

    fn open(dir: &std::path::Path) -> ConfigStore {
        ConfigStore::open(dir, schemas::registry()).unwrap()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mika-locale-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Perfil novo com SO em coreano abre em coreano, e continua assim no boot seguinte; perfil
    /// que nunca passou pela detecção segue em `DEFAULT_LOCALE`.
    #[test]
    fn first_run_persists_the_os_language_and_untouched_profiles_keep_the_default() {
        let dir = temp_dir("seed");
        let store = open(&dir);
        assert_eq!(app_locale(&store), DEFAULT_LOCALE);

        seed_first_run(&store, Some("ko_KR.UTF-8")).unwrap();
        drop(store);

        assert_eq!(app_locale(&open(&dir)), "ko-KR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_run_without_an_os_language_is_english() {
        let dir = temp_dir("none");
        let store = open(&dir);
        seed_first_run(&store, None).unwrap();
        assert_eq!(app_locale(&store), "en-US");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
