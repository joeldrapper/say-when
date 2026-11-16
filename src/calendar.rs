//! The days people don't work, which only the caller knows: public
//! holidays are set by governments, differ from place to place, and change
//! from year to year, so none is built in. A caller gives a [`Calendar`]:
//! the days that are off, each with what it's called if it's called
//! anything, and which days of the week are worked.
//!
//! The library asks it one thing, whether a day is a working day, for "3
//! working days before christmas" and "the last working day of the month".
//! And the names in it can be said: with a day in it called "Summer bank
//! holiday", "the summer bank holiday", "the august bank holiday" and "the
//! next bank holiday" all read, without the library knowing what a bank
//! holiday is.
//!
//! With no calendar, a working day is Monday to Friday.

use crate::day::{Day, Weekday};
use crate::words::{Token, tokens};
use std::collections::HashSet;

/// A day that's off, like a public holiday.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DayOff {
    /// The day.
    pub day: Day,
    /// What it's called, like "Summer bank holiday", if anything.
    #[cfg_attr(feature = "serde", serde(default))]
    pub name: Option<String>,
}

/// The days that aren't worked: some days of the week, and the days off a
/// caller knows of.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Calendar {
    /// The days off, in any order. Days it doesn't cover, like those of
    /// years it has nothing for, count as worked when their weekday is.
    pub days_off: Vec<DayOff>,
    /// The days of the week that are worked: Monday to Friday, unless said.
    pub working_week: Vec<Weekday>,
}

impl Default for Calendar {
    fn default() -> Calendar {
        Calendar { days_off: Vec::new(), working_week: Weekday::ALL[..5].to_vec() }
    }
}

/// Words that mean any day off, whatever it's called.
const ANY: [&[&str]; 7] = [
    &["bank", "holiday"],
    &["public", "holiday"],
    &["federal", "holiday"],
    &["national", "holiday"],
    &["statutory", "holiday"],
    &["holiday"],
    &["day", "off"],
];

/// A calendar as the readers ask it: its days in order, each name as the
/// words a phrase would have.
#[derive(Clone, Debug, Default)]
pub(crate) struct Working {
    days: Vec<(Day, Option<String>, Vec<String>)>,
    /// Each name there is among them, once, as its words.
    names: Vec<Vec<String>>,
    /// The days off that would otherwise be worked, in order, once each.
    off: Vec<Day>,
    /// Every word a day off can be said with: those of the names, and
    /// those for any day off.
    said: HashSet<String>,
    week: [bool; 7],
}

impl From<Calendar> for Working {
    fn from(calendar: Calendar) -> Working {
        let mut days: Vec<_> = calendar
            .days_off
            .into_iter()
            .map(|off| {
                let words = off.name.as_deref().map(name_words).unwrap_or_default();
                (off.day, off.name, words)
            })
            .collect();
        days.sort_by_key(|(day, ..)| *day);
        let mut week = [false; 7];
        for weekday in calendar.working_week {
            week[weekday as usize] = true;
        }
        let mut names: Vec<Vec<String>> = days
            .iter()
            .map(|(_, _, words)| words.clone())
            .filter(|words| !words.is_empty())
            .collect();
        names.sort();
        names.dedup();
        let said = names
            .iter()
            .flatten()
            .cloned()
            .chain(ANY.iter().flat_map(|any| any.iter().map(|word| word.to_string())))
            .collect();
        let mut off: Vec<Day> =
            days.iter().map(|(day, ..)| *day).filter(|day| week[day.weekday() as usize]).collect();
        off.dedup();
        Working { days, names, off, said, week }
    }
}

/// A name as the words a phrase would have, numbers and all.
fn name_words(name: &str) -> Vec<String> {
    tokens(name)
        .into_iter()
        .map(|token| match token {
            Token::Word(word) | Token::Number(_, word) => word,
            Token::Ordinal(n) => n.to_string(),
        })
        .collect()
}

impl Working {
    /// A calendar with nothing off but the weekend.
    pub fn weekdays() -> Working {
        Working::from(Calendar::default())
    }

    fn off(&self, day: Day) -> bool {
        self.days.binary_search_by_key(&day, |(day, ..)| *day).is_ok()
    }

    /// Whether a day is worked: a day of the working week that isn't off.
    pub fn is_working_day(&self, day: Day) -> bool {
        self.week[day.weekday() as usize] && !self.off(day)
    }

    /// The first day off, on or after a day and within a span if one's
    /// given, that's said some way, with its name.
    pub fn find(&self, said: &Said, from: Day, to: Option<Day>) -> Option<(Day, &str)> {
        let start = self.days.partition_point(|(day, ..)| *day < from);
        self.days[start..]
            .iter()
            .take_while(|(day, ..)| to.is_none_or(|to| *day <= to))
            .find(|(_, _, words)| match said {
                Said::Any => true,
                Said::Ending(ending) => words.ends_with(ending),
            })
            .map(|(day, name, _)| (*day, name.as_deref().unwrap_or("a day off")))
    }

    /// How some words name days off, if they do: any of them, when
    /// they're words for a day off ("holiday", "bank holiday", "day off"),
    /// or those called that: a day's whole name ("summer bank holiday"), or
    /// the end of one, of two words or more. With `typing`, the last word
    /// can be the start of one, still being typed.
    pub fn naming(&self, words: &[&str], typing: bool) -> Option<Said> {
        let (last, rest) = words.split_last()?;
        let typing = typing && last.len() >= 2;
        // Most words are in no name at all, which is quick to say.
        if self.days.is_empty() || !(typing || self.said.contains(*last)) {
            return None;
        }
        // Whether these are the words a name ends with.
        let ends = |name: &mut dyn DoubleEndedIterator<Item = &str>| {
            let fits = |end: &str| end == *last || (typing && end.starts_with(last));
            name.next_back().is_some_and(fits)
                && name.rev().take(rest.len()).eq(rest.iter().rev().copied())
        };
        if ANY.iter().any(|any| any.len() == words.len() && ends(&mut any.iter().copied())) {
            return Some(Said::Any);
        }
        let name = self.names.iter().find(|name| {
            let whole = name.len() == words.len();
            (whole || (name.len() > words.len() && words.len() >= 2))
                && ends(&mut name.iter().map(String::as_str))
        })?;
        Some(Said::Ending(name[name.len() - words.len()..].to_vec()))
    }

    /// How many worked days there are from one day to another, counting
    /// both, by the days of the week alone or with the days off left out.
    fn worked(&self, first: Day, last: Day, days_off: bool) -> i64 {
        let days = first.days_until(last) + 1;
        let each_week = self.week.iter().filter(|worked| **worked).count() as i64;
        let odd = (0..days % 7)
            .filter_map(|back| last.add_days(-back))
            .filter(|day| self.week[day.weekday() as usize])
            .count() as i64;
        let off = |before: Day| self.off.partition_point(|day| *day < before) as i64;
        let left_out = if days_off {
            off(last) - off(first) + i64::from(self.off.binary_search(&last).is_ok())
        } else {
            0
        };
        days / 7 * each_week + odd - left_out
    }

    /// The day so many worked days after a day, or before it when
    /// negative: whole weeks at a time while there are weeks to go, counting
    /// what's worked in them, then a day at a time. So a hundred thousand
    /// working days on is a few steps, not a hundred thousand.
    pub fn moved(&self, from: Day, by: i64, days_off: bool) -> Option<Day> {
        let each_week = self.week.iter().filter(|worked| **worked).count() as i64;
        if each_week == 0 {
            return None;
        }
        let way = by.signum();
        let (mut day, mut left) = (from, by.abs());
        // So many weeks have fewer worked days than are left, so the last
        // of them is always come to a day at a time, and is a worked day.
        while left > each_week {
            let to = day.add_days(way * 7 * ((left - 1) / each_week))?;
            let (first, last) =
                if way > 0 { (day.add_days(1)?, to) } else { (to, day.add_days(-1)?) };
            left -= self.worked(first, last, days_off);
            day = to;
        }
        while left > 0 {
            day = day.add_days(way)?;
            let off = days_off && self.off.binary_search(&day).is_ok();
            if self.week[day.weekday() as usize] && !off {
                left -= 1;
            }
        }
        Some(day)
    }

    /// Whether a word is one a day off is said with.
    pub fn says(&self, word: &str) -> bool {
        self.said.contains(word)
    }

    /// The most words a day off's name has.
    pub fn longest(&self) -> usize {
        self.names.iter().map(Vec::len).max().unwrap_or(0).max(2)
    }
}

/// How some words name days off.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Said {
    /// Any day off.
    Any,
    /// Those whose names end with these words.
    Ending(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar() -> Working {
        let off = |day: &str, name: Option<&str>| DayOff {
            day: Day::parse(day).unwrap(),
            name: name.map(str::to_string),
        };
        Working::from(Calendar {
            days_off: vec![
                off("2026-12-25", Some("Christmas Day")),
                off("2026-08-31", Some("Summer bank holiday")),
                off("2026-05-04", Some("Early May bank holiday")),
                off("2026-11-06", None),
            ],
            ..Calendar::default()
        })
    }

    #[test]
    fn a_working_day_is_one_of_the_week_that_isnt_off() {
        let day = |text| Day::parse(text).unwrap();
        let calendar = calendar();
        assert!(calendar.is_working_day(day("2026-10-20")));
        assert!(!calendar.is_working_day(day("2026-10-24")));
        assert!(!calendar.is_working_day(day("2026-12-25")));
        assert!(!calendar.is_working_day(day("2026-11-06")));
        assert!(Working::weekdays().is_working_day(day("2026-12-25")));

        // Moving by many at once lands where a day at a time would.
        for by in [1, 4, 5, 6, 10, 15, 37, 46, 47, 260, -1, -5, -10, -44, -300] {
            let mut slow = day("2026-10-20");
            for _ in 0..i64::abs(by) {
                slow = slow.add_days(i64::signum(by)).unwrap();
                while !calendar.is_working_day(slow) {
                    slow = slow.add_days(i64::signum(by)).unwrap();
                }
            }
            assert_eq!(calendar.moved(day("2026-10-20"), by, true), Some(slow), "{by}");
            // And from a Sunday, which isn't worked itself.
            let mut slow = day("2026-11-15");
            for _ in 0..i64::abs(by) {
                slow = slow.add_days(i64::signum(by)).unwrap();
                while !calendar.is_working_day(slow) {
                    slow = slow.add_days(i64::signum(by)).unwrap();
                }
            }
            assert_eq!(
                calendar.moved(day("2026-11-15"), by, true),
                Some(slow),
                "{by} from a Sunday"
            );
        }
        assert_eq!(calendar.moved(day("2026-10-20"), 3, false), Some(day("2026-10-23")));
        assert_eq!(calendar.moved(day("2026-10-20"), 10_000_000, true), None);
    }

    #[test]
    fn finds_days_off_by_what_theyre_called() {
        let day = |text| Day::parse(text).unwrap();
        let calendar = calendar();
        let from = day("2026-01-01");
        let find = |words: &[&str], from, to| {
            let said = calendar.naming(words, false);
            said.as_ref()
                .and_then(|said| calendar.find(said, from, to))
                .map(|(day, name)| (day.to_string(), name.to_string()))
        };
        assert_eq!(
            find(&["summer", "bank", "holiday"], from, None),
            Some(("2026-08-31".into(), "Summer bank holiday".into()))
        );
        assert_eq!(
            find(&["bank", "holiday"], from, None),
            Some(("2026-05-04".into(), "Early May bank holiday".into()))
        );
        assert_eq!(
            find(&["bank", "holiday"], day("2026-08-01"), Some(day("2026-08-31")))
                .map(|found| found.0),
            Some("2026-08-31".into())
        );
        assert_eq!(
            find(&["holiday"], day("2026-09-01"), None).map(|found| found.0),
            Some("2026-11-06".into())
        );
        assert_eq!(find(&["day"], from, None), None);
        assert_eq!(find(&["banana"], from, None), None);
    }
}
