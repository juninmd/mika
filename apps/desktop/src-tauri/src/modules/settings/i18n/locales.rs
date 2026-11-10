//! As 10 traduções (`packages/sdk/src/i18n/locales/*.ts`) embutidas no binário via `include_str!`
//! e o fallback de `t()` que `packages/sdk/src/i18n/index.ts::t` já define: dicionário do locale
//! pedido → `en-US` → a própria chave. Sem isso, uma declaração de settings com chave i18n não
//! resolvida mostraria `settings.vision.field.enabled` cru na tela em vez do rótulo — e uma chave
//! ausente em TODOS os dicionários (ou um valor que já É texto literal, não chave i18n — caso de
//! `LLM_PROVIDER_OPTIONS`, ver `feature_fields.rs` em `modules/llm/`) tem de devolver o texto
//! como está, nunca string vazia: é exatamente o que este fallback de 3 níveis garante.
//!
//! `include_str!` (não um asset gerado) porque os arquivos já são a fonte da verdade — gerar um
//! JSON intermediário criaria uma segunda cópia para divergir. Ver `parser.rs` para por que dá
//! para ler fonte TS sem um parser TS completo.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use super::parser::parse_dict;

pub const FALLBACK_LOCALE: &str = "en-US";

const EN_US: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/en-US.ts");
const PT_BR: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/pt-BR.ts");
const ES_ES: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/es-ES.ts");
const FR_FR: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/fr-FR.ts");
const IT_IT: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/it-IT.ts");
const JA_JP: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/ja-JP.ts");
const ZH_CN: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/zh-CN.ts");
const KO_KR: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/ko-KR.ts");
const RU_RU: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/ru-RU.ts");
const DE_DE: &str = include_str!("../../../../../../../packages/sdk/src/i18n/locales/de-DE.ts");

const LOCALES: [(&str, &str); 10] = [
    ("en-US", EN_US),
    ("pt-BR", PT_BR),
    ("es-ES", ES_ES),
    ("fr-FR", FR_FR),
    ("it-IT", IT_IT),
    ("ja-JP", JA_JP),
    ("zh-CN", ZH_CN),
    ("ko-KR", KO_KR),
    ("ru-RU", RU_RU),
    ("de-DE", DE_DE),
];

static DICTS: OnceLock<HashMap<&'static str, BTreeMap<String, String>>> = OnceLock::new();

fn dicts() -> &'static HashMap<&'static str, BTreeMap<String, String>> {
    DICTS.get_or_init(|| {
        LOCALES
            .iter()
            .map(|(locale, source)| (*locale, parse_dict(source)))
            .collect()
    })
}

/// Espelha `t(key, locale)` de `packages/sdk/src/i18n/index.ts`: dicionário do locale pedido,
/// senão `en-US`, senão a própria chave — nunca string vazia. `locale` desconhecido (fora dos 10
/// suportados) cai direto no fallback, igual ao `isSupportedLocale` do lado TS.
pub fn t(key: &str, locale: &str) -> String {
    let dicts = dicts();

    if let Some(value) = dicts.get(locale).and_then(|dict| dict.get(key)) {
        return value.clone();
    }
    if let Some(value) = dicts.get(FALLBACK_LOCALE).and_then(|dict| dict.get(key)) {
        return value.clone();
    }
    key.to_string()
}

/// [`t`] com `{{nome}}` substituído — para mensagem de erro que chega à UI com dado dentro
/// (provedor, região, status). O texto do dicionário é a única fonte: nada de `format!` em PT.
pub fn tf(key: &str, locale: &str, vars: &[(&str, &str)]) -> String {
    vars.iter().fold(t(key, locale), |text, (name, value)| text.replace(&format!("{{{{{name}}}}}"), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Piso de alcance: um parser quebrado encolhe o dicionário para quase zero, não para "quase
    /// certo" — 1500 é bem abaixo dos ~2000 reais de cada locale, com folga para o texto crescer
    /// ou encolher sem o teste ficar frágil.
    #[test]
    fn every_locale_parses_to_a_sane_entry_count() {
        for (locale, source) in LOCALES {
            let dict = parse_dict(source);
            assert!(
                dict.len() > 1500,
                "{locale}: só {} entradas — o parser de locales.ts pode ter quebrado",
                dict.len()
            );
        }
    }

    #[test]
    fn tf_fills_the_named_placeholders() {
        let text = tf("voice.tts.azure.invalidRegion", "en-US", &[("region", "x y")]);
        assert!(text.contains("\"x y\""), "{text}");
        assert!(!text.contains("{{"), "{text}");
    }

    #[test]
    fn resolves_a_known_key_in_the_requested_locale() {
        assert_eq!(t("button.save", "pt-BR"), "Salvar");
        assert_eq!(t("button.save", "en-US"), "Save");
    }

    /// Locale sem a chave (ou locale desconhecido) cai para `en-US` — nunca string vazia.
    #[test]
    fn falls_back_to_en_us_when_the_requested_locale_lacks_the_key_or_is_unknown() {
        assert_eq!(t("button.save", "xx-XX"), "Save");
    }

    /// Chave ausente em TODOS os dicionários (ou texto literal que nunca foi pensado como chave,
    /// caso de `LLM_PROVIDER_OPTIONS`) devolve a própria entrada — nunca vazio, nunca pânico.
    #[test]
    fn unknown_key_passes_through_unchanged() {
        assert_eq!(t("Ollama (local)", "pt-BR"), "Ollama (local)");
        assert_eq!(t("chave.que.nao.existe", "pt-BR"), "chave.que.nao.existe");
    }
}
