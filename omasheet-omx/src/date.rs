// Copyright (c) 2026 Stephen Roe

//! Calendar dates as a count of days since 1970-01-01 (proleptic Gregorian),
//! times of day as a count of seconds since midnight, and date-times as a
//! count of seconds since the start of 1970-01-01. None has a time zone.
//!
//! A sheet always writes them the ISO way (`2025-01-31`, `09:30`,
//! `2025-01-31T09:30`). A [`Style`] is how they are shown to, and typed by, a
//! person: `31/01/2025` in Britain, `01/31/2025` in the United States.

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

/// The second number of a time of day, or `None` if it is not a real time.
pub fn time_from(h: i64, m: i64, s: i64) -> Option<i32> {
    datetime_from(0, h, m, s).map(|t| t as i32)
}

/// `09:30`, with the seconds only when they are not zero.
pub fn format_time(secs: i32) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if s == 0 {
        format!("{h:02}:{m:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}")
    }
}

/// The day and the time of day of a date-time.
pub fn split_datetime(secs: i64) -> (i32, i32) {
    (
        secs.div_euclid(86_400) as i32,
        secs.rem_euclid(86_400) as i32,
    )
}

/// `2025-01-03T09:30`, with the seconds only when they are not zero.
pub fn format_datetime(secs: i64) -> String {
    let (days, time) = split_datetime(secs);
    format!("{}T{}", format(days), format_time(time))
}

/// The day of the week, from 1 for Monday to 7 for Sunday.
pub fn weekday(days: i32) -> i64 {
    (days as i64 + 3).rem_euclid(7) + 1
}

/// The order of the parts of a date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Ymd,
    Dmy,
    Mdy,
}

/// How dates and times are shown and typed. The year always has four digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub order: Order,
    pub sep: char,
    /// A twelve-hour clock with AM and PM.
    pub hour12: bool,
    /// The ISO forms exactly, with `T` between a date and its time.
    iso: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style::ISO
    }
}

impl Style {
    /// The way a sheet writes them: `2025-01-31`, `17:05`, `2025-01-31T17:05`.
    pub const ISO: Style = Style {
        order: Order::Ymd,
        sep: '-',
        hour12: false,
        iso: true,
    };

    pub fn new(order: Order, sep: char, hour12: bool) -> Style {
        Style {
            order,
            sep,
            hour12,
            iso: false,
        }
    }

    pub fn is_iso(&self) -> bool {
        self.iso
    }

    /// The style of a locale, from its `strftime` date and time formats
    /// (`%d/%m/%y` and `%T` in Britain). Only the order of the date, its
    /// separator and the kind of clock are taken; `None` if the date format
    /// does not have a day, a month and a year.
    pub fn from_strftime(d_fmt: &str, t_fmt: &str) -> Option<Style> {
        // What each conversion stands for, with flags and modifiers dropped.
        let expand = |fmt: &str| {
            let mut out = String::new();
            let mut chars = fmt.chars();
            while let Some(c) = chars.next() {
                if c != '%' {
                    out.push(c);
                    continue;
                }
                let conv = chars.find(|c| !"-_0^#EO".contains(*c));
                match conv {
                    Some('D') => out.push_str("%m/%d/%Y"),
                    Some('F') => out.push_str("%Y-%m-%d"),
                    Some('e') => out.push_str("%d"),
                    Some('y') => out.push_str("%Y"),
                    Some('r') => out.push_str("%I:%M:%S %p"),
                    Some('l') => out.push_str("%I"),
                    Some(other) => {
                        out.push('%');
                        out.push(other);
                    }
                    None => {}
                }
            }
            out
        };
        let date = expand(d_fmt);
        let at = |conv: &str| date.find(conv);
        let (d, m, y) = (at("%d")?, at("%m")?, at("%Y")?);
        let order = if y < d && y < m {
            Order::Ymd
        } else if d < m {
            Order::Dmy
        } else {
            Order::Mdy
        };
        let first = d.min(m).min(y) + 2;
        let sep = date[first..]
            .chars()
            .next()
            .filter(|c| !c.is_alphanumeric() && *c != '%' && !c.is_whitespace())?;
        let time = expand(t_fmt);
        Some(Style::new(
            order,
            sep,
            time.contains("%I") || time.contains("%p"),
        ))
    }

    /// The date format in words, such as `DD/MM/YYYY`.
    pub fn date_pattern(&self) -> String {
        let parts = match self.order {
            Order::Ymd => ["YYYY", "MM", "DD"],
            Order::Dmy => ["DD", "MM", "YYYY"],
            Order::Mdy => ["MM", "DD", "YYYY"],
        };
        parts.join(&self.sep.to_string())
    }

    pub fn date(&self, days: i32) -> String {
        let (y, m, d) = civil_from_days(days as i64);
        let s = self.sep;
        match self.order {
            Order::Ymd => format!("{y:04}{s}{m:02}{s}{d:02}"),
            Order::Dmy => format!("{d:02}{s}{m:02}{s}{y:04}"),
            Order::Mdy => format!("{m:02}{s}{d:02}{s}{y:04}"),
        }
    }

    pub fn time(&self, secs: i32) -> String {
        if !self.hour12 {
            return format_time(secs);
        }
        let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
        let (hour, half) = ((h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" });
        if s == 0 {
            format!("{hour}:{m:02} {half}")
        } else {
            format!("{hour}:{m:02}:{s:02} {half}")
        }
    }

    pub fn datetime(&self, secs: i64) -> String {
        if self.iso {
            return format_datetime(secs);
        }
        let (days, time) = split_datetime(secs);
        format!("{} {}", self.date(days), self.time(time))
    }

    /// Read a date typed in this style. Any of `/ - .` separates the parts,
    /// and a two-digit year is taken to be within 1969 to 2068.
    pub fn parse_date(&self, text: &str) -> Option<i32> {
        let parts: Vec<&str> = text.trim().split(['/', '-', '.']).collect();
        let [a, b, c] = parts.as_slice() else {
            return None;
        };
        let num = |p: &str, max: usize| {
            (!p.is_empty() && p.len() <= max && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse::<i64>().ok())
                .flatten()
        };
        let year = |p: &str| {
            let y = num(p, 4)?;
            match p.len() {
                4 => Some(y),
                2 => Some(if y < 69 { 2000 + y } else { 1900 + y }),
                _ => None,
            }
        };
        // A four-digit year first is a date written the ISO way.
        if a.len() == 4 {
            return from_ymd(num(a, 4)?, num(b, 2)?, num(c, 2)?);
        }
        match self.order {
            Order::Ymd => from_ymd(year(a)?, num(b, 2)?, num(c, 2)?),
            Order::Dmy => from_ymd(year(c)?, num(b, 2)?, num(a, 2)?),
            Order::Mdy => from_ymd(year(c)?, num(a, 2)?, num(b, 2)?),
        }
    }

    /// Read a time: `9:30`, `09:30:15`, `9:30 pm`.
    pub fn parse_time(&self, text: &str) -> Option<i32> {
        let lower = text.trim().to_ascii_lowercase();
        let (clock, half) = match lower.strip_suffix("am") {
            Some(rest) => (rest, Some(false)),
            None => match lower.strip_suffix("pm") {
                Some(rest) => (rest, Some(true)),
                None => (lower.as_str(), None),
            },
        };
        let parts: Vec<&str> = clock.trim().split(':').collect();
        let num = |p: &str| {
            (matches!(p.len(), 1 | 2) && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse::<i64>().ok())
                .flatten()
        };
        let (h, m, s) = match parts.as_slice() {
            [h, m] if m.len() == 2 => (num(h)?, num(m)?, 0),
            [h, m, s] if m.len() == 2 && s.len() == 2 => (num(h)?, num(m)?, num(s)?),
            _ => return None,
        };
        let h = match half {
            None => h,
            Some(_) if !(1..=12).contains(&h) => return None,
            Some(pm) => h % 12 + if pm { 12 } else { 0 },
        };
        time_from(h, m, s)
    }

    /// Read a date and a time, separated by a space or a `T`.
    pub fn parse_datetime(&self, text: &str) -> Option<i64> {
        let text = text.trim();
        let cut = text.find([' ', 'T', 't'])?;
        let days = self.parse_date(&text[..cut])?;
        let time = self.parse_time(&text[cut + 1..])?;
        Some(days as i64 * 86_400 + time as i64)
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

    #[test]
    fn times_and_weekdays() {
        assert_eq!(format_time(time_from(9, 30, 0).unwrap()), "09:30");
        assert_eq!(format_time(time_from(23, 59, 59).unwrap()), "23:59:59");
        assert_eq!(time_from(24, 0, 0), None);
        // 1970-01-01 was a Thursday.
        assert_eq!(weekday(0), 4);
        assert_eq!(weekday(from_ymd(2026, 10, 8).unwrap()), 4);
        assert_eq!(weekday(from_ymd(1969, 12, 28).unwrap()), 7);
    }

    #[test]
    fn styles_come_from_the_locale() {
        let gb = Style::from_strftime("%d/%m/%y", "%T").unwrap();
        assert_eq!(gb, Style::new(Order::Dmy, '/', false));
        let us = Style::from_strftime("%m/%d/%Y", "%r").unwrap();
        assert_eq!(us, Style::new(Order::Mdy, '/', true));
        let de = Style::from_strftime("%d.%m.%Y", "%T").unwrap();
        assert_eq!(de, Style::new(Order::Dmy, '.', false));
        let se = Style::from_strftime("%Y-%m-%d", "%H:%M:%S").unwrap();
        assert_eq!(se, Style::new(Order::Ymd, '-', false));
        assert_eq!(Style::from_strftime("%-m/%-d/%Y", "%I:%M %p"), Some(us));
        assert_eq!(Style::from_strftime("%D", "%T").unwrap().order, Order::Mdy);
        assert_eq!(Style::from_strftime("%a %b %e", "%T"), None);
        assert_eq!(gb.date_pattern(), "DD/MM/YYYY");
    }

    #[test]
    fn styles_show_and_read() {
        let gb = Style::new(Order::Dmy, '/', false);
        let us = Style::new(Order::Mdy, '/', true);
        let day = from_ymd(2026, 10, 8).unwrap();
        let evening = datetime_from(day, 17, 47, 0).unwrap();
        assert_eq!(gb.date(day), "08/10/2026");
        assert_eq!(us.date(day), "10/08/2026");
        assert_eq!(Style::ISO.date(day), "2026-10-08");
        assert_eq!(gb.datetime(evening), "08/10/2026 17:47");
        assert_eq!(us.datetime(evening), "10/08/2026 5:47 PM");
        assert_eq!(Style::ISO.datetime(evening), "2026-10-08T17:47");
        assert_eq!(us.time(0), "12:00 AM");
        assert_eq!(us.time(12 * 3600 + 5), "12:00:05 PM");

        assert_eq!(gb.parse_date("8/10/2026"), Some(day));
        assert_eq!(us.parse_date("10/8/2026"), Some(day));
        assert_eq!(gb.parse_date("08.10.26"), Some(day));
        assert_eq!(gb.parse_date("2026-10-08"), Some(day));
        assert_eq!(gb.parse_date("31/02/2026"), None);
        assert_eq!(gb.parse_date("10/2026"), None);
        assert_eq!(gb.parse_date("1/2/345"), None);
        assert_eq!(gb.parse_time("9:30"), time_from(9, 30, 0));
        assert_eq!(gb.parse_time("5:47 pm"), time_from(17, 47, 0));
        assert_eq!(gb.parse_time("12:00AM"), Some(0));
        assert_eq!(gb.parse_time("13:00 pm"), None);
        assert_eq!(gb.parse_time("9:3"), None);
        assert_eq!(gb.parse_datetime("8/10/2026 17:47"), Some(evening));
        assert_eq!(us.parse_datetime("10/8/2026 5:47 PM"), Some(evening));
        assert_eq!(gb.parse_datetime("2026-10-08T17:47"), Some(evening));
        assert_eq!(gb.parse_datetime("8/10/2026"), None);
    }
}
