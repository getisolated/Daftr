//! Purge des notes supprimées : passé ce délai, leur contenu est vidé et seule la tombstone
//! (uuid, date de suppression) reste.

/// Délai avant la purge d'une note supprimée, en jours.
pub const PURGE_AFTER_DAYS: i64 = 30;

/// La date `now` (format SQLite `YYYY-MM-DD HH:MM:SS`, UTC) reculée de [`PURGE_AFTER_DAYS`]
/// jours, même format ; `None` si `now` est mal formée.
///
/// ```
/// assert_eq!(
///     domain::sync::purge_cutoff("2026-03-01 08:30:00").as_deref(),
///     Some("2026-01-30 08:30:00")
/// );
/// ```
pub fn purge_cutoff(now: &str) -> Option<String> {
    let (date, time) = split_timestamp(now)?;
    let (year, month, day) = civil_from_days(days_from_civil(date)? - PURGE_AFTER_DAYS);

    Some(format!("{year:04}-{month:02}-{day:02} {time}"))
}

/// `text` est un horodatage SQLite valide (`YYYY-MM-DD HH:MM:SS`), le format des dates
/// des notes et des dossiers.
///
/// ```
/// assert!(domain::sync::is_timestamp("2026-02-28 23:59:59"));
/// assert!(!domain::sync::is_timestamp("2026-02-29 00:00:00"));
/// ```
pub fn is_timestamp(text: &str) -> bool {
    split_timestamp(text).is_some()
}

/// Une note supprimée le `deleted_at` se purge à la date `now` : au moins
/// [`PURGE_AFTER_DAYS`] jours se sont écoulés. Dans le doute (date mal formée), on garde.
///
/// ```
/// use domain::sync::is_purgeable;
///
/// assert!(is_purgeable("2026-01-01 10:00:00", "2026-01-31 10:00:00"));
/// assert!(!is_purgeable("2026-01-01 10:00:01", "2026-01-31 10:00:00"));
/// assert!(!is_purgeable("hier", "2026-01-31 10:00:00"));
/// ```
pub fn is_purgeable(deleted_at: &str, now: &str) -> bool {
    let Some(cutoff) = purge_cutoff(now) else {
        return false;
    };
    // Le format est triable : la comparaison de textes suit l'ordre chronologique
    is_timestamp(deleted_at) && deleted_at <= cutoff.as_str()
}

/// L'horodatage SQLite (`YYYY-MM-DD HH:MM:SS`, UTC) d'un instant donné en secondes depuis le
/// 1970-01-01 : le format de `datetime('now')`, pour les dates écrites hors de SQLite.
///
/// ```
/// assert_eq!(domain::sync::timestamp_from_unix(0), "1970-01-01 00:00:00");
/// assert_eq!(domain::sync::timestamp_from_unix(1_772_353_800), "2026-03-01 08:30:00");
/// ```
pub fn timestamp_from_unix(seconds: i64) -> String {
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let second_of_day = seconds.rem_euclid(86_400);
    let (hour, minute, second) = (
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60,
    );

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// La date `(année, mois, jour)` et l'heure `HH:MM:SS` d'un horodatage SQLite valide.
fn split_timestamp(timestamp: &str) -> Option<((i64, u32, u32), &str)> {
    let (date, time) = timestamp.split_once(' ')?;
    let time_parts = numbers(time, ':', [2, 2, 2])?;
    let date_parts = numbers(date, '-', [4, 2, 2])?;

    let valid_time = time_parts[0] < 24 && time_parts[1] < 60 && time_parts[2] < 60;
    let month = u32::try_from(date_parts[1]).ok()?;
    let day = u32::try_from(date_parts[2]).ok()?;
    let valid_date =
        (1..=12).contains(&month) && day >= 1 && day <= days_in_month(date_parts[0], month);

    (valid_time && valid_date).then_some(((date_parts[0], month, day), time))
}

/// Les trois nombres de `text` séparés par `separator`, chacun du nombre de chiffres attendu.
fn numbers(text: &str, separator: char, widths: [usize; 3]) -> Option<[i64; 3]> {
    let parts: Vec<&str> = text.split(separator).collect();
    if parts.len() != 3 {
        return None;
    }

    let mut values = [0_i64; 3];
    for ((value, part), width) in values.iter_mut().zip(&parts).zip(widths) {
        if part.len() != width || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *value = part.parse().ok()?;
    }
    Some(values)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Jours depuis le 1970-01-01 (algorithme de H. Hinnant, calendrier grégorien proleptique).
fn days_from_civil((year, month, day): (i64, u32, u32)) -> Option<i64> {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted_month = i64::from((month + 9) % 12);
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era.checked_mul(146_097)?.checked_add(day_of_era - 719_468)
}

/// Date `(année, mois, jour)` d'un nombre de jours depuis le 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    // Mois et jour sont bornés par construction (1..=12, 1..=31)
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosses_months_years_and_leap_days() {
        assert_eq!(
            purge_cutoff("2024-03-01 00:00:00").as_deref(),
            Some("2024-01-31 00:00:00")
        );
        assert_eq!(
            purge_cutoff("2026-01-15 23:59:59").as_deref(),
            Some("2025-12-16 23:59:59")
        );
        assert_eq!(
            purge_cutoff("2024-03-30 12:00:00").as_deref(),
            Some("2024-02-29 12:00:00")
        );
    }

    #[test]
    fn round_trips_every_day_of_a_long_period() {
        for days in -800_000..800_000 {
            let civil = civil_from_days(days);
            assert_eq!(days_from_civil(civil), Some(days), "{civil:?}");
        }
    }

    #[test]
    fn unix_instants_become_valid_timestamps() {
        assert_eq!(timestamp_from_unix(-1), "1969-12-31 23:59:59");
        assert_eq!(timestamp_from_unix(951_782_400), "2000-02-29 00:00:00");
        for seconds in (0..4_000_000_000_i64).step_by(7_919_993) {
            assert!(is_timestamp(&timestamp_from_unix(seconds)), "{seconds}");
        }
    }

    #[test]
    fn refuses_malformed_timestamps() {
        for text in [
            "",
            "2026-01-01",
            "2026-01-01T00:00:00",
            "2026-13-01 00:00:00",
            "2026-02-29 00:00:00",
            "2026-01-01 24:00:00",
            "2026-1-01 00:00:00",
            "+026-01-01 00:00:00",
        ] {
            assert_eq!(purge_cutoff(text), None, "{text}");
        }
    }

    #[test]
    fn never_purges_on_a_malformed_deletion_date() {
        assert!(!is_purgeable("2020-01-01", "2026-01-01 00:00:00"));
        assert!(!is_purgeable("2020-01-01 00:00:00", "maintenant"));
    }
}
