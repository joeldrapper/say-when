//! Where something is from a day it's said against, like a reminder from
//! the event it's for: "3 days before", "the friday before", "the weekend
//! before", "the day before at 9am", "an hour before". It's kept as said,
//! not as the day it lands on, so it lands somewhere else when the event
//! moves.
//!
//! Written as text, a plain length is an ISO 8601 duration, the way an
//! iCalendar alarm's `TRIGGER` has it: `-P3D` is 3 days before, `PT1H` an
//! hour after, `P0D` on the day. The rest are this library's own:
//!
//! | Text           | Said                     |
//! |----------------|--------------------------|
//! | `-2WORKDAY`    | 2 working days before    |
//! | `+3WEEKDAY`    | 3 weekdays after         |
//! | `-FR`          | the Friday before        |
//! | `+MO`          | the Monday after         |
//! | `-WEEKEND`     | the weekend before       |
//! | `-P1D@09:00`   | the day before at 9am    |
//!
//! A weekend lands on its Saturday.

use crate::calendar::Working;
use crate::day::{Day, ParseError, Time, Weekday};
use crate::days::{DayExpr, Parser, SpanExpr, Unit, weekend_beside};
use crate::words::Token;
use core::fmt;
use std::str::FromStr;

/// What an offset counts in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum OffsetUnit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
    /// A day of the working week that isn't a day off.
    WorkingDay,
    /// Monday to Friday, whatever's off.
    Weekday,
}

impl OffsetUnit {
    fn name(self) -> &'static str {
        match self {
            OffsetUnit::Minute => "minute",
            OffsetUnit::Hour => "hour",
            OffsetUnit::Day => "day",
            OffsetUnit::Week => "week",
            OffsetUnit::Month => "month",
            OffsetUnit::Year => "year",
            OffsetUnit::WorkingDay => "working day",
            OffsetUnit::Weekday => "weekday",
        }
    }

    /// Whether it's a length of time within a day, which needs a time to
    /// count from.
    pub fn is_time(self) -> bool {
        matches!(self, OffsetUnit::Minute | OffsetUnit::Hour)
    }
}

/// How far an offset goes, and which way.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum Shift {
    /// So many of a unit, before when it's negative: "3 days before", "an
    /// hour after", and with none, "on the day".
    By {
        /// How many, before when negative.
        count: i64,
        /// Of what.
        unit: OffsetUnit,
    },
    /// The first of a day of the week before or after: "the friday
    /// before".
    Weekday {
        /// The day of the week.
        weekday: Weekday,
        /// Before, or after.
        before: bool,
    },
    /// The first weekend all before or all after, from its Saturday:
    /// "the weekend before".
    Weekend {
        /// Before, or after.
        before: bool,
    },
}

/// Where something is from a day, and the time it's at if it says.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub struct Offset {
    /// How far, and which way.
    pub shift: Shift,
    /// The time it's at, if it says, like 9am in "the day before at 9am".
    /// Never with a length of time within a day.
    pub at: Option<Time>,
}

impl Offset {
    /// So many of a unit, before when negative.
    pub fn by(count: i64, unit: OffsetUnit) -> Offset {
        Offset { shift: Shift::By { count, unit }, at: None }
    }

    /// The first of a day of the week before or after.
    pub fn weekday(weekday: Weekday, before: bool) -> Offset {
        Offset { shift: Shift::Weekday { weekday, before }, at: None }
    }

    /// The first weekend before or after, from its Saturday.
    pub fn weekend(before: bool) -> Offset {
        Offset { shift: Shift::Weekend { before }, at: None }
    }

    /// At a time of day, or `None` for a length of time within a day,
    /// which says the time itself.
    pub fn at(self, time: Time) -> Option<Offset> {
        match self.shift {
            Shift::By { unit, .. } if unit.is_time() => None,
            _ => Some(Offset { at: Some(time), ..self }),
        }
    }

    /// Whether it says a length of time within a day, which needs a time
    /// to count from.
    pub fn is_time(&self) -> bool {
        matches!(self.shift, Shift::By { unit, .. } if unit.is_time())
    }

    /// Where it lands from a day, and a time on it if there's one: at the
    /// time it says, or the one it's from. A length of time within a day
    /// lands nowhere from a day with no time.
    pub(crate) fn from(
        &self,
        day: Day,
        time: Option<Time>,
        calendar: &Working,
    ) -> Option<(Day, Option<Time>)> {
        let landed = match self.shift {
            Shift::By { count, unit } => match unit {
                OffsetUnit::Minute | OffsetUnit::Hour => {
                    let minutes =
                        count.checked_mul(if unit == OffsetUnit::Hour { 60 } else { 1 })?;
                    let at = i64::from(time?.minutes()).checked_add(minutes)?;
                    let day = day.add_days(at.div_euclid(24 * 60))?;
                    return Some((day, Some(Time::from_minutes(at))));
                }
                OffsetUnit::Day => day.add_days(count)?,
                OffsetUnit::Week => day.add_days(count.checked_mul(7)?)?,
                OffsetUnit::Month => day.add_months(count)?,
                OffsetUnit::Year => day.add_months(count.checked_mul(12)?)?,
                OffsetUnit::WorkingDay => calendar.moved(day, count, true)?,
                OffsetUnit::Weekday => Working::weekdays().moved(day, count, false)?,
            },
            Shift::Weekday { weekday, before: true } => day.previous_before(weekday)?,
            Shift::Weekday { weekday, before: false } => day.next_after(weekday)?,
            Shift::Weekend { before } => weekend_beside(day, before)?,
        };
        Some((landed, self.at.or(time)))
    }

    /// How it's said: "3 days before", "the Friday before at 9am", "on
    /// the day".
    pub fn describe(&self) -> String {
        let way = |before: bool| if before { "before" } else { "after" };
        let said = match self.shift {
            Shift::By { count: 0, .. } => "on the day".to_string(),
            Shift::By { count, unit } => {
                let n = count.unsigned_abs();
                let length = match (n, unit) {
                    (1, OffsetUnit::Day) => "the day".to_string(),
                    (1, OffsetUnit::Hour) => "an hour".to_string(),
                    (1, _) => format!("a {}", unit.name()),
                    (n, _) => format!("{n} {}s", unit.name()),
                };
                format!("{length} {}", way(count < 0))
            }
            Shift::Weekday { weekday, before } => format!("the {} {}", weekday.name(), way(before)),
            Shift::Weekend { before } => format!("the weekend {}", way(before)),
        };
        match self.at {
            Some(time) => format!("{said} at {}", clock_words(time)),
            None => said,
        }
    }
}

/// Reads an offset said in words, as [`crate::Reader::offset`] does.
pub(crate) fn read(reader: &crate::Reader, text: &str) -> Option<Offset> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut found: Option<(Offset, Option<Time>)> = shift(reader, text, false).map(|o| (o, None));
    // A time it's at, after the rest ("the day before at 9am") or before
    // it ("9am the day before", not "3 days before").
    for split in (0..words.len()).rev() {
        if found.is_some() {
            break;
        }
        let (left, right) = words.split_at(split);
        let (said, at) = if right[0].eq_ignore_ascii_case("at") {
            (left, crate::read_clock(&right[1..].join(" "), None))
        } else if (1..=2).contains(&left.len()) && !left[0].bytes().all(|b| b.is_ascii_digit()) {
            (right, crate::read_clock(&left.join(" "), None))
        } else {
            continue;
        };
        let Some(at) = at else { continue };
        found = shift(reader, &said.join(" "), true).map(|offset| (offset, Some(at)));
    }
    let (offset, at) = found?;
    match at {
        Some(at) => offset.at(at),
        None => Some(offset),
    }
}

/// How far and which way some words say, with nothing else.
fn shift(reader: &crate::Reader, text: &str, timed: bool) -> Option<Offset> {
    let said = crate::words::tokens(text);
    let parser = reader.parser(&said);
    shift_of(&said, timed, &parser)
}

/// How far and which way some words say, read as a parser like this one
/// would.
pub(crate) fn shift_of(said: &[Token], timed: bool, like: &Parser) -> Option<Offset> {
    let plain: Vec<&str> = said
        .iter()
        .map(|token| match token {
            Token::Word(word) | Token::Number(_, word) => word.as_str(),
            Token::Ordinal(_) => "",
        })
        .collect();
    // "on the day", "the day of", "the same day", or a time on its own.
    if matches!(plain[..], [] if timed)
        || matches!(plain[..], ["day"] | ["same", "day"] | ["that", "day"])
    {
        return Some(Offset::by(0, OffsetUnit::Day));
    }
    // "an hour before", "30 minutes after", "half an hour before", "2 hrs
    // earlier".
    let (count, rest) = match &said[..] {
        [Token::Word(half), Token::Number(1, a), rest @ ..]
            if half == "half" && (a == "a" || a == "an") =>
        {
            (None, rest)
        }
        [Token::Number(n, _), rest @ ..] => (Some(i64::try_from(*n).ok()?), rest),
        _ => (Some(0), &said[..]),
    };
    if let [Token::Word(unit), Token::Word(way)] = rest {
        let minutes = match unit.as_str() {
            "minute" | "minutes" | "min" | "mins" | "m" => Some(1),
            "hour" | "hours" | "hr" | "hrs" | "h" => Some(60),
            _ => None,
        };
        let before = match way.as_str() {
            "before" | "earlier" | "beforehand" | "prior" => Some(true),
            "after" | "later" | "afterwards" => Some(false),
            _ => None,
        };
        if let (Some(minutes), Some(before), false) = (minutes, before, timed) {
            let count = match count {
                None if minutes == 60 => 30,
                None => return None,
                Some(0) => return None,
                Some(n) => n.checked_mul(minutes)?,
            };
            let count = if before { -count } else { count };
            return Some(if minutes == 60 && count % 60 == 0 {
                Offset::by(count / 60, OffsetUnit::Hour)
            } else {
                Offset::by(count, OffsetUnit::Minute)
            });
        }
    }
    // The rest as a day is read, said of today, which is taken out after:
    // "3 days before", "the friday before", "the weekend after".
    let mut said = said.to_vec();
    said.push(Token::Word("today".to_string()));
    let parser = Parser { tokens: &said, ..*like };
    let today = || Box::new(DayExpr::FromToday(0));
    parser.day(0, 0).into_iter().filter(|(_, end)| *end == said.len()).find_map(|(expr, _)| {
        Some(match expr {
            DayExpr::Moved { count, unit, before, base, .. } if base == today() => {
                let unit = match unit {
                    Unit::Day => OffsetUnit::Day,
                    Unit::Week => OffsetUnit::Week,
                    Unit::Month => OffsetUnit::Month,
                    Unit::Year => OffsetUnit::Year,
                    Unit::WorkingDay => OffsetUnit::WorkingDay,
                    Unit::Weekday => OffsetUnit::Weekday,
                };
                Offset::by(if before { -count } else { count }, unit)
            }
            DayExpr::WeekdayBeside { weekday, before, base } if base == today() => {
                Offset::weekday(weekday, before)
            }
            DayExpr::StartOf(SpanExpr::WeekendBeside { before, base }) if base == today() => {
                Offset::weekend(before)
            }
            _ => return None,
        })
    })
}

/// A time as it's said: "9am", "9:30pm", "noon", "midnight".
pub(crate) fn clock_words(time: Time) -> String {
    let (hour, minute) = (time.hour(), time.minute());
    match (hour, minute) {
        (0, 0) => "midnight".to_string(),
        (12, 0) => "noon".to_string(),
        _ => {
            let half = if hour < 12 { "am" } else { "pm" };
            let hour = match hour % 12 {
                0 => 12,
                hour => hour,
            };
            if minute == 0 { format!("{hour}{half}") } else { format!("{hour}:{minute:02}{half}") }
        }
    }
}

use crate::rule::CODES;

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = |before: bool| if before { "-" } else { "+" };
        match self.shift {
            Shift::By { count, unit } => {
                let minus = if count < 0 { "-" } else { "" };
                let n = count.unsigned_abs();
                match unit {
                    OffsetUnit::Minute => write!(f, "{minus}PT{n}M"),
                    OffsetUnit::Hour => write!(f, "{minus}PT{n}H"),
                    OffsetUnit::Day => write!(f, "{minus}P{n}D"),
                    OffsetUnit::Week => write!(f, "{minus}P{n}W"),
                    OffsetUnit::Month => write!(f, "{minus}P{n}M"),
                    OffsetUnit::Year => write!(f, "{minus}P{n}Y"),
                    OffsetUnit::WorkingDay => write!(f, "{}{n}WORKDAY", sign(count < 0)),
                    OffsetUnit::Weekday => write!(f, "{}{n}WEEKDAY", sign(count < 0)),
                }?;
            }
            Shift::Weekday { weekday, before } => {
                write!(f, "{}{}", sign(before), CODES[weekday as usize])?
            }
            Shift::Weekend { before } => write!(f, "{}WEEKEND", sign(before))?,
        }
        if let Some(time) = self.at {
            write!(f, "@{time}")?;
        }
        Ok(())
    }
}

impl FromStr for Offset {
    type Err = ParseError;

    /// An offset written as [`Offset`]'s `Display` writes it, in any case.
    fn from_str(text: &str) -> Result<Offset, ParseError> {
        let error = ParseError("not an offset");
        if !text.is_ascii() {
            return Err(error);
        }
        let text = text.trim().to_ascii_uppercase();
        let (shift, at) = match text.split_once('@') {
            Some((shift, at)) => (shift, Some(Time::parse(at).ok_or(error)?)),
            None => (text.as_str(), None),
        };
        let (before, rest) = match shift.as_bytes().first() {
            Some(b'-') => (true, &shift[1..]),
            Some(b'+') => (false, &shift[1..]),
            _ => (false, shift),
        };
        let signed = |n: &str| -> Result<i64, ParseError> {
            if n.is_empty() || n.len() > 9 || !n.bytes().all(|b| b.is_ascii_digit()) {
                return Err(error);
            }
            let n: i64 = n.parse().map_err(|_| error)?;
            Ok(if before { -n } else { n })
        };
        let offset = if let Some(time) = rest.strip_prefix("PT") {
            let (n, unit) = time.split_at(time.len().saturating_sub(1));
            let unit = match unit {
                "H" => OffsetUnit::Hour,
                "M" => OffsetUnit::Minute,
                _ => return Err(error),
            };
            Offset::by(signed(n)?, unit)
        } else if let Some(length) = rest.strip_prefix('P') {
            let (n, unit) = length.split_at(length.len().saturating_sub(1));
            let unit = match unit {
                "D" => OffsetUnit::Day,
                "W" => OffsetUnit::Week,
                "M" => OffsetUnit::Month,
                "Y" => OffsetUnit::Year,
                _ => return Err(error),
            };
            Offset::by(signed(n)?, unit)
        } else if let Some(n) = rest.strip_suffix("WORKDAY") {
            Offset::by(signed(n)?, OffsetUnit::WorkingDay)
        } else if let Some(n) = rest.strip_suffix("WEEKDAY") {
            Offset::by(signed(n)?, OffsetUnit::Weekday)
        } else if rest == "WEEKEND" && rest.len() < shift.len() {
            Offset::weekend(before)
        } else if let (Some(at), true) =
            (CODES.iter().position(|code| *code == rest), rest.len() < shift.len())
        {
            Offset::weekday(Weekday::ALL[at], before)
        } else {
            return Err(error);
        };
        match at {
            Some(time) => offset.at(time).ok_or(error),
            None => Ok(offset),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Offset {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Offset {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Offset, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_the_same() {
        let nine = Time::new(9, 0).unwrap();
        for (offset, text, said) in [
            (Offset::by(-3, OffsetUnit::Day), "-P3D", "3 days before"),
            (Offset::by(-1, OffsetUnit::Day), "-P1D", "the day before"),
            (Offset::by(0, OffsetUnit::Day), "P0D", "on the day"),
            (Offset::by(1, OffsetUnit::Hour), "PT1H", "an hour after"),
            (Offset::by(-30, OffsetUnit::Minute), "-PT30M", "30 minutes before"),
            (Offset::by(-2, OffsetUnit::Week), "-P2W", "2 weeks before"),
            (Offset::by(-1, OffsetUnit::Month), "-P1M", "a month before"),
            (Offset::by(-2, OffsetUnit::WorkingDay), "-2WORKDAY", "2 working days before"),
            (Offset::by(3, OffsetUnit::Weekday), "+3WEEKDAY", "3 weekdays after"),
            (Offset::weekday(Weekday::Friday, true), "-FR", "the Friday before"),
            (Offset::weekend(true), "-WEEKEND", "the weekend before"),
            (
                Offset::by(-1, OffsetUnit::Day).at(nine).unwrap(),
                "-P1D@09:00",
                "the day before at 9am",
            ),
        ] {
            assert_eq!(offset.to_string(), text);
            assert_eq!(text.parse::<Offset>(), Ok(offset), "{text}");
            assert_eq!(offset.describe(), said);
        }
        assert_eq!("+p3d".parse::<Offset>(), Ok(Offset::by(3, OffsetUnit::Day)));
        for wrong in
            ["", "P", "-PT", "P3X", "P3–", "-P–", "FR", "-PT1H@09:00", "WEEKEND", "-P1D@9", "--P1D"]
        {
            assert!(wrong.parse::<Offset>().is_err(), "{wrong}");
        }
        assert_eq!(Offset::by(1, OffsetUnit::Hour).at(nine), None);
    }

    #[test]
    fn lands_from_a_day() {
        let day = |text| Day::parse(text).unwrap();
        let calendar = Working::weekdays();
        let two = Time::new(14, 0);
        // A Monday.
        let from = day("2026-11-16");
        let land = |offset: Offset, time| offset.from(from, time, &calendar);
        assert_eq!(land(Offset::by(-3, OffsetUnit::Day), two), Some((day("2026-11-13"), two)));
        assert_eq!(
            land(Offset::by(-1, OffsetUnit::WorkingDay), two),
            Some((day("2026-11-13"), two))
        );
        assert_eq!(
            land(Offset::weekday(Weekday::Friday, true), None),
            Some((day("2026-11-13"), None))
        );
        assert_eq!(land(Offset::weekend(true), None), Some((day("2026-11-14"), None)));
        assert_eq!(
            land(Offset::by(-15, OffsetUnit::Hour), two),
            Some((day("2026-11-15"), Time::new(23, 0)))
        );
        assert_eq!(land(Offset::by(-1, OffsetUnit::Hour), None), None);
        let nine = Time::new(9, 0).unwrap();
        assert_eq!(
            land(Offset::by(-1, OffsetUnit::Day).at(nine).unwrap(), two),
            Some((day("2026-11-15"), Some(nine)))
        );
    }
}
