//! Contexto de memória para o prompt — porte de `episodic-context.ts::buildContext`.
//!
//! A data de cada fato é `YYYY-MM-DD` no fuso do SO (`chrono::Local`, como `persona/user_clock.rs`);
//! o TS usava `Intl.DateTimeFormat` no locale do usuário — só o FORMATO diverge, o dia é o mesmo.

use chrono::{Local, TimeZone};

use super::types::MemoryEntry;

fn local_date<Tz: TimeZone>(timestamp_ms: i64, tz: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    tz.timestamp_millis_opt(timestamp_ms).single().map_or_else(String::new, |d| d.format("%Y-%m-%d").to_string())
}

/// Uma memória já pontuada e pronta para virar uma linha `[Fato N | data | ...]: conteúdo`.
pub struct ContextFlags {
    pub emotional_tagging: bool,
}

/// = `buildContext`, já com a lista de memórias relevantes resolvida pelo chamador (busca semântica
/// com fallback para palavra-chave — a MESMA escolha que `buildContext` faz, só que a decisão de
/// QUAL busca rodar fica no `service.rs`, não aqui).
pub fn build_context(relevant: &[MemoryEntry], flags: ContextFlags) -> String {
    if relevant.is_empty() {
        return String::new();
    }

    let lines: Vec<String> = relevant
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let date = local_date(e.timestamp, &Local);
            let mut prefix = format!("[Fact {} | {date}", i + 1);
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&e.metadata) {
                if flags.emotional_tagging {
                    if let Some(emotion) = meta.get("emotion").and_then(|v| v.as_str()) {
                        if emotion != "neutral" {
                            prefix += &format!(" | Sentiment: {emotion}");
                        }
                    }
                }
                let importance = meta.get("importance");
                let is_important = importance.and_then(|v| v.as_f64()).map(|n| n >= 4.0).unwrap_or(false)
                    || importance == Some(&serde_json::Value::Bool(true))
                    || importance.and_then(|v| v.as_str()) == Some("true");
                if is_important {
                    prefix += " | Important";
                }
            }
            prefix += "]";
            format!("{prefix}: {}", e.content)
        })
        .collect();

    format!("\n\n## Relevant Memories\n{}\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(content: &str, ts: i64, metadata: &str) -> MemoryEntry {
        MemoryEntry { id: "id".into(), content: content.into(), r#type: "conversation".into(), timestamp: ts, metadata: metadata.into(), embedding: None }
    }

    #[test]
    fn empty_relevant_list_yields_empty_context() {
        assert_eq!(build_context(&[], ContextFlags { emotional_tagging: true }), "");
    }

    #[test]
    fn formats_date_emotion_and_importance_tags() {
        let e = entry("gosta de rust", 1_700_000_000_000, "{\"emotion\":\"feliz\",\"importance\":5}");
        let ctx = build_context(&[e], ContextFlags { emotional_tagging: true });
        assert!(ctx.contains("Sentiment: feliz"), "{ctx}");
        assert!(ctx.contains("Important"), "{ctx}");
        assert!(ctx.contains("gosta de rust"), "{ctx}");
    }

    #[test]
    fn neutral_emotion_is_never_shown() {
        let e = entry("neutro", 0, "{\"emotion\":\"neutral\"}");
        let ctx = build_context(&[e], ContextFlags { emotional_tagging: true });
        assert!(!ctx.contains("Sentiment"), "{ctx}");
    }

    /// 2023-11-14T22:13:20Z já é dia 14 em UTC, mas 01:13 do dia 15 em UTC+3: o fato é do dia local.
    #[test]
    fn date_is_the_calendar_day_in_the_user_zone() {
        let utc = chrono::Utc;
        let plus3 = chrono::FixedOffset::east_opt(3 * 3600).unwrap();
        assert_eq!(local_date(0, &utc), "1970-01-01");
        assert_eq!(local_date(1_700_000_000_000, &utc), "2023-11-14");
        assert_eq!(local_date(1_700_000_000_000, &plus3), "2023-11-15");
    }
}
