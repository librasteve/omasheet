// Copyright (c) 2026 Stephen Roe

//! A date or time shown by its column's format, `%d %b %Y`, as glibc's
//! `strftime` shows it, with the names of days and months in the locale's
//! language.

use omasheet_omx::date::{civil_from_days, days_from_civil, hms, split_datetime, weekday};
use omasheet_omx::datefmt::{DateFormat, Item, Pad};

/// What a locale calls the days, the months and the halves of the day, and
/// how it writes `%c`, `%x`, `%X` and `%r`. The default is the `C` locale's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Names {
    /// From Sunday, as C counts them.
    pub days: [String; 7],
    pub short_days: [String; 7],
    pub months: [String; 12],
    pub short_months: [String; 12],
    /// Before and after noon: `AM`, `PM`.
    pub halves: [String; 2],
    /// `%c`, `%x`, `%X` and `%r`, as `strftime` formats.
    pub date_time: String,
    pub date: String,
    pub time: String,
    pub time12: String,
}

impl Default for Names {
    fn default() -> Self {
        let all = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        let days = all(&[
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ]);
        let months = all(&[
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]);
        let short = |names: &[String]| names.iter().map(|n| n[..3].to_string()).collect::<Vec<_>>();
        Names {
            short_days: short(&days).try_into().unwrap(),
            short_months: short(&months).try_into().unwrap(),
            days: days.try_into().unwrap(),
            months: months.try_into().unwrap(),
            halves: ["AM".into(), "PM".into()],
            date_time: "%a %b %e %H:%M:%S %Y".into(),
            date: "%m/%d/%y".into(),
            time: "%H:%M:%S".into(),
            time12: "%I:%M:%S %p".into(),
        }
    }
}

/// Where a date-time is: the instant, in seconds UTC, how far its zone's
/// clocks are ahead of UTC, and what the zone is called then, `BST`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instant {
    pub utc: i64,
    pub offset: i64,
    pub abbreviation: String,
}

/// What a format shows: a day, a time of day in nanoseconds, or both, and
/// for a date-time where it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moment {
    pub days: Option<i32>,
    pub nanos: Option<i64>,
    pub instant: Option<Instant>,
}

impl Moment {
    pub fn date(days: i32) -> Moment {
        Moment {
            days: Some(days),
            ..Moment::default()
        }
    }

    pub fn time(nanos: i64) -> Moment {
        Moment {
            nanos: Some(nanos),
            ..Moment::default()
        }
    }

    /// A date-time, held in microseconds on the clocks of its zone.
    pub fn datetime(micros: i64, instant: Option<Instant>) -> Moment {
        let (days, nanos) = split_datetime(micros);
        Moment {
            days: Some(days),
            nanos: Some(nanos),
            instant,
        }
    }
}

/// `m` shown by `f` with the names of `names`, or `None` where the format
/// asks for what `m` does not have: a time of day of a date.
pub fn format_date(m: &Moment, f: &DateFormat, names: &Names) -> Option<String> {
    let mut out = String::new();
    for item in &f.items {
        match item {
            Item::Text(text) => out.push_str(text),
            Item::Conv {
                pad,
                upper,
                swap,
                width,
                conv,
                digits,
            } => {
                let part = conversion(m, *conv, *digits, *upper, *swap, names)?;
                out.push_str(&padded(part, *pad, *width));
            }
        }
    }
    Some(out)
}

/// What one conversion shows, before it is padded to a width.
enum Part {
    /// A number, at least `digits` digits, padded with zeros or, for `%e`,
    /// `%k` and `%l`, spaces.
    Number {
        n: i64,
        digits: usize,
        spaces: bool,
    },
    Text(String),
}

fn conversion(
    m: &Moment,
    conv: char,
    digits: Option<usize>,
    upper: bool,
    swap: bool,
    names: &Names,
) -> Option<Part> {
    let number = |n: i64, digits: usize| Part::Number {
        n,
        digits,
        spaces: false,
    };
    let spaced = |n: i64| Part::Number {
        n,
        digits: 2,
        spaces: true,
    };
    // A name, in capitals for `^` or `#`, as glibc has it.
    let name = |text: &str| {
        Part::Text(if upper || swap {
            text.to_uppercase()
        } else {
            text.into()
        })
    };
    // Several conversions as one, `%F`, in the format the locale or C has.
    let nested = |text: &str| {
        let f = DateFormat::parse(text).ok()?;
        let shown = format_date(m, &f, names)?;
        Some(Part::Text(if upper { shown.to_uppercase() } else { shown }))
    };
    let date = || {
        let days = m.days?;
        let (y, mo, d) = civil_from_days(days as i64);
        Some((days, y, mo, d))
    };
    let time = || m.nanos.map(hms);
    Some(match conv {
        'a' => name(&names.short_days[weekday(date()?.0) as usize % 7]),
        'A' => name(&names.days[weekday(date()?.0) as usize % 7]),
        'b' | 'h' => name(&names.short_months[date()?.2 as usize - 1]),
        'B' => name(&names.months[date()?.2 as usize - 1]),
        'C' => number(date()?.1.div_euclid(100), 1),
        'd' => number(date()?.3, 2),
        'e' => spaced(date()?.3),
        'D' => nested("%m/%d/%y")?,
        'F' => nested("%Y-%m-%d")?,
        'g' => number(iso_week(date()?.0).0.rem_euclid(100), 2),
        'G' => number(iso_week(date()?.0).0, 1),
        'V' => number(iso_week(date()?.0).1, 2),
        'j' => {
            let (days, y, ..) = date()?;
            number(days as i64 - days_from_civil(y, 1, 1) + 1, 3)
        }
        'm' => number(date()?.2, 2),
        'u' => number(weekday(date()?.0), 1),
        'w' => number(weekday(date()?.0) % 7, 1),
        'U' | 'W' => {
            let (days, y, ..) = date()?;
            let yday = days as i64 - days_from_civil(y, 1, 1);
            // Days since the first day of the week, Sunday or Monday.
            let into = match conv {
                'U' => weekday(days) % 7,
                _ => weekday(days) - 1,
            };
            number((yday + 7 - into) / 7, 2)
        }
        'x' => nested(&names.date)?,
        'y' => number(date()?.1.rem_euclid(100), 2),
        'Y' => number(date()?.1, 1),
        'H' => number(time()?.0, 2),
        'I' => number((time()?.0 + 11) % 12 + 1, 2),
        'k' => spaced(time()?.0),
        'l' => spaced((time()?.0 + 11) % 12 + 1),
        'M' => number(time()?.1, 2),
        'S' => number(time()?.2, 2),
        'p' | 'P' => {
            let half = &names.halves[usize::from(time()?.0 >= 12)];
            // `#` asks `%p` for small letters, as `%P` has them.
            Part::Text(match (conv, swap, upper) {
                ('P', ..) | ('p', true, _) => half.to_lowercase(),
                ('p', _, true) => half.to_uppercase(),
                _ => half.clone(),
            })
        }
        'r' => nested(&names.time12)?,
        'R' => nested("%H:%M")?,
        'T' => nested("%H:%M:%S")?,
        'X' => nested(&names.time)?,
        'f' => {
            let frac = format!("{:09}", time()?.3);
            Part::Text(match digits {
                Some(n) => frac[..n].to_string(),
                None => match frac.trim_end_matches('0') {
                    "" => "0".to_string(),
                    held => held.to_string(),
                },
            })
        }
        'c' => nested(&names.date_time)?,
        's' => number(m.instant.as_ref()?.utc, 1),
        'z' => {
            let offset = m.instant.as_ref()?.offset;
            let (sign, abs) = (if offset < 0 { '-' } else { '+' }, offset.abs());
            Part::Text(format!("{sign}{:02}{:02}", abs / 3600, abs / 60 % 60))
        }
        'Z' => {
            let abbreviation = &m.instant.as_ref()?.abbreviation;
            // `#` asks for the zone's name in small letters, as glibc has it.
            Part::Text(match (swap, upper) {
                (true, _) => abbreviation.to_lowercase(),
                (_, true) => abbreviation.to_uppercase(),
                _ => abbreviation.clone(),
            })
        }
        'n' => Part::Text("\n".into()),
        't' => Part::Text("\t".into()),
        _ => return None,
    })
}

/// The ISO 8601 year and week of a day: the week is the one its Thursday
/// is in, from Monday, and the year is that Thursday's.
fn iso_week(days: i32) -> (i64, i64) {
    let thursday = days as i64 - (weekday(days) - 1) + 3;
    let (y, ..) = civil_from_days(thursday);
    (y, (thursday - days_from_civil(y, 1, 1)) / 7 + 1)
}

/// A part padded to `width`, or to its own width where there is none, as
/// glibc pads: a number with zeros, or spaces for `%e`, `%k`, `%l`, and
/// text with spaces; `_` pads with spaces, `0` with zeros, and `-` leaves a
/// number unpadded but for a width it is given, which it pads with spaces.
fn padded(part: Part, pad: Option<Pad>, width: Option<usize>) -> String {
    match part {
        Part::Number { n, digits, spaces } => {
            let body = n.unsigned_abs().to_string();
            let sign = if n < 0 { "-" } else { "" };
            let fill = match pad {
                Some(Pad::None) if width.is_none() => return format!("{sign}{body}"),
                Some(Pad::None | Pad::Space) => ' ',
                Some(Pad::Zero) => '0',
                None if spaces => ' ',
                None => '0',
            };
            let width = width.unwrap_or(digits + sign.len());
            let short = width.saturating_sub(body.len() + sign.len());
            let fill = fill.to_string().repeat(short);
            match fill.starts_with('0') {
                true => format!("{sign}{fill}{body}"),
                false => format!("{fill}{sign}{body}"),
            }
        }
        Part::Text(text) => {
            let short = width.unwrap_or(0).saturating_sub(text.chars().count());
            let fill = match pad {
                Some(Pad::Zero) => "0",
                _ => " ",
            };
            format!("{}{text}", fill.repeat(short))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omasheet_omx::date::{datetime_from, from_ymd, time_from};

    fn day(y: i64, m: i64, d: i64) -> i32 {
        from_ymd(y, m, d).unwrap()
    }

    fn shown(m: &Moment, text: &str) -> String {
        let f = DateFormat::parse(text).unwrap();
        format_date(m, &f, &Names::default()).unwrap_or_else(|| panic!("{text}"))
    }

    #[test]
    fn shows_the_scenarios() {
        assert_eq!(
            shown(&Moment::date(day(2025, 1, 31)), "%a %d %b %Y"),
            "Fri 31 Jan 2025"
        );
        assert_eq!(
            shown(&Moment::date(day(2025, 1, 5)), "%e %B %Y"),
            " 5 January 2025"
        );
        let t = time_from(17, 5, 0).unwrap();
        assert_eq!(shown(&Moment::time(t), "%I:%M %p"), "05:05 PM");
        let dt = datetime_from(day(2025, 1, 31), 9, 30, 15).unwrap();
        assert_eq!(
            shown(&Moment::datetime(dt, None), "%d/%m %H:%M"),
            "31/01 09:30"
        );
    }

    #[test]
    fn pads_as_the_flags_say() {
        let m = Moment::date(day(2025, 3, 5));
        assert_eq!(shown(&m, "%-d/%-m"), "5/3");
        assert_eq!(shown(&m, "%_d|%0e|%-e"), " 5|05|5");
        assert_eq!(shown(&m, "%6Y|%_6Y|%-6Y|%-Y"), "002025|  2025|  2025|2025");
        assert_eq!(shown(&m, "%^a %#B %^b"), "WED MARCH MAR");
        assert_eq!(shown(&m, "%10A|%-10A"), " Wednesday| Wednesday");
        assert_eq!(shown(&m, "%%%]"), "%]");
    }

    #[test]
    fn shows_fractions() {
        let t = time_from(9, 30, 15).unwrap() + 123_400_000;
        assert_eq!(shown(&Moment::time(t), "%T.%f"), "09:30:15.1234");
        assert_eq!(
            shown(&Moment::time(t), "%3f|%6f|%9f"),
            "123|123400|123400000"
        );
        assert_eq!(shown(&Moment::time(t - 123_400_000), "%f"), "0");
    }

    #[test]
    fn shows_where_a_datetime_is() {
        let dt = datetime_from(day(2025, 7, 1), 12, 0, 0).unwrap();
        let instant = Instant {
            utc: 1_751_367_600,
            offset: 3600,
            abbreviation: "BST".into(),
        };
        let m = Moment::datetime(dt, Some(instant));
        assert_eq!(
            shown(&m, "%H:%M %z %Z %#Z %s"),
            "12:00 +0100 BST bst 1751367600"
        );
        let f = DateFormat::parse("%z").unwrap();
        assert_eq!(
            format_date(&Moment::datetime(dt, None), &f, &Names::default()),
            None
        );
        let f = DateFormat::parse("%H").unwrap();
        assert_eq!(
            format_date(&Moment::date(day(2025, 1, 1)), &f, &Names::default()),
            None
        );
    }

    /// Every conversion agrees with glibc's `strftime` in the `C` locale, on
    /// days that turn the years and weeks about.
    #[cfg(target_os = "linux")]
    #[test]
    fn agrees_with_c() {
        let c = |days: i32, nanos: i64, text: &str| -> String {
            let (y, m, d) = civil_from_days(days as i64);
            let (h, min, s, _) = hms(nanos);
            let yday = days as i64 - days_from_civil(y, 1, 1);
            // SAFETY: a zeroed `tm` is valid, and the locale is made and
            // freed here; the buffer is large enough for any test format.
            unsafe {
                let mut tm: libc::tm = std::mem::zeroed();
                tm.tm_year = (y - 1900) as i32;
                tm.tm_mon = (m - 1) as i32;
                tm.tm_mday = d as i32;
                tm.tm_hour = h as i32;
                tm.tm_min = min as i32;
                tm.tm_sec = s as i32;
                tm.tm_wday = (weekday(days) % 7) as i32;
                tm.tm_yday = yday as i32;
                let locale =
                    libc::newlocale(libc::LC_ALL_MASK, c"C".as_ptr(), std::ptr::null_mut());
                let format = std::ffi::CString::new(text).unwrap();
                let mut buf = [0u8; 256];
                let n = libc::strftime_l(
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    format.as_ptr(),
                    &tm,
                    locale,
                );
                libc::freelocale(locale);
                String::from_utf8(buf[..n].to_vec()).unwrap()
            }
        };
        let formats = [
            "%a %A %b %h %B %C %d %D %e %F",
            "%g %G %j %m %u %U %V %w %W %x %y %Y",
            "%H %I %k %l %M %p %P %r %R %S %T %X %c %n%t%%",
            "%-d %_d %0e %-e %^a %#A %^B %#b %10A %-10B %_10Y %08Y %5m %_3H %-I %#p %^P",
            "%^c %^x %Ey %Od %EY %OH %-j %_j %3d %-6Y %-5m %_-4d %08A",
        ];
        let days = [
            day(2025, 1, 31),
            day(2024, 12, 30),
            day(2021, 1, 1),
            day(2020, 12, 31),
            day(2027, 1, 3),
            day(2026, 12, 28),
            day(2000, 2, 29),
            day(1970, 1, 1),
            day(1999, 1, 1),
            day(205, 6, 7),
            day(9999, 12, 31),
        ];
        let times = [
            0,
            time_from(0, 30, 5).unwrap(),
            time_from(12, 0, 0).unwrap(),
            time_from(23, 59, 59).unwrap(),
            time_from(9, 5, 7).unwrap(),
        ];
        for (k, &d) in days.iter().enumerate() {
            let t = times[k % times.len()];
            let m = Moment::datetime(d as i64 * omasheet_omx::date::DAY_MICROS + t / 1000, None);
            for f in formats {
                assert_eq!(
                    shown(&m, f),
                    c(d, t, f),
                    "{f} on {:?}",
                    civil_from_days(d as i64)
                );
            }
        }
    }
}
