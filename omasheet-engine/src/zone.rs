// Copyright (c) 2026 Stephen Roe

//! Time zones, from the time zone database of the operating system.
//!
//! A date-time is kept as a wall clock: the seconds since the start of
//! 1970-01-01 that a clock in its zone shows. A [`Zone`] turns that into the
//! same count in UTC, which is the same instant everywhere, and back.

use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Zone {
    name: String,
    tz: TimeZone,
}

impl PartialEq for Zone {
    fn eq(&self, other: &Zone) -> bool {
        self.name == other.name
    }
}

impl Zone {
    /// The zone with an IANA name such as `Europe/London`, or `None` if the
    /// time zone database has no such zone.
    pub fn named(name: &str) -> Option<Zone> {
        let tz = TimeZone::get(name).ok()?;
        Some(Zone {
            name: tz.iana_name().unwrap_or(name).to_string(),
            tz,
        })
    }

    /// The zone this machine is set to.
    pub fn system() -> Zone {
        let tz = TimeZone::system();
        Zone {
            name: tz.iana_name().unwrap_or("local").to_string(),
            tz,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The instant at which the clocks of this zone show `wall`. A time the
    /// clocks skip is taken as if they had not gone forward, and a time they
    /// show twice as the first of the two. `None` if it is out of range.
    pub fn to_utc(&self, wall: i64) -> Option<i64> {
        let civil = Offset::UTC.to_datetime(Timestamp::from_second(wall).ok()?);
        let instant = self.tz.to_ambiguous_timestamp(civil).compatible().ok()?;
        Some(instant.as_second())
    }

    /// What the clocks of this zone show at the instant `utc`.
    pub fn from_utc(&self, utc: i64) -> Option<i64> {
        let offset = self.tz.to_offset(Timestamp::from_second(utc).ok()?);
        utc.checked_add(offset.seconds() as i64)
    }
}

/// The instant now, as seconds since the start of 1970-01-01 in UTC.
pub fn utc_now() -> i64 {
    Timestamp::now().as_second()
}

/// A date-time in a zone other than the sheet's own.
#[derive(Clone, Debug)]
pub struct Zoned {
    /// What the clocks of `zone` show.
    pub wall: i64,
    /// The instant, in UTC.
    pub utc: i64,
    pub zone: Rc<Zone>,
    /// The zone of the sheet, to compare with its own date-times.
    pub home: Rc<Zone>,
}

impl Zoned {
    /// The instant `utc` in `zone`. `None` if it is out of range.
    pub fn at(utc: i64, zone: &Rc<Zone>, home: &Rc<Zone>) -> Option<Zoned> {
        Some(Zoned {
            wall: zone.from_utc(utc)?,
            utc,
            zone: zone.clone(),
            home: home.clone(),
        })
    }

    /// What the clocks of the sheet's zone show at this instant.
    pub fn home_wall(&self) -> i64 {
        self.home.from_utc(self.utc).unwrap_or(self.wall)
    }

    /// The clocks of the same zone, `secs` later.
    pub fn shifted(&self, secs: i64) -> Option<Zoned> {
        let wall = self.wall.checked_add(secs)?;
        Some(Zoned {
            wall,
            utc: self.zone.to_utc(wall)?,
            ..self.clone()
        })
    }

    /// Seconds ahead of UTC.
    pub fn offset(&self) -> i64 {
        self.wall - self.utc
    }
}

/// An offset from UTC as `+01:00`.
pub fn format_offset(secs: i64) -> String {
    let (sign, abs) = (if secs < 0 { '-' } else { '+' }, secs.abs());
    format!("{sign}{:02}:{:02}", abs / 3600, abs / 60 % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omasheet_omx::date::{datetime_from, from_ymd};

    fn at(y: i64, m: i64, d: i64, h: i64, min: i64) -> i64 {
        datetime_from(from_ymd(y, m, d).unwrap(), h, min, 0).unwrap()
    }

    #[test]
    fn zones_convert_with_daylight_saving() {
        let london = Zone::named("Europe/London").unwrap();
        let tokyo = Zone::named("Asia/Tokyo").unwrap();
        assert_eq!(london.name(), "Europe/London");
        assert!(Zone::named("Europe/Atlantis").is_none());
        // Winter is GMT, summer an hour ahead.
        assert_eq!(
            london.to_utc(at(2025, 1, 15, 12, 0)),
            Some(at(2025, 1, 15, 12, 0))
        );
        assert_eq!(
            london.to_utc(at(2025, 7, 15, 12, 0)),
            Some(at(2025, 7, 15, 11, 0))
        );
        assert_eq!(
            london.from_utc(at(2025, 7, 15, 11, 0)),
            Some(at(2025, 7, 15, 12, 0))
        );
        assert_eq!(
            tokyo.from_utc(at(2025, 7, 15, 11, 0)),
            Some(at(2025, 7, 15, 20, 0))
        );
        // 01:30 on 30 March 2025 was skipped, and on 26 October came twice.
        assert_eq!(
            london.to_utc(at(2025, 3, 30, 1, 30)),
            Some(at(2025, 3, 30, 1, 30))
        );
        assert_eq!(
            london.to_utc(at(2025, 10, 26, 1, 30)),
            Some(at(2025, 10, 26, 0, 30))
        );
        // Far ahead still follows the rule.
        assert_eq!(
            london.to_utc(at(2090, 7, 15, 12, 0)),
            Some(at(2090, 7, 15, 11, 0))
        );
    }

    #[test]
    fn zoned_values_keep_their_instant() {
        let home = Rc::new(Zone::named("Europe/London").unwrap());
        let tokyo = Rc::new(Zone::named("Asia/Tokyo").unwrap());
        let z = Zoned::at(at(2025, 7, 15, 11, 0), &tokyo, &home).unwrap();
        assert_eq!(z.wall, at(2025, 7, 15, 20, 0));
        assert_eq!(z.home_wall(), at(2025, 7, 15, 12, 0));
        assert_eq!(z.offset(), 9 * 3600);
        assert_eq!(z.shifted(3600).unwrap().utc, at(2025, 7, 15, 12, 0));
        assert_eq!(format_offset(9 * 3600), "+09:00");
        assert_eq!(format_offset(-(3 * 3600 + 30 * 60)), "-03:30");
        assert_eq!(format_offset(0), "+00:00");
    }
}
