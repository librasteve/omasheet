//! Calendar dates as a count of days since 1970-01-01 (proleptic Gregorian),
//! and date-times as a count of seconds since the start of that day. A
//! date-time has no time zone.

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The day number of a date, or `None` if it is not a real calendar date.
pub fn from_ymd(y: i64, m: i64, d: i64) -> Option<i32> {
    if !(1..=12).contains(&m) || d < 1 {
        return None;
    }
    let days = days_from_civil(y, m, d);
    (civil_from_days(days) == (y, m, d)).then_some(days as i32)
}

pub fn format(days: i32) -> String {
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The second number of a time on a day, or `None` if it is not a real time.
pub fn datetime_from(days: i32, h: i64, m: i64, s: i64) -> Option<i64> {
    ((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&s))
        .then(|| days as i64 * 86_400 + h * 3600 + m * 60 + s)
}

/// `2025-01-03T09:30`, with the seconds only when they are not zero.
pub fn format_datetime(secs: i64) -> String {
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (h, m, s) = (rest / 3600, rest / 60 % 60, rest % 60);
    let day = format(days as i32);
    if s == 0 {
        format!("{day}T{h:02}:{m:02}")
    } else {
        format!("{day}T{h:02}:{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(from_ymd(1970, 1, 1), Some(0));
        assert_eq!(format(from_ymd(2025, 1, 1).unwrap()), "2025-01-01");
        assert_eq!(format(from_ymd(2024, 2, 29).unwrap()), "2024-02-29");
        assert_eq!(from_ymd(2025, 2, 29), None);
        assert_eq!(from_ymd(2025, 13, 1), None);
    }

    #[test]
    fn datetimes_round_trip() {
        let day = from_ymd(2025, 1, 3).unwrap();
        assert_eq!(datetime_from(0, 0, 0, 0), Some(0));
        assert_eq!(
            format_datetime(datetime_from(day, 9, 30, 0).unwrap()),
            "2025-01-03T09:30"
        );
        assert_eq!(
            format_datetime(datetime_from(day, 23, 59, 59).unwrap()),
            "2025-01-03T23:59:59"
        );
        let before_epoch = datetime_from(from_ymd(1969, 12, 31).unwrap(), 18, 0, 0).unwrap();
        assert_eq!(format_datetime(before_epoch), "1969-12-31T18:00");
        assert_eq!(datetime_from(day, 24, 0, 0), None);
        assert_eq!(datetime_from(day, 9, 60, 0), None);
    }
}
