//! Extrai pares chave/valor de um `TranslationDict` TS (`packages/sdk/src/i18n/locales/*.ts`) sem
//! reimplementar um parser TS inteiro — o formato é regular: só pares de string entre aspas
//! (simples OU duplas — o Biome/Prettier deste repo escolhe o delimitador que evita escape).
//!
//! **Uma aspa PODE vir escapada dentro do valor** (`\'` dentro de string de aspas simples) quando
//! o texto tem as DUAS aspas ao mesmo tempo — Prettier não tem para onde fugir e escapa uma. Ex.
//! real (`en-US.ts`): `'Could not mark this person as "don\'t remember".'`. Uma primeira leitura
//! deste módulo tinha confirmado "zero aspas escapadas nos 10 locales" com um grep de shell —
//! grep esse que, por escaping errado, sempre dava zero. `cargo test` (que lê o arquivo de
//! verdade, não um grep de terminal) achou o defeito no primeiro `en-US.ts` real: o parser sem
//! suporte a escape perdia a sincronia de pares assim que cruzava a primeira ocorrência e o
//! dicionário encolhia para menos de um terço do tamanho real — sem erro, sem pânico, só um
//! dicionário incompleto. `tests::escaped_apostrophe_inside_a_single_quoted_value` prende esse
//! caso exato; `locales::tests::every_locale_parses_to_a_sane_entry_count` é o gate que o pegou.

use std::collections::BTreeMap;

/// Todo token entre aspas do arquivo, na ordem em que aparece, já sem escape (`\'`/`\"` viram a
/// aspa literal). Não distingue chave de valor — `parse_dict` decide isso pelo pareamento.
fn quoted_tokens(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            let quote = c;
            let mut content = String::new();
            let mut j = i + 1;

            while j < chars.len() && chars[j] != quote {
                if chars[j] == '\\' && j + 1 < chars.len() {
                    // Só o caractere escapado entra no conteúdo — a barra invertida não é dado,
                    // é sintaxe. Não há `\\`/`\n` reais nos 10 locales hoje (conferido), então não
                    // há ambiguidade sobre "escapar o quê": o próximo caractere, sempre.
                    content.push(chars[j + 1]);
                    j += 2;
                } else {
                    content.push(chars[j]);
                    j += 1;
                }
            }

            tokens.push(content);
            i = j + 1;
        } else {
            i += 1;
        }
    }

    tokens
}

/// A primeira string do arquivo é sempre o caminho do `import type` (`'../translation-dict'`) —
/// não é um par chave/valor; o resto pareia (chave, valor) na ordem em que aparece. Um dict TS não
/// tem chave duplicada (se tivesse, a última venceria, igual a um objeto JS — `BTreeMap::insert`
/// reproduz isso sozinho).
pub fn parse_dict(source: &str) -> BTreeMap<String, String> {
    let tokens = quoted_tokens(source);
    let entries = tokens.get(1..).unwrap_or(&[]);

    entries
        .chunks_exact(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"import type { TranslationDict } from '../translation-dict'

export const ptBR: TranslationDict = {
  'button.save': 'Salvar',
  'shell.palette.toolError': 'Não consegui executar "{{label}}"',
  'settings.mood.tab.description':
    'Como o humor da Mika muda.',
  "settings.personCatalog.form.blockButtonLabel": "Não lembrar",
  'settings.personCatalog.crud.errorBlock': 'Could not mark this person as "don\'t remember".',
  'after.the.escape': 'ainda sincronizado',
}
"#;

    #[test]
    fn parses_single_and_double_quoted_entries() {
        let dict = parse_dict(SAMPLE);
        assert_eq!(dict.get("button.save"), Some(&"Salvar".to_string()));
        assert_eq!(
            dict.get("settings.personCatalog.form.blockButtonLabel"),
            Some(&"Não lembrar".to_string())
        );
    }

    /// Valor de aspas simples pode conter aspas duplas literais, sem escape — o terminador é
    /// sempre o MESMO caractere que abriu.
    #[test]
    fn a_single_quoted_value_may_contain_literal_double_quotes() {
        let dict = parse_dict(SAMPLE);
        assert_eq!(
            dict.get("shell.palette.toolError"),
            Some(&"Não consegui executar \"{{label}}\"".to_string())
        );
    }

    /// O defeito real: uma aspa simples ESCAPADA dentro de um valor de aspas simples (porque o
    /// valor também tem aspas duplas, e Prettier não tem delimitador livre de escape). Sem tratar
    /// `\'`, o scanner fecha a string no lugar errado e todo par SEGUINTE no arquivo desalinha.
    #[test]
    fn escaped_apostrophe_inside_a_single_quoted_value_does_not_desync_the_next_pair() {
        let dict = parse_dict(SAMPLE);
        assert_eq!(
            dict.get("settings.personCatalog.crud.errorBlock"),
            Some(&"Could not mark this person as \"don't remember\".".to_string())
        );
        // A prova de que NÃO desalinhou: o par seguinte ainda pareia chave com o valor certo.
        assert_eq!(
            dict.get("after.the.escape"),
            Some(&"ainda sincronizado".to_string())
        );
    }

    /// Chave e valor podem estar em linhas diferentes quando a string é longa — o parser não
    /// depende de fronteira de linha, só de aspas.
    #[test]
    fn key_and_value_may_span_multiple_lines() {
        let dict = parse_dict(SAMPLE);
        assert_eq!(
            dict.get("settings.mood.tab.description"),
            Some(&"Como o humor da Mika muda.".to_string())
        );
    }

    #[test]
    fn ignores_the_leading_import_path() {
        let dict = parse_dict(SAMPLE);
        assert!(!dict.contains_key("../translation-dict"));
    }

    #[test]
    fn empty_source_yields_an_empty_dict() {
        assert!(parse_dict("").is_empty());
    }
}
