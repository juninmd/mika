//! Tempo de uso por programa — o livro-caixa puro por trás de "quais programas você mais usa".
//!
//! Guarda SÓ o nome do processo em primeiro plano (sem título de janela, sem conteúdo) e segundos
//! por dia civil; dias além de [`KEEP_DAYS`] são podados. Quem alimenta e persiste é
//! [`super::habits`].

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Janela mais longa exposta (top de 30 dias); o resto é apagado.
pub const KEEP_DAYS: i64 = 30;
const NAME_CAP: usize = 40;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageLog {
    /// `YYYY-MM-DD` → programa → segundos.
    days: BTreeMap<String, BTreeMap<String, u64>>,
}

fn day_key(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

/// Nome estável para somar: sem espaço sobrando nem `.exe`, capado. Vazio → `None`.
pub fn program_key(owner: &str) -> Option<String> {
    let trimmed = owner.trim();
    let name = trimmed.strip_suffix(".exe").or_else(|| trimmed.strip_suffix(".EXE")).unwrap_or(trimmed).trim();
    (!name.is_empty()).then(|| name.chars().take(NAME_CAP).collect())
}

impl UsageLog {
    pub fn record(&mut self, day: NaiveDate, program: &str, secs: u64) {
        let Some(key) = program_key(program) else { return };
        *self.days.entry(day_key(day)).or_default().entry(key).or_insert(0) += secs;
    }

    /// Apaga dias com mais de [`KEEP_DAYS`] (e datas futuras, de relógio adiantado).
    pub fn prune(&mut self, today: NaiveDate) {
        let oldest = day_key(today - chrono::Duration::days(KEEP_DAYS - 1));
        let newest = day_key(today);
        self.days.retain(|day, _| *day >= oldest && *day <= newest);
    }

    /// Os `n` programas com mais tempo nos últimos `days` dias (hoje incluso), maior primeiro.
    pub fn top(&self, today: NaiveDate, days: i64, n: usize) -> Vec<(String, u64)> {
        let oldest = day_key(today - chrono::Duration::days(days - 1));
        let newest = day_key(today);
        let mut totals: BTreeMap<&str, u64> = BTreeMap::new();
        for (_, programs) in self.days.range(oldest..=newest) {
            for (name, secs) in programs {
                *totals.entry(name).or_insert(0) += secs;
            }
        }
        let mut ranked: Vec<(String, u64)> = totals.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked.truncate(n);
        ranked
    }
}

/// `Code 12h, chrome 3h, Discord 40min` — compacto para o prompt; abaixo de 30 min não entra.
pub fn compact_list(entries: &[(String, u64)]) -> String {
    entries
        .iter()
        .filter(|(_, secs)| *secs >= 30 * 60)
        .map(|(name, secs)| {
            let minutes = secs / 60;
            if minutes < 60 { format!("{name} {minutes}min") } else { format!("{name} {}h", (minutes + 30) / 60) }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, d).unwrap()
    }

    #[test]
    fn top_sums_the_window_and_ranks_by_time() {
        let mut log = UsageLog::default();
        log.record(day(20), "Code.exe", 3_000);
        log.record(day(27), "Code", 600);
        log.record(day(27), "chrome", 3_000);
        log.record(day(28), "Discord", 100);

        let week: Vec<(String, u64)> = vec![("chrome".into(), 3_000), ("Code".into(), 600), ("Discord".into(), 100)];
        assert_eq!(log.top(day(28), 7, 5), week, "o dia 20 fica fora da semana");
        assert_eq!(log.top(day(28), 30, 1), vec![("Code".to_string(), 3_600)]);
    }

    #[test]
    fn prune_drops_days_past_the_retention_window() {
        let mut log = UsageLog::default();
        log.record(NaiveDate::from_ymd_opt(2026, 8, 29).unwrap(), "old", 60);
        log.record(NaiveDate::from_ymd_opt(2026, 8, 30).unwrap(), "kept", 60);
        log.record(day(29), "future", 60);
        log.prune(day(28));
        assert_eq!(log.top(day(28), 30, 5), vec![("kept".to_string(), 60)]);
        assert!(log.top(day(29), 1, 5).is_empty(), "data futura (relógio adiantado) não sobrevive à poda");
    }

    #[test]
    fn program_names_are_normalized_and_blank_ones_ignored() {
        assert_eq!(program_key("  Code.exe "), Some("Code".to_string()));
        assert_eq!(program_key("   "), None);
        let mut log = UsageLog::default();
        log.record(day(28), "", 60);
        assert_eq!(log, UsageLog::default());
    }

    #[test]
    fn compact_list_skips_trivial_use_and_rounds_hours() {
        let list = compact_list(&[("Code".into(), 4 * 3600 + 40 * 60), ("chrome".into(), 45 * 60), ("calc".into(), 10 * 60)]);
        assert_eq!(list, "Code 5h, chrome 45min");
    }
}
