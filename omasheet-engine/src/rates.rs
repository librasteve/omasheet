// Copyright (c) 2026 Stephen Roe

//! Exchange rates: where they come from, the copy kept for when there is no
//! network, and what date they are as of.
//!
//! The rates are [currency-api]'s: one file, `usd.json`, of how many of each
//! currency (fiat and crypto, in lower case) make a US dollar, with the date
//! they are for. They change about once a day, and the date is all the
//! source says, so a cached copy also keeps when it was fetched.
//!
//! A registry always has rates: the snapshot compiled into `siquant` is the
//! baseline, a cached copy newer than it replaces that at start-up, and
//! [`update`] fetches the latest. Evaluating a file never fetches by itself,
//! so what a file shows depends only on the rates in use, which [`status`]
//! names.
//!
//! [currency-api]: https://latest.currency-api.pages.dev/v1/currencies/usd.json

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};
use siquant::Registry;

/// Where the latest rates are, and a mirror for when that is down. A dated
/// copy for the history, `@2025-01-31` for `@latest`, is on the mirror.
const SOURCES: [&str; 2] = [
    "https://latest.currency-api.pages.dev/v1/currencies/usd.json",
    "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json",
];

/// Where the rates in use came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The snapshot compiled into the program.
    Bundled,
    /// The copy kept from an earlier fetch.
    Cache,
    /// Fetched in this run.
    Live,
}

impl Source {
    /// A word for it, as `omasheet rates` says it.
    pub fn name(self) -> &'static str {
        match self {
            Source::Bundled => "bundled",
            Source::Cache => "cached",
            Source::Live => "live",
        }
    }
}

/// The rates in use: what they are as of, and when they were fetched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// The date the source says they are for, `2026-11-18`.
    pub as_of: String,
    /// When they were fetched, in seconds since 1970; none for the bundled
    /// ones.
    pub fetched_at: Option<i64>,
    pub source: Source,
}

impl Status {
    /// `rates 2026-11-18 (today)`, for a status bar: or `(cached)` and the like
    /// for rates of an earlier day.
    pub fn short(&self) -> String {
        self.short_on(&jiff::Zoned::now().date().to_string())
    }

    /// [`Status::short`] as of `today`: rates for that day say so, in place
    /// of where they came from, which is said for older ones.
    fn short_on(&self, today: &str) -> String {
        let how = if self.as_of == today {
            "today"
        } else {
            self.source.name()
        };
        format!("rates {} ({how})", self.as_of)
    }

    /// `rates 2026-11-18 (live, fetched 2026-11-18T09:14:05Z)`.
    pub fn describe(&self) -> String {
        let mut text = format!("rates {} ({}", self.as_of, self.source.name());
        if let Some(at) = self
            .fetched_at
            .and_then(|s| jiff::Timestamp::from_second(s).ok())
        {
            text += &format!(", fetched {at}");
        }
        text + ")"
    }
}

/// Does `text` write money: a sign of it, or a currency code? Then its sums
/// are by rates, which a front end names. A word that is a code and nothing
/// to do with money, `GBP` in a name, is taken for it: this only decides
/// whether to say what the rates are.
pub fn mentions(text: &str) -> bool {
    static CODES: OnceLock<regex::Regex> = OnceLock::new();
    if text.contains(siquant_money()) {
        return true;
    }
    let codes = CODES.get_or_init(|| {
        let names: Vec<String> = Registry::builtin()
            .units()
            .filter(|u| u.currency)
            .map(|u| regex::escape(u.symbol))
            .collect();
        regex::Regex::new(&format!(r"(?:^|[^A-Za-z])(?:{})\b", names.join("|")))
            .expect("codes are a pattern")
    });
    codes.is_match(text)
}

/// The signs of money, the same as the lexer reads.
fn siquant_money() -> &'static [char] {
    omasheet_omx::units::MONEY
}

struct State {
    fetched_at: Option<i64>,
    source: Source,
}

static STATE: Mutex<State> = Mutex::new(State {
    fetched_at: None,
    source: Source::Bundled,
});

/// The rates in use.
pub fn status() -> Status {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    Status {
        as_of: Registry::builtin().rates_as_of(),
        fetched_at: state.fetched_at,
        source: state.source,
    }
}

/// The date, the rates and, of a kept copy, when it was fetched.
pub type Parsed = (String, Vec<(String, f64)>, Option<i64>);

/// The date and the rates in a file in the form the source serves,
/// `{"date": "2026-11-18", "usd": {"eur": 0.86, ...}}`, which a cached copy
/// is too, with `fetched_at` added.
pub fn parse(text: &str) -> Result<Parsed, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let date = value
        .get("date")
        .and_then(Value::as_str)
        .ok_or("no `date`")?
        .to_string();
    let usd = value
        .get("usd")
        .and_then(Value::as_object)
        .ok_or("no `usd` rates")?;
    let rates: Vec<(String, f64)> = usd
        .iter()
        .filter_map(|(code, rate)| Some((code.clone(), rate.as_f64()?)))
        .collect();
    if rates.is_empty() {
        return Err("no rates in it".to_string());
    }
    let fetched_at = value.get("fetched_at").and_then(Value::as_i64);
    Ok((date, rates, fetched_at))
}

/// Puts rates in use, and how many were set.
fn apply(date: &str, rates: &[(String, f64)]) -> usize {
    Registry::builtin().set_rates(date, rates.iter().map(|(k, v)| (k.as_str(), *v)))
}

/// The file the latest fetch is kept in, `rates.json` in the cache
/// directory of the user.
pub fn cache_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(|h| PathBuf::from(h).join(".cache"))
        })
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    Some(dir.join("omasheet").join("rates.json"))
}

/// Uses the copy kept from an earlier fetch, if it is there and is for a
/// later date than the rates in use. Nothing is said of one that is not
/// readable: there are always the bundled rates.
pub fn load_cache() -> Option<Status> {
    let text = std::fs::read_to_string(cache_path()?).ok()?;
    let (date, rates, fetched_at) = parse(&text).ok()?;
    if date < Registry::builtin().rates_as_of() {
        return None;
    }
    if apply(&date, &rates) == 0 {
        return None;
    }
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.fetched_at = fetched_at;
    state.source = Source::Cache;
    drop(state);
    Some(status())
}

/// The text of a page, by `curl`, which every system this runs on has; a
/// connection that does not answer in a few seconds is given up.
fn get(url: &str) -> Result<String, String> {
    let out = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location"])
        .args(["--connect-timeout", "5", "--max-time", "15", url])
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{url}: {}", why.trim()));
    }
    String::from_utf8(out.stdout).map_err(|_| format!("{url}: not text"))
}

/// Fetches the latest rates, puts them in use and keeps a copy for the next
/// time there is no network. A fetch that fails, or gives nothing usable,
/// leaves the rates as they were.
pub fn update() -> Result<Status, String> {
    let mut problems = Vec::new();
    for url in SOURCES {
        let text = match get(url) {
            Ok(text) => text,
            Err(e) => {
                problems.push(e);
                continue;
            }
        };
        let (date, rates, _) = match parse(&text) {
            Ok(found) => found,
            Err(e) => {
                problems.push(format!("{url}: {e}"));
                continue;
            }
        };
        if apply(&date, &rates) == 0 {
            problems.push(format!("{url}: no currency it knows"));
            continue;
        }
        let now = jiff::Timestamp::now().as_second();
        {
            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            state.fetched_at = Some(now);
            state.source = Source::Live;
        }
        keep(&date, &rates, now);
        return Ok(status());
    }
    Err(problems.join("; "))
}

/// Keeps the rates as a file, written whole and then moved into place so a
/// reader never sees half of one. Failing to is no matter: it is only a copy.
fn keep(date: &str, rates: &[(String, f64)], fetched_at: i64) {
    let Some(path) = cache_path() else { return };
    let usd: serde_json::Map<String, Value> = rates
        .iter()
        .map(|(code, rate)| (code.clone(), json!(rate)))
        .collect();
    let body = json!({"fetched_at": fetched_at, "date": date, "usd": usd});
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, body.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sources_shape_is_read() {
        let (date, rates, at) =
            parse("{\"date\": \"2026-11-18\", \"usd\": {\"eur\": 0.86, \"btc\": 1e-5}}").unwrap();
        assert_eq!(date, "2026-11-18");
        assert_eq!(rates.len(), 2);
        assert_eq!(at, None);
        // A kept copy says when it was fetched.
        let kept = "{\"fetched_at\": 5, \"date\": \"d\", \"usd\": {\"eur\": 1}}";
        assert_eq!(parse(kept).unwrap().2, Some(5));
    }

    #[test]
    fn a_file_with_money_in_it_is_known() {
        assert!(mentions("| a | €5 |"));
        assert!(mentions("x := 5 + 3USD"));
        assert!(mentions("= 2.to('GBP')"));
        assert!(!mentions("| a | 5kg |"));
        assert!(!mentions("a US and a UK"));
    }

    #[test]
    fn the_note_says_today_for_todays_rates() {
        let status = Status {
            as_of: "2026-11-18".into(),
            fetched_at: None,
            source: Source::Live,
        };
        assert_eq!(status.short_on("2026-11-18"), "rates 2026-11-18 (today)");
        assert_eq!(status.short_on("2026-11-19"), "rates 2026-11-18 (live)");
    }

    #[test]
    fn what_is_not_rates_is_refused() {
        assert!(parse("<html>").is_err());
        assert!(parse("{\"usd\": {\"eur\": 1}}").is_err());
        assert!(parse("{\"date\": \"d\", \"usd\": {}}").is_err());
        assert!(parse("{\"date\": \"d\"}").is_err());
    }

    #[test]
    fn the_bundled_rates_are_in_use_to_begin_with() {
        let s = status();
        assert!(!s.as_of.is_empty());
        assert!(Registry::builtin().rate("EUR").is_some());
    }
}
