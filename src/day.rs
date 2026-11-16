//! Calendar days with no time and no time zone, and the arithmetic every
//! reader shares: so many days, weeks or months on, the day of the week,
//! the ends of a week and a month.
//!
//! A day is kept as a count of days, so moving and comparing are whole
//! numbers. Every day is in a year written with four digits, 0001 to 9999:
//! arithmetic that would leave them answers `None` rather than a day that
//! can't be written.

use core::fmt;

/// A day of the week.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[allow(missing_docs)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    /// Every day of the week, from Monday, as weeks run here.
    pub const ALL: [Weekday; 7] = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];

    /// How many days after Monday it is, from 0.
    pub const fn from_monday(self) -> i32 {
        self as i32
    }

    /// Whether it's a Saturday or a Sunday.
    pub const fn is_weekend(self) -> bool {
        matches!(self, Weekday::Saturday | Weekday::Sunday)
    }

    /// Its name in English, like "Tuesday".
    pub const fn name(self) -> &'static str {
        ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]
            [self as usize]
    }
}

/// A calendar day, like 20 October 2026.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(i32);

// Days from 1 January 1970 to the first and last days there are.
const FIRST: i32 = days_from_civil(1, 1, 1);
const LAST: i32 = days_from_civil(9999, 12, 31);

/// Days from 1 January 1970 to a day of the proleptic Gregorian calendar
/// (Howard Hinnant's `days_from_civil`).
const fn days_from_civil(year: i32, month: u32, day: u32) -> i32 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = (month as i32 + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The year, month and day of the month so many days from 1 January 1970.
const fn civil_from_days(days: i32) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}

/// Whether a year has a 29 February.
pub const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// How many days a month has, from 1 for January.
pub const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Day {
    /// The first day there is: 1 January 0001.
    pub const MIN: Day = Day(FIRST);
    /// The last day there is: 31 December 9999.
    pub const MAX: Day = Day(LAST);

    /// The day with a year, a month from 1 for January, and a day of the
    /// month, or `None` when there's no such day, like 30 February.
    pub const fn new(year: i32, month: u32, day: u32) -> Option<Day> {
        if year < 1 || year > 9999 || month < 1 || month > 12 {
            return None;
        }
        if day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Day(days_from_civil(year, month, day)))
    }

    /// The same, with a day past the end of its month taken as the month's
    /// last, so 29 February is the 28th in a year without one.
    pub const fn new_within_month(year: i32, month: u32, day: u32) -> Option<Day> {
        if year < 1 || year > 9999 || month < 1 || month > 12 || day < 1 {
            return None;
        }
        let last = days_in_month(year, month);
        Day::new(year, month, if day > last { last } else { day })
    }

    /// A day written `YYYY-MM-DD`, or `None` when it isn't one.
    pub fn parse(text: &str) -> Option<Day> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let number = |from: usize, to: usize| {
            bytes[from..to].iter().try_fold(0u32, |sum, byte| {
                byte.is_ascii_digit().then(|| sum * 10 + u32::from(byte - b'0'))
            })
        };
        Day::new(number(0, 4)? as i32, number(5, 7)?, number(8, 10)?)
    }

    /// Its year, its month from 1 for January, and its day of the month.
    pub const fn ymd(self) -> (i32, u32, u32) {
        civil_from_days(self.0)
    }

    /// Its year.
    pub const fn year(self) -> i32 {
        self.ymd().0
    }

    /// Its month, from 1 for January.
    pub const fn month(self) -> u32 {
        self.ymd().1
    }

    /// Its day of the month.
    pub const fn day(self) -> u32 {
        self.ymd().2
    }

    /// Its day of the week.
    pub const fn weekday(self) -> Weekday {
        // 1 January 1970 was a Thursday.
        Weekday::ALL[(self.0 + 3).rem_euclid(7) as usize]
    }

    /// The day so many days after it, or before it when negative.
    pub fn add_days(self, days: i64) -> Option<Day> {
        let to = i64::from(self.0).checked_add(days)?;
        (i64::from(FIRST)..=i64::from(LAST)).contains(&to).then_some(Day(to as i32))
    }

    /// So many months on, on the same day of the month, or the month's
    /// last when it's shorter.
    pub fn add_months(self, months: i64) -> Option<Day> {
        let (year, month, day) = self.ymd();
        let count = i64::from(year) * 12 + i64::from(month) - 1;
        let to = count.checked_add(months)?;
        let year = i32::try_from(to.div_euclid(12)).ok()?;
        Day::new_within_month(year, to.rem_euclid(12) as u32 + 1, day)
    }

    /// How many days on another day is, or back when negative.
    pub const fn days_until(self, other: Day) -> i64 {
        (other.0 - self.0) as i64
    }

    /// The first day of its month.
    pub fn month_start(self) -> Day {
        Day(self.0 - self.day() as i32 + 1)
    }

    /// The last day of its month.
    pub fn month_end(self) -> Day {
        let (year, month, day) = self.ymd();
        Day(self.0 + (days_in_month(year, month) - day) as i32)
    }

    /// The first day of the week it's in, for weeks that start on a day
    /// of the week: `Weekday::Monday`, as iCalendar's weeks do, or
    /// [`Locale::week_start`](crate::Locale::week_start) for weeks as
    /// they run somewhere.
    pub fn week_start(self, first: Weekday) -> Option<Day> {
        self.previous(first)
    }

    /// The next of a day of the week, on this day or after it.
    pub fn next(self, weekday: Weekday) -> Option<Day> {
        let ahead = (weekday.from_monday() - self.weekday().from_monday()).rem_euclid(7);
        self.add_days(i64::from(ahead))
    }

    /// The next of a day of the week after this day.
    pub fn next_after(self, weekday: Weekday) -> Option<Day> {
        self.add_days(1)?.next(weekday)
    }

    /// The last of a day of the week, on this day or before it.
    pub fn previous(self, weekday: Weekday) -> Option<Day> {
        let back = (self.weekday().from_monday() - weekday.from_monday()).rem_euclid(7);
        self.add_days(-i64::from(back))
    }

    /// The last of a day of the week before this day.
    pub fn previous_before(self, weekday: Weekday) -> Option<Day> {
        self.add_days(-1)?.previous(weekday)
    }

    /// The nearest of a day of the week: this day, or one no more than
    /// three days either side of it. At either end of the days there
    /// are, where the nearest isn't one, it's the one the other side.
    pub fn nearest(self, weekday: Weekday) -> Option<Day> {
        let ahead = i64::from((weekday.from_monday() - self.weekday().from_monday()).rem_euclid(7));
        let (near, far) = if ahead <= 3 { (ahead, ahead - 7) } else { (ahead - 7, ahead) };
        self.add_days(near).or_else(|| self.add_days(far))
    }
}

/// Written `YYYY-MM-DD`.
/// What was written isn't a day, a time or a rule this reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParseError(pub(crate) &'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ParseError {}

impl std::str::FromStr for Day {
    type Err = ParseError;

    /// A day written `YYYY-MM-DD`.
    fn from_str(text: &str) -> Result<Day, ParseError> {
        Day::parse(text).ok_or(ParseError("not a day written YYYY-MM-DD"))
    }
}

impl std::str::FromStr for Time {
    type Err = ParseError;

    /// A time written `HH:MM`.
    fn from_str(text: &str) -> Result<Time, ParseError> {
        Time::parse(text).ok_or(ParseError("not a time written HH:MM"))
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = self.ymd();
        write!(f, "{year:04}-{month:02}-{day:02}")
    }
}

impl fmt::Debug for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Day({self})")
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Day {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Day {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Day, D::Error> {
        let text = String::deserialize(deserializer)?;
        Day::parse(&text).ok_or_else(|| serde::de::Error::custom("not a day written YYYY-MM-DD"))
    }
}

/// A time of day, to the minute, in no time zone.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time(u16);

impl Time {
    /// Midnight, as a day starts.
    pub const MIDNIGHT: Time = Time(0);

    /// The time at an hour and a minute, or `None` past 23:59.
    pub const fn new(hour: u32, minute: u32) -> Option<Time> {
        if hour > 23 || minute > 59 { None } else { Some(Time((hour * 60 + minute) as u16)) }
    }

    /// A time written `HH:MM`.
    pub fn parse(text: &str) -> Option<Time> {
        let (hour, minute) = text.split_once(':')?;
        if hour.len() != 2 || minute.len() != 2 {
            return None;
        }
        Time::new(hour.parse().ok()?, minute.parse().ok()?)
    }

    /// Minutes since midnight.
    pub const fn minutes(self) -> u32 {
        self.0 as u32
    }

    /// The time so many minutes since midnight, round the clock.
    pub const fn from_minutes(minutes: i64) -> Time {
        Time(minutes.rem_euclid(24 * 60) as u16)
    }

    /// Its hour, from 0 to 23.
    pub const fn hour(self) -> u32 {
        self.0 as u32 / 60
    }

    /// Its minute.
    pub const fn minute(self) -> u32 {
        self.0 as u32 % 60
    }
}

/// Written `HH:MM`.
impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour(), self.minute())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Time {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Time {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Time, D::Error> {
        let text = String::deserialize(deserializer)?;
        Time::parse(&text).ok_or_else(|| serde::de::Error::custom("not a time written HH:MM"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "serde")]
    #[test]
    fn reads_from_anything_serde_reads() {
        let day: Day = serde_json::from_reader(&b"\"2026-11-16\""[..]).unwrap();
        assert_eq!(day, Day::new(2026, 11, 16).unwrap());
        let time: Time = serde_json::from_value(serde_json::json!("09:30")).unwrap();
        assert_eq!(time.to_string(), "09:30");
    }

    fn day(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    #[test]
    fn finds_the_nearest_of_a_day_of_the_week() {
        // A Sunday: the Wednesday before is four days off, the one after three.
        assert_eq!(day("2026-11-29").nearest(Weekday::Wednesday), Some(day("2026-12-02")));
        assert_eq!(day("2026-11-29").nearest(Weekday::Thursday), Some(day("2026-11-26")));
        assert_eq!(day("2026-11-29").nearest(Weekday::Sunday), Some(day("2026-11-29")));
        // The first day there is is a Monday, and the last a Friday: the
        // nearest is the other side where there's no day the near side.
        assert_eq!(Day::MIN.nearest(Weekday::Monday), Some(Day::MIN));
        assert_eq!(Day::MIN.nearest(Weekday::Sunday), Some(day("0001-01-07")));
        assert_eq!(Day::MAX.nearest(Weekday::Saturday), Some(day("9999-12-25")));
        for from in [Day::MIN, Day::MAX, day("2026-11-15")] {
            for weekday in Weekday::ALL {
                assert_eq!(from.nearest(weekday).map(Day::weekday), Some(weekday));
            }
        }
    }

    #[test]
    fn reads_and_writes_real_days_only() {
        assert_eq!(day("2026-10-20").to_string(), "2026-10-20");
        assert_eq!(day("2028-02-29").ymd(), (2028, 2, 29));
        for not in ["2026-02-30", "2027-02-29", "2026-13-01", "0000-01-01", "2026-1-01", "x"] {
            assert_eq!(Day::parse(not), None, "{not}");
        }
        assert_eq!(Day::MIN.to_string(), "0001-01-01");
        assert_eq!(Day::MAX.to_string(), "9999-12-31");
    }

    #[test]
    fn every_day_round_trips_in_order() {
        let mut at = day("1899-12-25");
        for _ in 0..80_000 {
            let next = at.add_days(1).unwrap();
            let (year, month, date) = next.ymd();
            assert_eq!(Day::new(year, month, date), Some(next));
            assert_eq!(next.weekday().from_monday(), (at.weekday().from_monday() + 1) % 7);
            at = next;
        }
    }

    #[test]
    fn knows_the_day_of_the_week() {
        assert_eq!(day("2026-10-20").weekday(), Weekday::Tuesday);
        assert_eq!(day("1970-01-01").weekday(), Weekday::Thursday);
        assert_eq!(day("0001-01-01").weekday(), Weekday::Monday);
    }

    #[test]
    fn moves_by_months_to_the_months_last_when_short() {
        assert_eq!(day("2027-01-31").add_months(1), Some(day("2027-02-28")));
        assert_eq!(day("2028-02-29").add_months(12), Some(day("2029-02-28")));
        assert_eq!(day("2026-10-20").add_months(-10), Some(day("2025-12-20")));
        assert_eq!(day("2026-10-20").add_months(12 * 9000), None);
        assert_eq!(day("2026-10-20").add_days(i64::MAX), None);
    }

    #[test]
    fn finds_weekdays_and_the_ends_of_weeks_and_months() {
        let tuesday = day("2026-10-20");
        assert_eq!(tuesday.next(Weekday::Tuesday), Some(tuesday));
        assert_eq!(tuesday.next_after(Weekday::Tuesday), Some(day("2026-10-27")));
        assert_eq!(tuesday.previous(Weekday::Friday), Some(day("2026-10-16")));
        assert_eq!(tuesday.previous_before(Weekday::Tuesday), Some(day("2026-10-13")));
        assert_eq!(tuesday.week_start(Weekday::Monday), Some(day("2026-10-19")));
        assert_eq!(tuesday.week_start(Weekday::Sunday), Some(day("2026-10-18")));
        assert_eq!(tuesday.month_start(), day("2026-10-01"));
        assert_eq!(tuesday.month_end(), day("2026-10-31"));
        assert_eq!(tuesday.days_until(day("2026-12-25")), 66);
    }

    #[test]
    fn times_are_minutes_of_a_day() {
        assert_eq!(Time::parse("09:30").unwrap().to_string(), "09:30");
        assert_eq!(Time::new(24, 0), None);
        assert_eq!(Time::from_minutes(25 * 60).to_string(), "01:00");
        assert_eq!(Time::parse("9:30"), None);
    }
}
