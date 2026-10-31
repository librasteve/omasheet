// Copyright (c) 2026 Stephen Roe

//! The named sequences of English: days of the week, months of the year,
//! and a month with its year. A sequence written in them takes its step
//! from its terms, as `"Monday", "Tuesday" ... Inf` does, and the names go
//! round: Sunday is followed by Monday, and December by January.

/// Which run of names a sequence is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cyc {
    /// `Monday` … `Sunday`.
    Day,
    /// `Mon` … `Sun`.
    DayShort,
    /// `January` … `December`.
    Month,
    /// `Jan` … `Dec`.
    MonthShort,
    /// A month with its year, `Jan-2026`, which carries into the year.
    MonthYear { short: bool, sep: char },
}

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

const MONTHS: [&str; 12] = [
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
];

/// How a name was written, so that the next one is written the same way.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Title,
    Lower,
    Upper,
}

impl Case {
    fn of(s: &str) -> Case {
        let letters = || s.chars().filter(|c| c.is_alphabetic());
        if letters().all(char::is_uppercase) && letters().count() > 1 {
            Case::Upper
        } else if letters().all(char::is_lowercase) {
            Case::Lower
        } else {
            Case::Title
        }
    }

    fn apply(self, s: &str) -> String {
        match self {
            Case::Title => s.to_string(),
            Case::Lower => s.to_lowercase(),
            Case::Upper => s.to_uppercase(),
        }
    }
}

fn names(c: Cyc) -> &'static [&'static str] {
    match c {
        Cyc::Day | Cyc::DayShort => &DAYS,
        _ => &MONTHS,
    }
}

/// How many letters a name is written with: all of them, or the first
/// three.
fn short(c: Cyc) -> bool {
    match c {
        Cyc::DayShort | Cyc::MonthShort => true,
        Cyc::MonthYear { short, .. } => short,
        _ => false,
    }
}

fn written(c: Cyc, i: usize, case: Case) -> String {
    let full = names(c)[i];
    let name = if short(c) { &full[..3] } else { full };
    case.apply(name)
}

/// The position of `word` in the run of names, ignoring its case.
fn position(c: Cyc, word: &str) -> Option<usize> {
    let want = if short(c) { 3 } else { word.len() };
    names(c).iter().position(|n| {
        let n = if short(c) { &n[..3] } else { *n };
        n.len() == want && n.eq_ignore_ascii_case(word)
    })
}

/// A month with its year split into its parts: `Jan-2026` is the month,
/// the year, and the character between them.
fn month_year(s: &str) -> Option<(usize, i64, bool, char)> {
    let at = s.char_indices().find(|(_, c)| *c == '-' || *c == ' ')?.0;
    let (word, rest) = s.split_at(at);
    let sep = rest.chars().next()?;
    let year: i64 = rest[sep.len_utf8()..].parse().ok()?;
    let short = word.len() == 3;
    let kind = Cyc::MonthYear { short, sep };
    Some((position(kind, word)?, year, short, sep))
}

/// The whole count a value stands for, which steps by one from each name
/// to the next. Days and months go round, so their count is a position;
/// a month with its year counts on from the year nought.
fn ordinal(c: Cyc, s: &str) -> Option<i64> {
    match c {
        Cyc::MonthYear { .. } => {
            let (m, y, ..) = month_year(s)?;
            Some(y * 12 + m as i64)
        }
        _ => position(c, s).map(|i| i as i64),
    }
}

/// Which run of names a value is written in, if any. A month with its
/// year is looked for first: `Jan-2026` is not the bare month `Jan`.
fn kind_of(s: &str) -> Option<Cyc> {
    if let Some((_, _, short, sep)) = month_year(s) {
        return Some(Cyc::MonthYear { short, sep });
    }
    [Cyc::Day, Cyc::DayShort, Cyc::Month, Cyc::MonthShort]
        .into_iter()
        .find(|&c| position(c, s).is_some())
}

/// The run of names the terms are written in and the step between them,
/// if they are all of one run and evenly spaced. One term alone steps by
/// one.
pub fn detect(seeds: &[&str]) -> Option<(Cyc, i64)> {
    let first = seeds.first()?;
    let kind = kind_of(first)?;
    let round = match kind {
        Cyc::MonthYear { .. } => None,
        c => Some(names(c).len() as i64),
    };
    let mut at = Vec::with_capacity(seeds.len());
    for s in seeds {
        // Every term must be written the same way, so that `Mon, Tuesday`
        // is not taken for a sequence.
        if kind_of(s) != Some(kind) {
            return None;
        }
        at.push(ordinal(kind, s)?);
    }
    if at.len() == 1 {
        return Some((kind, 1));
    }
    // Names that go round are evenly spaced if their steps agree once
    // the wrap at the end of the run is taken off.
    let step = |a: i64, b: i64| match round {
        Some(n) => (b - a).rem_euclid(n),
        None => b - a,
    };
    let first_step = step(at[0], at[1]);
    at.windows(2)
        .all(|w| step(w[0], w[1]) == first_step)
        .then_some((kind, first_step))
}

/// The value `step` places after `value` in its run of names.
pub fn advance(c: Cyc, value: &str, step: i64) -> Option<String> {
    let case = Case::of(value);
    match c {
        Cyc::MonthYear { sep, .. } => {
            let (m, y, ..) = month_year(value)?;
            let total = (y * 12 + m as i64).checked_add(step)?;
            let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as usize);
            Some(format!("{}{sep}{y:04}", written(c, m, case)))
        }
        _ => {
            let n = names(c).len() as i64;
            let at = position(c, value)? as i64;
            Some(written(
                c,
                at.checked_add(step)?.rem_euclid(n) as usize,
                case,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_months_are_told_apart_and_go_round() {
        assert_eq!(detect(&["Monday", "Tuesday"]), Some((Cyc::Day, 1)));
        assert_eq!(detect(&["Mon", "Wed"]), Some((Cyc::DayShort, 2)));
        assert_eq!(detect(&["January", "February"]), Some((Cyc::Month, 1)));
        assert_eq!(detect(&["Jan", "Feb", "Mar"]), Some((Cyc::MonthShort, 1)));
        assert_eq!(advance(Cyc::Day, "Sunday", 1).as_deref(), Some("Monday"));
        assert_eq!(advance(Cyc::MonthShort, "Dec", 1).as_deref(), Some("Jan"));
        // March is both a month and no day, so days never match it.
        assert_eq!(detect(&["Mar", "Apr"]), Some((Cyc::MonthShort, 1)));
    }

    #[test]
    fn a_run_that_wraps_is_still_evenly_spaced() {
        assert_eq!(
            detect(&["Saturday", "Sunday", "Monday"]),
            Some((Cyc::Day, 1))
        );
        assert_eq!(detect(&["Nov", "Jan", "Mar"]), Some((Cyc::MonthShort, 2)));
    }

    #[test]
    fn terms_written_differently_are_not_a_run() {
        assert_eq!(detect(&["Mon", "Tuesday"]), None);
        assert_eq!(detect(&["Monday", "Wednesday", "Thursday"]), None);
        assert_eq!(detect(&["Kettle", "Toaster"]), None);
    }

    #[test]
    fn a_month_with_its_year_carries_into_the_year() {
        let kind = Cyc::MonthYear {
            short: true,
            sep: '-',
        };
        assert_eq!(detect(&["Jan-2026", "Feb-2026"]), Some((kind, 1)));
        assert_eq!(advance(kind, "Dec-2026", 1).as_deref(), Some("Jan-2027"));
        assert_eq!(advance(kind, "Jan-2026", 12).as_deref(), Some("Jan-2027"));
        // Quarters, and the longer way of writing a month.
        assert_eq!(detect(&["Jan-2026", "Apr-2026"]), Some((kind, 3)));
        let long = Cyc::MonthYear {
            short: false,
            sep: ' ',
        };
        assert_eq!(detect(&["January 2026", "February 2026"]), Some((long, 1)));
    }

    #[test]
    fn the_case_of_the_terms_is_kept() {
        assert_eq!(advance(Cyc::Day, "monday", 1).as_deref(), Some("tuesday"));
        assert_eq!(advance(Cyc::DayShort, "MON", 1).as_deref(), Some("TUE"));
    }
}
