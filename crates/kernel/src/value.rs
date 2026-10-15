//! Typed values: each kind of value has one look, used everywhere, and dates are written one way, absolute or relative by one setting.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::app::Moment;
use crate::ink::Ink;

/// Which kind of value a component shows. Each has one look: a count and a date in the monospace face, a state filled with its ink, a version, a product code, a piece of code and a mode each in their own box, an error in the danger ink. The boxed ones are badges: they say what something is and never act.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Count,
    Date,
    Version,
    State(Ink),
    Product,
    Code,
    Mode,
    Error,
}

impl Value {
    /// Whether it is drawn in a box one column wider than its text.
    pub fn boxed(self) -> bool {
        !matches!(self, Value::Count | Value::Error)
    }
}

/// How every date is written: as the day and time it was, or as how long ago it was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dates {
    #[default]
    Absolute,
    Relative,
}

/// What writing a date needs: the setting, the moment it is now and the time zone. A live calendar reads the clock each time the view is built; a fixed one, used for proofs and laws, always says it is noon on 1 June 2026 in UTC, so a render never depends on when or where it was made.
#[derive(Clone, Debug, PartialEq)]
pub struct Calendar {
    pub dates: Dates,
    pub now: Moment,
    pub zone: TimeZone,
    pub live: bool,
}

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;

impl Default for Calendar {
    fn default() -> Calendar {
        Calendar { dates: Dates::Absolute, now: Moment(1_780_315_200_000), zone: TimeZone::UTC, live: false }
    }
}

impl Calendar {
    /// The calendar of the machine it runs on: its clock and its time zone, with dates written as `dates` says.
    pub fn live(dates: Dates) -> Calendar {
        Calendar { dates, now: Moment::now(), zone: TimeZone::system(), live: true }
    }

    /// A fixed calendar that writes dates as `dates` says.
    pub fn fixed(dates: Dates) -> Calendar {
        Calendar { dates, ..Calendar::default() }
    }

    /// The day it is now, in the calendar's time zone.
    pub fn today(&self) -> crate::days::Day {
        let Ok(now) = Timestamp::from_millisecond(self.now.0) else { return crate::days::Day::default() };
        let date = now.to_zoned(self.zone.clone()).date();
        crate::days::Day::new(date.year() as i32, date.month() as u32, date.day() as u32).unwrap_or_default()
    }

    /// `at`, written the one way every date is written, and the moment that writing next changes, if it does.
    pub fn write(&self, at: Moment) -> (String, Option<Moment>) {
        let (Ok(when), Ok(now)) = (Timestamp::from_millisecond(at.0), Timestamp::from_millisecond(self.now.0)) else { return ("?".into(), None) };
        let (when, now) = (when.to_zoned(self.zone.clone()), now.to_zoned(self.zone.clone()));
        if self.dates == Dates::Absolute {
            return (when.strftime("%Y-%m-%d %H:%M").to_string(), None);
        }
        let midnight = now.date().tomorrow().ok().and_then(|d| d.to_zoned(self.zone.clone()).ok()).map(|z| Moment(z.timestamp().as_millisecond()));
        let days = now.date().since(when.date()).map_or(0, |s| s.get_days() as i64);
        if days != 0 {
            return (relative_day(days), midnight);
        }
        let ago = self.now.0 - at.0;
        let unit = if ago.abs() < HOUR { MINUTE } else { HOUR };
        let next = Moment(self.now.0 + unit - (self.now.0 - at.0).rem_euclid(unit));
        (relative_time(ago), Some(midnight.map_or(next, |m| m.min(next))))
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

fn relative_day(days: i64) -> String {
    let ago = |n: i64, one: &str, many: &str| format!("{} ago", plural(n, one, many));
    let ahead = |n: i64, one: &str, many: &str| format!("in {}", plural(n, one, many));
    match days {
        1 => "yesterday".into(),
        -1 => "tomorrow".into(),
        2..=13 => ago(days, "day", "days"),
        14..=59 => ago(days / 7, "week", "weeks"),
        60..=729 => ago(days / 30, "month", "months"),
        730.. => ago(days / 365, "year", "years"),
        -13..=-2 => ahead(-days, "day", "days"),
        -59..=-14 => ahead(-days / 7, "week", "weeks"),
        -729..=-60 => ahead(-days / 30, "month", "months"),
        _ => ahead(-days / 365, "year", "years"),
    }
}

fn relative_time(ago: i64) -> String {
    if ago <= -HOUR {
        format!("in {} h", -ago / HOUR)
    } else if ago <= -MINUTE {
        format!("in {} min", -ago / MINUTE)
    } else if ago < MINUTE {
        "just now".into()
    } else if ago < HOUR {
        format!("{} min ago", ago / MINUTE)
    } else {
        format!("{} h ago", ago / HOUR)
    }
}

/// A whole number a count can show.
pub trait Integer: Copy {
    fn wide(self) -> i128;
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl Integer for $t {
            fn wide(self) -> i128 {
                self as i128
            }
        }
    )*};
}

integer!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, i128);

/// A count with its thousands grouped: 1,284.
pub fn grouped(n: impl Integer) -> String {
    let n = n.wide();
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('−');
    }
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}
