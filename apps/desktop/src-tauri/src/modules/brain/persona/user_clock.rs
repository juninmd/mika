//! === infrastructure/user-clock.ts (fatia usada por `## TIME AWARENESS`) ===================
//!
//! [`user_now`] resolve o relógio do USUÁRIO a cada chamada (nunca no boot): `user.timezone` quando
//! é um fuso IANA válido (`America/Sao_Paulo`) ou um deslocamento fixo (`UTC-3`, `GMT+05:30`,
//! `-03:00`), senão o fuso do SO — mesma ordem de `resolveUserTimeZone` no TS. O banco IANA vem do
//! `jiff` (zoneinfo do SO no Linux/macOS, embutido no Windows). Antes daqui o prompt usava só
//! `chrono::Local` e ignorava o campo que a tela de Usuário promete ("para a Mika saber se é dia ou
//! noite aí").
//!
//! - **Nomes de dia/mês e formato por locale** para os 10 `app.locale` suportados, em tabela própria
//!   (a tabela inteira do `Intl` pediria uma dependência nova, `icu`). Hora sempre em 24h. Locale
//!   desconhecido cai no formato inglês.

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

use crate::config::store::ConfigStore;

const WEEKDAYS_PT: [&str; 7] =
    ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];
const MONTHS_PT: [&str; 12] =
    ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

const WEEKDAYS_ES: [&str; 7] = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];
const MONTHS_ES: [&str; 12] =
    ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

const WEEKDAYS_FR: [&str; 7] = ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"];
const MONTHS_FR: [&str; 12] =
    ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

const WEEKDAYS_IT: [&str; 7] = ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"];
const MONTHS_IT: [&str; 12] =
    ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"];

const WEEKDAYS_DE: [&str; 7] = ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"];
const MONTHS_DE: [&str; 12] =
    ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];

const WEEKDAYS_RU: [&str; 7] = ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"];
/// Genitivo — é a forma que acompanha o dia ("24 сентября").
const MONTHS_RU: [&str; 12] =
    ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];

const WEEKDAYS_JA: [&str; 7] = ["日曜日", "月曜日", "火曜日", "水曜日", "木曜日", "金曜日", "土曜日"];
const WEEKDAYS_ZH: [&str; 7] = ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"];
const WEEKDAYS_KO: [&str; 7] = ["일요일", "월요일", "화요일", "수요일", "목요일", "금요일", "토요일"];

/// O instante atual no fuso do usuário: data/hora civil + como nomear o fuso no prompt.
#[derive(Debug, Clone)]
pub(crate) struct UserNow {
    pub local: NaiveDateTime,
    /// `America/Sao_Paulo (UTC-03:00)` ou só `UTC-03:00` quando o fuso não tem nome IANA.
    pub zone_label: String,
}

/// `UTC-3`, `GMT+05:30`, `-03:00`, `+0530` → segundos. Qualquer outra coisa → `None`.
fn parse_fixed_offset(raw: &str) -> Option<i32> {
    let upper = raw.trim().to_uppercase();
    let rest = upper.strip_prefix("UTC").or_else(|| upper.strip_prefix("GMT")).unwrap_or(&upper).trim();
    let (sign, digits) = match rest.chars().next()? {
        '+' => (1, &rest[1..]),
        '-' => (-1, &rest[1..]),
        _ => return None,
    };
    let (hours, minutes) = match digits.split_once(':') {
        Some((h, m)) => (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?),
        None if digits.len() == 4 && digits.is_ascii() => (digits[..2].parse().ok()?, digits[2..].parse().ok()?),
        None => (digits.parse().ok()?, 0),
    };
    ((0..=14).contains(&hours) && (0..60).contains(&minutes)).then_some(sign * (hours * 3600 + minutes * 60))
}

/// `user.timezone` → fuso. Vazio ou desconhecido cai no do SO, nunca em UTC por acidente.
fn resolve_zone(configured: &str) -> jiff::tz::TimeZone {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return jiff::tz::TimeZone::system();
    }
    if let Some(seconds) = parse_fixed_offset(trimmed) {
        if let Ok(offset) = jiff::tz::Offset::from_seconds(seconds) {
            return jiff::tz::TimeZone::fixed(offset);
        }
    }
    jiff::tz::TimeZone::get(trimmed).unwrap_or_else(|_| {
        log::debug!("user.timezone '{trimmed}' não é um fuso conhecido — usando o do sistema");
        jiff::tz::TimeZone::system()
    })
}

fn offset_label(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.abs();
    format!("UTC{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

/// Converte um instante (`epoch_ms`) para o relógio civil do fuso `configured`. Pura sobre o
/// instante — o chamador de produção passa "agora", o teste passa um instante fixo.
pub(crate) fn user_time_at(epoch_ms: i64, configured: &str) -> UserNow {
    let tz = resolve_zone(configured);
    let zoned = jiff::Timestamp::from_millisecond(epoch_ms).unwrap_or_else(|_| jiff::Timestamp::now()).to_zoned(tz.clone());
    let local = NaiveDate::from_ymd_opt(i32::from(zoned.year()), zoned.month() as u32, zoned.day() as u32)
        .and_then(|d| d.and_hms_opt(zoned.hour() as u32, zoned.minute() as u32, zoned.second() as u32))
        .unwrap_or_default();
    let offset = offset_label(zoned.offset().seconds());
    let zone_label = match tz.iana_name() {
        Some(name) if name != "UTC" => format!("{name} ({offset})"),
        _ => offset,
    };
    UserNow { local, zone_label }
}

/// O relógio do usuário AGORA, relido do store a cada chamada (o turno nunca herda a hora do boot).
pub(crate) fn user_now(store: &ConfigStore) -> UserNow {
    let configured = store.get("user.timezone").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    user_time_at(jiff::Timestamp::now().as_millisecond(), &configured)
}

/// Espelha `formatUserDateTime` (`dateStyle: 'full', timeStyle: 'short'`); o fuso vai numa linha
/// própria do prompt ([`UserNow::zone_label`]).
pub fn format_user_date_time(now: NaiveDateTime, locale: &str) -> String {
    let (weekday, month0) = (now.weekday().num_days_from_sunday() as usize, (now.month() - 1) as usize);
    let (d, m, y) = (now.day(), now.month(), now.year());
    let time = format!("{:02}:{:02}", now.hour(), now.minute());
    match crate::config::locale::language_code(&locale.trim().to_lowercase()) {
        "pt" => format!("{}, {d} de {} de {y}, {time}", WEEKDAYS_PT[weekday], MONTHS_PT[month0]),
        "es" => format!("{}, {d} de {} de {y}, {time}", WEEKDAYS_ES[weekday], MONTHS_ES[month0]),
        "fr" => format!("{} {d} {} {y} à {time}", WEEKDAYS_FR[weekday], MONTHS_FR[month0]),
        "it" => format!("{} {d} {} {y} alle ore {time}", WEEKDAYS_IT[weekday], MONTHS_IT[month0]),
        "de" => format!("{}, {d}. {} {y} um {time} Uhr", WEEKDAYS_DE[weekday], MONTHS_DE[month0]),
        "ru" => format!("{}, {d} {} {y} г. в {time}", WEEKDAYS_RU[weekday], MONTHS_RU[month0]),
        "ja" => format!("{y}年{m}月{d}日{} {time}", WEEKDAYS_JA[weekday]),
        "zh" => format!("{y}年{m}月{d}日{} {time}", WEEKDAYS_ZH[weekday]),
        "ko" => format!("{y}년 {m}월 {d}일 {} {time}", WEEKDAYS_KO[weekday]),
        _ => now.format("%A, %B %-d, %Y, %H:%M").to_string(),
    }
}

/// Espelha `formatUserDate` — `toLocaleDateString` SEM `dateStyle`, formato numérico curto do
/// `Intl` de cada locale (`24/09/2025` em pt-BR, `24.9.2025` em de-DE, `2025/9/24` em ja-JP).
pub fn format_user_date(now: NaiveDateTime, locale: &str) -> String {
    let (d, m, y) = (now.day(), now.month(), now.year());
    match crate::config::locale::language_code(&locale.trim().to_lowercase()) {
        "pt" | "fr" => format!("{d:02}/{m:02}/{y}"),
        "es" | "it" => format!("{d}/{m}/{y}"),
        "de" => format!("{d}.{m}.{y}"),
        "ru" => format!("{d:02}.{m:02}.{y}"),
        "ja" | "zh" => format!("{y}/{m}/{d}"),
        "ko" => format!("{y}. {m}. {d}."),
        _ => format!("{m:02}/{d:02}/{y}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 9, 24).unwrap().and_hms_opt(14, 7, 0).unwrap()
    }

    /// 2025-09-24T17:07:00Z — 14:07 em São Paulo (UTC-3, sem horário de verão desde 2019).
    const INSTANT_MS: i64 = 1_758_733_620_000;

    /// `user.timezone` é texto livre: quatro BYTES não são quatro dígitos (`a` + `日` = 1 + 3), e
    /// fatiar `[..2]` caía no meio do `日` — pânico que derrubava a montagem do prompt de todo turno.
    #[test]
    fn a_non_ascii_four_byte_timezone_is_unknown_not_a_panic() {
        assert_eq!(parse_fixed_offset("+a日"), None);
        assert_eq!(parse_fixed_offset("UTC-é日"), None);
        assert_eq!(user_time_at(INSTANT_MS, "+a日").local, user_time_at(INSTANT_MS, "").local);
    }

    #[test]
    fn format_user_date_time_renders_the_full_pt_br_format() {
        let out = format_user_date_time(sample(), "pt-BR");
        assert_eq!(out, "quarta-feira, 24 de setembro de 2025, 14:07");
    }

    #[test]
    fn format_user_date_renders_the_short_numeric_pt_br_format() {
        assert_eq!(format_user_date(sample(), "pt-BR"), "24/09/2025");
    }

    /// Os 10 locales suportados, mesmo instante (quarta, 24/09/2025, 14:07): nenhum cai no inglês,
    /// que era o que um app em coreano ou russo recebia como "hora atual" no prompt.
    #[test]
    fn every_supported_locale_has_its_own_date_format() {
        for (locale, full, short) in [
            ("en-US", "Wednesday, September 24, 2025, 14:07", "09/24/2025"),
            ("pt-BR", "quarta-feira, 24 de setembro de 2025, 14:07", "24/09/2025"),
            ("es-ES", "miércoles, 24 de septiembre de 2025, 14:07", "24/9/2025"),
            ("fr-FR", "mercredi 24 septembre 2025 à 14:07", "24/09/2025"),
            ("it-IT", "mercoledì 24 settembre 2025 alle ore 14:07", "24/9/2025"),
            ("de-DE", "Mittwoch, 24. September 2025 um 14:07 Uhr", "24.9.2025"),
            ("ru-RU", "среда, 24 сентября 2025 г. в 14:07", "24.09.2025"),
            ("ja-JP", "2025年9月24日水曜日 14:07", "2025/9/24"),
            ("zh-CN", "2025年9月24日星期三 14:07", "2025/9/24"),
            ("ko-KR", "2025년 9월 24일 수요일 14:07", "2025. 9. 24."),
            ("xx-XX", "Wednesday, September 24, 2025, 14:07", "09/24/2025"),
        ] {
            assert_eq!(format_user_date_time(sample(), locale), full, "{locale}");
            assert_eq!(format_user_date(sample(), locale), short, "{locale}");
        }
    }

    #[test]
    fn locale_matching_ignores_case_and_region() {
        assert_eq!(format_user_date(sample(), "PT-br"), "24/09/2025");
        assert_eq!(format_user_date(sample(), "pt"), "24/09/2025");
    }

    /// O campo `user.timezone` decide a hora do prompt: o mesmo instante vira 14:07 em São Paulo e
    /// 02:07 do dia seguinte em Tóquio — o defeito era o prompt usar sempre o fuso do processo.
    #[test]
    fn the_configured_iana_zone_drives_the_civil_time() {
        let sp = user_time_at(INSTANT_MS, "America/Sao_Paulo");
        assert_eq!(sp.local, sample());
        assert_eq!(sp.zone_label, "America/Sao_Paulo (UTC-03:00)");

        let tokyo = user_time_at(INSTANT_MS, "Asia/Tokyo");
        assert_eq!(format_user_date_time(tokyo.local, "pt-BR"), "quinta-feira, 25 de setembro de 2025, 02:07");
    }

    #[test]
    fn fixed_offsets_in_the_forms_people_type_are_accepted() {
        for raw in ["UTC-3", "GMT-03:00", "-03:00", "-0300", " utc-3 "] {
            let now = user_time_at(INSTANT_MS, raw);
            assert_eq!(now.local, sample(), "{raw}");
            assert_eq!(now.zone_label, "UTC-03:00", "{raw}");
        }
        assert_eq!(user_time_at(INSTANT_MS, "UTC+05:30").zone_label, "UTC+05:30");
    }

    #[test]
    fn an_unknown_or_empty_zone_falls_back_to_the_system_zone() {
        let system = user_time_at(INSTANT_MS, "");
        assert_eq!(user_time_at(INSTANT_MS, "Nowhere/Atlantis").local, system.local);
        assert_eq!(parse_fixed_offset("UTC+99"), None);
        assert_eq!(parse_fixed_offset("banana"), None);
    }
}
