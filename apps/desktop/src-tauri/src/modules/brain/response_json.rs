//! Extração tolerante de um objeto JSON `{ ... }` de dentro de uma resposta de LLM em texto livre
//! — nenhum provedor alcançável por [`super::turn::run_turn_llm_raw`] garante saída estruturada
//! de verdade (o `generateObject`/`generateStructuredToolCall` do AI SDK que o TypeScript usava
//! não tem equivalente aqui), então toda extração estruturada deste crate (reflexão, fatos) precisa
//! do mesmo passo: achar o JSON dentro do texto, tolerando cerca de código markdown.
//!
//! Compartilhado por [`super::reflection::parsing`] e [`super::post_turn::facts`] — as duas rodadas
//! de extração deste módulo (`ReflectionInsightSchema` e `FactSchema` no TypeScript) tinham a MESMA
//! função duplicada até esta rodada; um bug de balanceamento de chaves corrigido num lado e não no
//! outro é a classe de defeito que a extração para um arquivo só evita.

/// Extrai o primeiro objeto JSON `{ ... }` balanceado do texto — tolera a resposta vir envolta em
/// ` ```json ... ``` `. Conta chaves respeitando string entre aspas (uma `{` dentro de um valor de
/// string não deve contar).
pub(super) fn extract_json_object(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = text.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, &byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + offset + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acha_o_objeto_em_texto_puro() {
        let text = "aqui está: {\"a\": \"ok\"} — fim";
        assert_eq!(extract_json_object(text), Some("{\"a\": \"ok\"}"));
    }

    #[test]
    fn ignora_a_cerca_de_codigo_markdown() {
        let text = "```json\n{\"a\": \"vale a pena\"}\n```";
        assert_eq!(extract_json_object(text), Some("{\"a\": \"vale a pena\"}"));
    }

    /// Uma `{` dentro de uma STRING não pode contar para o balanceamento.
    #[test]
    fn nao_se_confunde_com_chave_dentro_de_string() {
        let text = "{\"a\": \"ela disse: { isso não é json }\"}";
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn devolve_none_sem_objeto_nenhum() {
        assert_eq!(extract_json_object("desculpa, não consigo responder agora"), None);
    }

    /// Gemma local, tool-free, tende a "pensar em voz alta" em bullets ANTES de obedecer ao formato
    /// pedido — o objeto real vem depois, dentro de cerca de código. Reprodução do padrão real que
    /// derrubava `reflection::parsing` (defects.md "memory-relationship" P0).
    #[test]
    fn acha_o_objeto_com_bullets_de_markdown_antes_da_cerca_de_codigo() {
        let text = "- Mika foi direta\n- Persona mantida\n\n```json\n{\"reflection\": \"ok\", \"rating\": 6}\n```";
        assert_eq!(extract_json_object(text), Some("{\"reflection\": \"ok\", \"rating\": 6}"));
    }

    /// Mesmo padrão, sem cerca de código: só prosa solta antes do objeto.
    #[test]
    fn acha_o_objeto_com_prosa_antes_e_depois_sem_cerca_nenhuma() {
        let text = "Aqui está minha análise:\n{\"reflection\": \"ok\"}\nEspero que ajude!";
        assert_eq!(extract_json_object(text), Some("{\"reflection\": \"ok\"}"));
    }
}
