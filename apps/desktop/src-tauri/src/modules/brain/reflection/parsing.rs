//! O que fazer com o QUE O MODELO DEVOLVEU: interpretar o JSON e aceitar (ou não) a reflexão.
//! Porte de `ReflectionInsightSchema` (zod) + o pós-processamento de `maybeRun` depois de
//! `generateObject` devolver.

use serde_json::Value;

use super::super::response_json::extract_json_object;

/// Porte do `ReflectionInsightSchema` (zod) do TypeScript: os quatro campos que o modelo devolve.
/// `reflection` vai para a memória episódica; `observations`/`rating` alimentam o laço de metas
/// (`Events.REFLECTED`) — consumidor fora do escopo deste módulo (ver cabeçalho de `mod.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectionInsight {
    pub reflection: String,
    pub observations: Vec<String>,
    pub suggested_custom_instructions: String,
    pub rating: f64,
}

/// Porte do que `maybeRun` faz com `object` depois que `generateObject` devolve: `trim()` em
/// `reflection`, e vazio-depois-do-trim vira `None` — mesma regra de
/// `if (reflection === '') return null`. Uma reflexão em branco não vale a chamada de
/// armazenamento que viria a seguir.
pub fn accept_insight(mut insight: ReflectionInsight) -> Option<ReflectionInsight> {
    insight.reflection = insight.reflection.trim().to_string();
    if insight.reflection.is_empty() {
        return None;
    }
    Some(insight)
}

/// Porte tolerante do `ReflectionInsightSchema` (zod): extrai o objeto JSON da resposta bruta do
/// modelo e valida os quatro campos. `rating` fora de `[0, 10]` é recortado (`clamp`), igual ao
/// `z.number().min(0).max(10)` do TS rejeitar — mas aqui rejeitar destruiria uma reflexão por
/// inteiro por causa de um número fora de faixa; recortar é honesto e não descarta o resto.
pub(super) fn parse_reflection_json(text: &str) -> Result<ReflectionInsight, String> {
    let json_text = extract_json_object(text).ok_or_else(|| "resposta do modelo não contém um objeto JSON".to_string())?;
    let value: Value = serde_json::from_str(json_text).map_err(|e| format!("JSON de reflexão inválido: {e}"))?;

    let reflection = value.get("reflection").and_then(Value::as_str).ok_or_else(|| "campo 'reflection' ausente ou não é texto".to_string())?;
    let observations = value
        .get("observations")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let suggested_custom_instructions = value.get("suggestedCustomInstructions").and_then(Value::as_str).unwrap_or_default().to_string();
    let rating = value.get("rating").and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 10.0);

    Ok(ReflectionInsight { reflection: reflection.to_string(), observations, suggested_custom_instructions, rating })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insight(reflection: &str) -> ReflectionInsight {
        ReflectionInsight {
            reflection: reflection.to_string(),
            observations: vec!["ser mais direta".to_string()],
            suggested_custom_instructions: String::new(),
            rating: 4.0,
        }
    }

    // --- insight: trim + vazio ------------------------------------------------------------------

    #[test]
    fn accept_insight_apara_espacos_da_reflexao() {
        let accepted = accept_insight(insight("  texto útil  ")).unwrap();
        assert_eq!(accepted.reflection, "texto útil");
    }

    #[test]
    fn accept_insight_devolve_none_quando_so_sobra_espaco_apos_o_trim() {
        assert!(accept_insight(insight("   ")).is_none());
    }

    #[test]
    fn accept_insight_devolve_none_para_string_vazia() {
        assert!(accept_insight(insight("")).is_none());
    }

    #[test]
    fn accept_insight_preserva_os_demais_campos() {
        let accepted = accept_insight(insight("ok")).unwrap();
        assert_eq!(accepted.observations, vec!["ser mais direta".to_string()]);
        assert_eq!(accepted.rating, 4.0);
    }

    // --- parse_reflection_json ---------------------------------------------------------------
    // extract_json_object agora é compartilhado — ver testes em `super::super::response_json`.

    #[test]
    fn parse_reflection_json_le_os_quatro_campos() {
        let text = r#"{"reflection": "  foi um bom papo  ", "observations": ["gosta de RPG"], "suggestedCustomInstructions": "seja mais direta", "rating": 7}"#;
        let insight = parse_reflection_json(text).unwrap();
        assert_eq!(insight.reflection, "  foi um bom papo  ", "trim é responsabilidade de accept_insight, não do parser");
        assert_eq!(insight.observations, vec!["gosta de RPG".to_string()]);
        assert_eq!(insight.suggested_custom_instructions, "seja mais direta");
        assert_eq!(insight.rating, 7.0);
    }

    #[test]
    fn parse_reflection_json_recorta_rating_fora_da_faixa_em_vez_de_rejeitar_tudo() {
        let text = r#"{"reflection": "ok", "rating": 999}"#;
        assert_eq!(parse_reflection_json(text).unwrap().rating, 10.0);
    }

    #[test]
    fn parse_reflection_json_falha_alto_sem_o_campo_reflection() {
        let text = r#"{"observations": [], "rating": 1}"#;
        assert!(parse_reflection_json(text).is_err());
    }

    #[test]
    fn parse_reflection_json_falha_alto_sem_json_nenhum() {
        assert!(parse_reflection_json("não sei o que responder").is_err());
    }

    #[test]
    fn parse_reflection_json_aceita_campos_ausentes_opcionais_com_default() {
        let insight = parse_reflection_json(r#"{"reflection": "ok"}"#).unwrap();
        assert!(insight.observations.is_empty());
        assert_eq!(insight.suggested_custom_instructions, "");
        assert_eq!(insight.rating, 0.0);
    }

    // --- reprodução de saída real do Gemma local (defects.md "memory-relationship" P0) -----------
    // O prompt antigo (`reflection-subconscious.md`) mostrava bullets Markdown como exemplo de saída
    // e pedia uma chamada de ferramenta indisponível nesta etapa tool-free — o parser sempre foi
    // tolerante (ver `response_json.rs`), o defeito real era o PROMPT ensinando o formato errado.
    // Estes dois testes travam que o parser sobrevive ao padrão que o modelo ainda pode produzir
    // por inércia (few-shot antigo, cache, ou um modelo que ignora a instrução).

    #[test]
    fn parse_reflection_json_sobrevive_a_bullets_markdown_antes_do_json_em_cerca_de_codigo() {
        let text = "Aqui vai minha reflexão:\n\
                    - Mika foi muito prolixa sobre Elden Ring\n\
                    - Usuário pareceu frustrado\n\n\
                    ```json\n\
                    {\"reflection\": \"• foi prolixa\\n• usuário frustrado\", \"observations\": [\"ser mais direta\"], \"suggestedCustomInstructions\": \"\", \"rating\": 7}\n\
                    ```";
        let insight = parse_reflection_json(text).unwrap();
        assert_eq!(insight.reflection, "• foi prolixa\n• usuário frustrado");
        assert_eq!(insight.observations, vec!["ser mais direta".to_string()]);
        assert_eq!(insight.rating, 7.0);
    }

    #[test]
    fn parse_reflection_json_sobrevive_a_json_sem_cerca_com_prosa_em_volta() {
        let text = "Claro, aqui está a análise solicitada:\n\
                    {\"reflection\": \"• persona mantida\", \"rating\": 3}\n\
                    Qualquer coisa, é só chamar!";
        let insight = parse_reflection_json(text).unwrap();
        assert_eq!(insight.reflection, "• persona mantida");
        assert_eq!(insight.rating, 3.0);
    }
}
