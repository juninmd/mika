//! Conversão epoch-ms <-> ISO 8601 UTC (`YYYY-MM-DDTHH:MM:SS.sssZ`), SEM `chrono` — não declarada
//! em `Cargo.toml` (fora do que esta rodada pode adicionar; `Cargo.toml` é de outro dono).
//!
//! **Por que isto precisa existir.** `game_sessions.start_time`/`end_time` (SQLite) são TEXT no
//! formato que `new Date().toISOString()` já grava — `session-memory-rows.ts::rowToSession` lê de
//! volta com `new Date(row.start_time as string)`. Gravar em outro formato (ex.: inteiro epoch)
//! quebraria a ordenação lexicográfica de `ORDER BY start_time DESC` contra as linhas ANTIGAS que o
//! TS já escreveu no disco do usuário — a mesma classe de defeito que `chat_db.rs` documenta no
//! cabeçalho para `chat.db`.
//!
//! Algoritmo de calendário (`days_from_civil`/`civil_from_days`) é o de Howard Hinnant, domínio
//! público, o mesmo que `chrono`/`libc++` usam por baixo — aritmética inteira pura, sem tabela de
//! fuso horário (não precisa: é sempre UTC).

/// Dias desde 1970-01-01 (UTC) para uma data civil `(ano, mês 1-12, dia 1-31)`.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64; // [0, 399]
    let mp = (m as i64 + 9) % 12; // Mar=0 .. Feb=11
    let doy = (153 * mp + 2) / 5 + d as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Inverso de [`days_from_civil`]: dias desde epoch -> `(ano, mês, dia)`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `epoch_ms` -> `"YYYY-MM-DDTHH:MM:SS.sssZ"`, byte-idêntico a `Date.prototype.toISOString()` para
/// qualquer instante >= 1970-01-01 (sessões de jogo nunca têm timestamp negativo).
pub fn format_iso8601(epoch_ms: u64) -> String {
    let days = (epoch_ms / 86_400_000) as i64;
    let rem_ms = epoch_ms % 86_400_000;
    let (y, m, d) = civil_from_days(days);
    let hh = rem_ms / 3_600_000;
    let mm = (rem_ms / 60_000) % 60;
    let ss = (rem_ms / 1_000) % 60;
    let millis = rem_ms % 1_000;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.{millis:03}Z")
}

/// Instante atual em epoch-ms — mesmo helper privado que `session_memory.rs` já tinha, exportado
/// aqui para [`super::game_locations`] não duplicar a mesma conta de 3 linhas.
pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Inverso de [`format_iso8601`], tolerante a milissegundos ausentes (`...T00:00:00Z`, sem `.sss`)
/// — defensivo contra uma linha gravada por um caminho diferente do `toISOString()` padrão.
/// `None` para qualquer texto que não bata no formato, nunca panic.
pub fn parse_iso8601(text: &str) -> Option<u64> {
    let s = text.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut date_parts = date.split('-');
    let y: i64 = date_parts.next()?.parse().ok()?;
    let m: u32 = date_parts.next()?.parse().ok()?;
    let d: u32 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }

    let (hms, millis) = match time.split_once('.') {
        Some((hms, frac)) => (hms, frac),
        None => (time, "0"),
    };
    let mut time_parts = hms.split(':');
    let hh: u64 = time_parts.next()?.parse().ok()?;
    let mm: u64 = time_parts.next()?.parse().ok()?;
    let ss: u64 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() || hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    let millis: u64 = format!("{millis:0<3}").get(..3)?.parse().ok()?;

    let days = days_from_civil(y, m, d);
    let day_ms = (days as i128) * 86_400_000;
    let time_ms = (hh * 3_600_000 + mm * 60_000 + ss * 1_000 + millis) as i128;
    u64::try_from(day_ms + time_ms).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_the_unix_epoch_instant() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn formats_a_known_instant_with_milliseconds() {
        // 2024-02-29T13:45:30.250Z (ano bissexto) — vetor calculado independentemente.
        let ms = 1_709_214_330_250;
        assert_eq!(format_iso8601(ms), "2024-02-29T13:45:30.250Z");
    }

    #[test]
    fn round_trips_through_format_and_parse() {
        for ms in [0u64, 1_000, 86_399_999, 1_709_214_330_250, 1_893_456_000_000, 1_735_689_599_999] {
            let text = format_iso8601(ms);
            assert_eq!(parse_iso8601(&text), Some(ms), "round-trip falhou para {text}");
        }
    }

    #[test]
    fn parses_the_typescript_toisostring_format_exactly() {
        // Formato literal que `new Date().toISOString()` produz — o que já está gravado no disco
        // dos usuários existentes.
        //
        // Vetor conferido à mão, não pela implementação: 1_758_446_400_000 ms = 1_758_446_400 s;
        // 1_758_446_400 / 86_400 = 20_352 dias exatos com resto 33_600 s = 09:20:00. E 20_352 dias
        // desde 1970-01-01 = 55 anos (20_075 dias) + 14 bissextos (1972..2024) = 20_089 dias até
        // 2025-01-01, mais 263 dias (Jan 31 + Fev 28 + ... + Ago 31 = 243, mais 20) = 2025-09-21.
        assert_eq!(format_iso8601(1_758_446_400_000), "2025-09-21T09:20:00.000Z");
        assert_eq!(parse_iso8601("2025-09-21T09:20:00.000Z"), Some(1_758_446_400_000));
    }

    #[test]
    fn tolerates_missing_milliseconds() {
        assert_eq!(parse_iso8601("1970-01-01T00:00:01Z"), Some(1_000));
    }

    #[test]
    fn rejects_malformed_text_instead_of_panicking() {
        assert_eq!(parse_iso8601("not-a-date"), None);
        assert_eq!(parse_iso8601("2026-13-01T00:00:00.000Z"), None);
        assert_eq!(parse_iso8601("2026-09-21T25:00:00.000Z"), None);
        assert_eq!(parse_iso8601(""), None);
    }
}
