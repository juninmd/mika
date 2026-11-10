//! Port de `services/steam-vdf.ts` — parser mínimo do formato VDF (Valve Data Format) usado por
//! `libraryfolders.vdf`, `appmanifest_*.acf` e `localconfig.vdf`. Sem crate nova: é um parser de
//! texto recursivo-descendente, igual ao TS.

use std::collections::HashMap;

pub type VdfObject = HashMap<String, VdfNode>;

#[derive(Debug, Clone, PartialEq)]
pub enum VdfNode {
    Value(String),
    Object(VdfObject),
}

fn skip_whitespace_and_comments(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            _ => break,
        }
    }
    i
}

/// Lê uma string entre aspas a partir de `start` (que deve apontar para `"`). Decodifica `\n`/`\t`
/// escapados; qualquer outro caractere escapado passa como está. Texto truncado sem aspas de
/// fechamento devolve `None` — quem chama trata como "parcial", nunca entra em loop infinito.
fn read_quoted(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start) != Some(&'"') {
        return None;
    }
    let mut i = start + 1;
    let mut value = String::new();
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                match chars.get(i + 1) {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some(&other) => value.push(other),
                    None => {}
                }
                i += 2;
            }
            '"' => return Some((value, i + 1)),
            c => {
                value.push(c);
                i += 1;
            }
        }
    }
    None
}

fn parse_object(chars: &[char], start: usize) -> (VdfObject, usize) {
    let mut result = VdfObject::new();
    let mut i = start;
    while i < chars.len() {
        i = skip_whitespace_and_comments(chars, i);
        if chars.get(i) == Some(&'}') {
            return (result, i + 1);
        }

        let Some((key, next)) = read_quoted(chars, i) else {
            return (result, i + 1);
        };
        i = skip_whitespace_and_comments(chars, next);

        if chars.get(i) == Some(&'{') {
            let (nested, next2) = parse_object(chars, i + 1);
            result.insert(key, VdfNode::Object(nested));
            i = next2;
            continue;
        }

        let Some((value, next2)) = read_quoted(chars, i) else {
            return (result, i);
        };
        result.insert(key, VdfNode::Value(value));
        i = next2;
    }
    (result, i)
}

pub fn parse_vdf(text: &str) -> VdfObject {
    let chars: Vec<char> = text.chars().collect();
    let mut result = VdfObject::new();
    let mut i = 0;
    while i < chars.len() {
        i = skip_whitespace_and_comments(&chars, i);
        let Some((key, next)) = read_quoted(&chars, i) else { break };
        i = skip_whitespace_and_comments(&chars, next);
        if chars.get(i) != Some(&'{') {
            break;
        }
        let (nested, next2) = parse_object(&chars, i + 1);
        result.insert(key, VdfNode::Object(nested));
        i = next2;
    }
    result
}

fn find_key<'a>(obj: &'a VdfObject, key: &str) -> Option<&'a str> {
    if let Some((actual, _)) = obj.get_key_value(key) {
        return Some(actual.as_str());
    }
    obj.keys().find(|k| k.eq_ignore_ascii_case(key)).map(String::as_str)
}

/// Navega por chaves aninhadas, tentando a chave exata antes de cair para busca
/// case-insensitive — mesma regra de `vdfPath` (`steam-vdf.ts`).
pub fn vdf_path<'a>(root: Option<&'a VdfObject>, keys: &[&str]) -> Option<&'a VdfObject> {
    let mut current = root?;
    for key in keys {
        let actual = find_key(current, key)?;
        match current.get(actual)? {
            VdfNode::Object(obj) => current = obj,
            VdfNode::Value(_) => return None,
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(v: &str) -> VdfNode {
        VdfNode::Value(v.to_string())
    }

    #[test]
    fn reads_simple_key_value_pairs_inside_a_root_object() {
        let parsed = parse_vdf("\"root\"\n{\n  \"key\" \"value\"\n}");
        let VdfNode::Object(root) = parsed.get("root").unwrap() else { panic!("expected object") };
        assert_eq!(root.get("key"), Some(&value("value")));
    }

    #[test]
    fn reads_nested_objects_at_any_depth() {
        let parsed = parse_vdf("\"a\" { \"b\" { \"c\" \"1\" } }");
        let VdfNode::Object(a) = parsed.get("a").unwrap() else { panic!() };
        let VdfNode::Object(b) = a.get("b").unwrap() else { panic!() };
        assert_eq!(b.get("c"), Some(&value("1")));
    }

    #[test]
    fn ignores_comment_lines_without_breaking_the_surrounding_parse() {
        let parsed = parse_vdf("\"root\"\n{\n  // comentário\n  \"key\" \"value\"\n}");
        let VdfNode::Object(root) = parsed.get("root").unwrap() else { panic!() };
        assert_eq!(root.get("key"), Some(&value("value")));
    }

    #[test]
    fn decodes_escaped_newline_and_tab_inside_quoted_values() {
        let parsed = parse_vdf("\"root\" { \"key\" \"linha1\\nlinha2\\tfim\" }");
        let VdfNode::Object(root) = parsed.get("root").unwrap() else { panic!() };
        assert_eq!(root.get("key"), Some(&value("linha1\nlinha2\tfim")));
    }

    #[test]
    fn empty_text_returns_empty_object_instead_of_panicking() {
        assert!(parse_vdf("").is_empty());
    }

    #[test]
    fn truncated_text_mid_value_does_not_infinite_loop_and_returns_the_partial_result() {
        let parsed = parse_vdf("\"root\" { \"key\" \"valor sem fechar aspas");
        let VdfNode::Object(root) = parsed.get("root").unwrap() else { panic!() };
        assert!(root.is_empty());
    }

    #[test]
    fn walks_multiple_nested_keys() {
        let root = parse_vdf("\"a\" { \"b\" { \"c\" { \"d\" \"valor\" } } }");
        let found = vdf_path(Some(&root), &["a", "b", "c"]).unwrap();
        assert_eq!(found.get("d"), Some(&value("valor")));
    }

    #[test]
    fn falls_back_to_case_insensitive_lookup_when_the_exact_key_is_missing() {
        let root = parse_vdf("\"UserLocalConfigStore\" { \"Apps\" { \"10\" { \"Playtime\" \"5\" } } }");
        let found = vdf_path(Some(&root), &["UserLocalConfigStore", "apps"]).unwrap();
        let VdfNode::Object(entry) = found.get("10").unwrap() else { panic!() };
        assert_eq!(entry.get("Playtime"), Some(&value("5")));
    }

    #[test]
    fn returns_none_when_the_path_does_not_exist() {
        let root = parse_vdf("\"a\" { \"b\" \"1\" }");
        assert!(vdf_path(Some(&root), &["a", "x", "y"]).is_none());
    }
}
