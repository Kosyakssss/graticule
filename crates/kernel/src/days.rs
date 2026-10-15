//! Days and times of day as facts, and reading them from what people type: `2026-06-03`, `tomorrow`, `next tue`, `3pm`.

use std::fmt;

use ledger::{Field, Value};

/// A day on the calendar, with no time and no zone: a due date, a birthday. Kept as the number of days since 1 January 1970.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub i32);

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

impl Day {
    /// Day `day` of `month`, 1 to 12, in `year`, if there is one.
    pub fn new(year: i32, month: u32, day: u32) -> Option<Day> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in(year, month)).then(|| Day(days_from(year, month, day)))
    }

    /// The year, month (1 to 12) and day of the month.
    pub fn parts(self) -> (i32, u32, u32) {
        let z = self.0 as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        (year, month, day)
    }

    /// The day of the week, 0 for Monday to 6 for Sunday.
    pub fn weekday(self) -> u32 {
        (self.0 as i64 + 3).rem_euclid(7) as u32
    }

    /// The day `months` months on, on the same day of the month or the month's last if it is shorter.
    pub fn add_months(self, months: i32) -> Day {
        let (year, month, day) = self.parts();
        let index = year * 12 + month as i32 - 1 + months;
        let (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u32 + 1);
        Day(days_from(year, month, day.min(days_in(year, month))))
    }

    /// The month it is in, counted as `year * 12 + month - 1`, so months next to each other differ by one.
    pub fn month_index(self) -> i32 {
        let (year, month, _) = self.parts();
        year * 12 + month as i32 - 1
    }

    /// The first day of the month counted as `month_index` counts.
    pub fn first_of(index: i32) -> Day {
        Day(days_from(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1))
    }

    /// Said in full, for a screen reader: `Wednesday 3 June 2026`.
    pub fn spoken(self) -> String {
        let (year, month, day) = self.parts();
        format!("{} {day} {} {year}", WEEKDAYS[self.weekday() as usize], MONTHS[month as usize - 1])
    }

    /// The month's name and year: `June 2026`.
    pub fn month_name(self) -> String {
        let (year, month, _) = self.parts();
        format!("{} {year}", MONTHS[month as usize - 1])
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = self.parts();
        write!(f, "{year:04}-{month:02}-{day:02}")
    }
}

impl Field for Day {
    ledger::register!(Day);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Int(self.0 as i64)
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Int(i) => i32::try_from(*i).ok().map(Day),
            _ => None,
        }
    }
}
impl ledger::Document for Day {}

/// A time of day, with no day and no zone: when a shop opens. Kept as minutes after midnight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(pub u16);

const DAY_MINUTES: i32 = 24 * 60;

impl Time {
    /// `hour`:`minute`, if it is a time of day.
    pub fn new(hour: u32, minute: u32) -> Option<Time> {
        (hour < 24 && minute < 60).then(|| Time((hour * 60 + minute) as u16))
    }

    /// The hour, 0 to 23, and the minute.
    pub fn parts(self) -> (u32, u32) {
        (self.0 as u32 / 60, self.0 as u32 % 60)
    }

    /// The time `minutes` later, going round midnight.
    pub fn add_minutes(self, minutes: i32) -> Time {
        Time((self.0 as i32 + minutes).rem_euclid(DAY_MINUTES) as u16)
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (hour, minute) = self.parts();
        write!(f, "{hour:02}:{minute:02}")
    }
}

impl Field for Time {
    ledger::register!(Time);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::UInt(self.0 as u64)
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::UInt(m) if *m < DAY_MINUTES as u64 => Some(Time(*m as u16)),
            _ => None,
        }
    }
}
impl ledger::Document for Time {}

fn leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in(year: i32, month: u32) -> u32 {
    match month {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn days_from(year: i32, month: u32, day: u32) -> i32 {
    let year = year as i64 - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i32
}

/// What doesn't read as a day says this.
pub const NOT_A_DAY: &str = "That isn't a day: try 2026-06-03, tomorrow, next tue or jun 3";
/// What doesn't read as a time of day says this.
pub const NOT_A_TIME: &str = "That isn't a time of day: try 15:30, 3pm or noon";

/// The day `text` names, counting from `today`: a date written `2026-06-03`, `today`, `tomorrow`, `yesterday`, a weekday (`tue` is the coming one, today included; `next tue` the one after today; `last tue` the one before), `+3` or `in 3 days`, `2 weeks ago`, `next week`, `next month`, or a month and a day in either order with an optional year (`jun 3`, `3 june 2027`).
pub fn read_day(text: &str, today: Day) -> Result<Day, &'static str> {
    let text = text.trim().to_lowercase();
    let words: Vec<&str> = text.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).collect();
    let known = match words.as_slice() {
        [] => None,
        ["today"] => Some(today),
        ["tomorrow"] => Some(Day(today.0 + 1)),
        ["yesterday"] => Some(Day(today.0 - 1)),
        [one] if one.starts_with('+') || one.starts_with('-') && one.len() > 1 && !one[1..].contains('-') => one.parse::<i32>().ok().map(|n| Day(today.0 + n)),
        [one] if one.contains('-') || one.contains('/') => iso(one),
        [one] => weekday(one).map(|w| Day(today.0 + (w as i32 - today.weekday() as i32).rem_euclid(7))).or_else(|| one.parse::<u32>().ok().and_then(|d| this_month(today, d))),
        ["next", "week"] => Some(Day(today.0 + 7)),
        ["next", "month"] => Some(today.add_months(1)),
        ["next", "year"] => Some(today.add_months(12)),
        ["last", "week"] => Some(Day(today.0 - 7)),
        ["last", "month"] => Some(today.add_months(-1)),
        ["next", w] => weekday(w).map(|w| Day(today.0 + (w as i32 - today.weekday() as i32 - 1).rem_euclid(7) + 1)),
        ["last", w] => weekday(w).map(|w| Day(today.0 - (today.weekday() as i32 - w as i32 - 1).rem_euclid(7) - 1)),
        ["in", n, unit] => amount(n, unit).map(|(n, months)| if months { today.add_months(n) } else { Day(today.0 + n) }),
        [n, unit, "ago"] => amount(n, unit).map(|(n, months)| if months { today.add_months(-n) } else { Day(today.0 - n) }),
        [a, b] => month_day(a, b, None, today),
        [a, b, year] => year.parse::<i32>().ok().and_then(|y| month_day(a, b, Some(y), today)),
        _ => None,
    };
    known.ok_or(NOT_A_DAY)
}

/// The time of day `text` names: `15:30`, `15.30`, `1530`, `15`, `3pm`, `3:30 pm`, `noon` or `midnight`.
pub fn read_time(text: &str) -> Result<Time, &'static str> {
    let text = text.trim().to_lowercase().replace(' ', "");
    let known = match text.as_str() {
        "noon" | "midday" => Time::new(12, 0),
        "midnight" => Time::new(0, 0),
        _ => {
            let (clock, half) = match text.strip_suffix("am").or_else(|| text.strip_suffix('a')) {
                Some(rest) => (rest, Some(false)),
                None => match text.strip_suffix("pm").or_else(|| text.strip_suffix('p')) {
                    Some(rest) => (rest, Some(true)),
                    None => (text.as_str(), None),
                },
            };
            let (hour, minute) = match clock.split_once([':', '.', 'h']) {
                Some((h, m)) => (h.parse::<u32>().ok(), if m.is_empty() { Some(0) } else { m.parse::<u32>().ok().filter(|_| m.len() == 2) }),
                None if clock.len() > 2 && clock.chars().all(|c| c.is_ascii_digit()) => (clock[..clock.len() - 2].parse().ok(), clock[clock.len() - 2..].parse().ok()),
                None => (clock.parse().ok(), Some(0)),
            };
            match (hour, minute, half) {
                (Some(h), Some(m), None) => Time::new(h, m),
                (Some(h @ 1..=12), Some(m), Some(pm)) => Time::new(h % 12 + if pm { 12 } else { 0 }, m),
                _ => None,
            }
        }
    };
    known.ok_or(NOT_A_TIME)
}

fn iso(text: &str) -> Option<Day> {
    let parts: Vec<&str> = text.split(['-', '/']).collect();
    let [y, m, d] = parts.as_slice() else { return None };
    Day::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
}

fn weekday(word: &str) -> Option<u32> {
    (word.len() >= 2).then(|| WEEKDAYS.iter().position(|w| w.to_lowercase().starts_with(word))).flatten().map(|w| w as u32)
}

fn month(word: &str) -> Option<u32> {
    (word.len() >= 3).then(|| MONTHS.iter().position(|m| m.to_lowercase().starts_with(word))).flatten().map(|m| m as u32 + 1)
}

fn this_month(today: Day, day: u32) -> Option<Day> {
    let (year, month, _) = today.parts();
    Day::new(year, month, day)
}

fn amount(n: &str, unit: &str) -> Option<(i32, bool)> {
    let n: i32 = n.parse().ok()?;
    match unit.trim_end_matches('s') {
        "day" => Some((n, false)),
        "week" => Some((n * 7, false)),
        "month" => Some((n, true)),
        "year" => Some((n * 12, true)),
        _ => None,
    }
}

fn month_day(a: &str, b: &str, year: Option<i32>, today: Day) -> Option<Day> {
    let (m, d) = match (month(a), month(b)) {
        (Some(m), None) => (m, b.trim_end_matches(['s', 't', 'n', 'd', 'r', 'h']).parse().ok()?),
        (None, Some(m)) => (m, a.trim_end_matches(['s', 't', 'n', 'd', 'r', 'h']).parse().ok()?),
        _ => return None,
    };
    Day::new(year.unwrap_or(today.parts().0), m, d)
}
