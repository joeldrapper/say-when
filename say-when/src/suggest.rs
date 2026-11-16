//! The days some typed words might mean, for a date picker's search field,
//! as it's typed: "tom", "fri", "next week", "9 dec", "3d", "3rd wed jan",
//! "10d from dec 1". Any word can be half typed, so "fr" is Friday and "t"
//! is Today, Tomorrow, Tuesday and Thursday, each offered by name. Never a
//! day that's already gone, so "jan 5" is next January.
//!
//! These are the short forms, which know half-typed words best. The day
//! reader ([`crate::days`]) reads whatever else a longer phrase might
//! mean, and what both find is offered together, these first.

use crate::clock::figures;
use crate::day::{Day, Weekday};
use crate::days::plural;
use crate::locale::{Locale, month_name};
use crate::options::{MONTH_WORDS, WEEKDAY_WORDS};
use crate::words::{Token, tokens};

/// A day found, with a name when the day's own label won't do, like "In 3
/// days" or "Friday next week".
pub(crate) type Named = (Day, Option<String>);

/// How long a unit is: so many days, or a month or a year.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Length {
    Days(i64),
    Month,
    Year,
}

fn unit(word: &str) -> Option<(Length, &'static str)> {
    Some(match word {
        "d" | "day" | "days" => (Length::Days(1), "day"),
        "w" | "wk" | "week" | "weeks" => (Length::Days(7), "week"),
        "mo" | "month" | "months" => (Length::Month, "month"),
        "y" | "yr" | "year" | "years" => (Length::Year, "year"),
        _ => return None,
    })
}

fn span(from: Day, n: i64, length: Length) -> Option<Day> {
    match length {
        Length::Days(days) => from.add_days(n.checked_mul(days)?),
        Length::Month => from.add_months(n),
        Length::Year => from.add_months(n.checked_mul(12)?),
    }
}

/// The words in a list that start with what's typed, by their place in it.
fn starting(word: &str, list: &[&str]) -> Vec<usize> {
    if word.is_empty() {
        return Vec::new();
    }
    list.iter()
        .enumerate()
        .filter(|(_, name)| name.starts_with(word))
        .map(|(index, _)| index)
        .collect()
}

/// The days of the week that start with what's typed, from Sunday.
fn weekdays(word: &str) -> Vec<Weekday> {
    let names: Vec<&str> = WEEKDAY_WORDS.iter().map(|(name, _)| *name).collect();
    starting(word, &names).into_iter().map(|index| WEEKDAY_WORDS[index].1).collect()
}

/// The months that start with what's typed, from 1 for January.
fn months(word: &str) -> Vec<u32> {
    starting(word, &MONTH_WORDS).into_iter().map(|index| index as u32 + 1).collect()
}

fn letters(word: &str) -> bool {
    !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// A count and a word run together or a space apart, like "3d" or "10
/// days", as they are.
fn counted(text: &str) -> Option<(&str, &str)> {
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let (count, word) = text.split_at(digits);
    let word = word.strip_prefix(' ').unwrap_or(word);
    (digits > 0 && letters(word)).then_some((count, word))
}

fn ordinal(word: &str) -> Option<i64> {
    Some(match word {
        "1st" | "first" => 1,
        "2nd" | "second" => 2,
        "3rd" | "third" => 3,
        "4th" | "fourth" => 4,
        "5th" | "fifth" => 5,
        "last" => -1,
        _ => return None,
    })
}

fn capitalise(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
}

pub(crate) struct Suggester {
    pub today: Day,
    pub locale: Locale,
}

impl Suggester {
    /// The next time a month and day come round, today or later.
    fn next_month_day(&self, month: u32, date: u32) -> Option<Day> {
        let year = self.today.year();
        (year..year + 5)
            .find_map(|year| Day::new(year, month, date).filter(|day| *day >= self.today))
    }

    /// The nth of a day of the week in a month, or its last for -1.
    fn nth_weekday(&self, year: i32, month: u32, weekday: Weekday, n: i64) -> Option<Day> {
        let first = Day::new(year, month, 1)?;
        if n == -1 {
            return first.month_end().previous(weekday);
        }
        first.next(weekday)?.add_days(7 * (n - 1)).filter(|day| day.month() == month)
    }

    /// Every day some words on their own might mean. `text` is in lower
    /// case, its words a space apart.
    pub fn days(&self, text: &str) -> Vec<Named> {
        let today = self.today;
        let mut found: Vec<Named> = Vec::new();
        let mut push = |day: Option<Day>, name: Option<String>| {
            if let Some(day) = day.filter(|day| *day >= today) {
                found.push((day, name));
            }
        };
        let words: Vec<&str> = text.split(' ').collect();
        let (a, b, c) = (words[0], words.get(1).copied(), words.get(2).copied());

        // "10d from dec 1", "dec 1 + 10d".
        let sides = [" from ", " after "]
            .iter()
            .filter_map(|joiner| text.find(joiner).map(|at| (at, joiner.len(), false)))
            .min()
            .or_else(|| {
                text.char_indices().skip(1).find(|(_, c)| *c == '+').map(|(at, _)| (at, 1, true))
            });
        if let Some((at, size, plus)) = sides {
            let (before, after) = (text[..at].trim_end(), text[at + size..].trim_start());
            let (length, base) = if plus { (after, before) } else { (before, after) };
            if !after.is_empty() {
                let from = self.days(base).first().map(|(day, _)| *day);
                let said = counted(length)
                    .and_then(|(count, word)| Some((count.parse::<i64>().ok()?, unit(word)?)));
                if let (Some(from), Some((count, (length, name)))) = (from, said) {
                    push(span(from, count, length), Some(format!("{} after", plural(count, name))));
                }
                return found;
            }
        }

        // "in 3 days", "3d", "2 weeks", "6mo".
        let rest = text.strip_prefix("in ").unwrap_or(text);
        if let Some((count, (length, name))) =
            counted(rest).and_then(|(count, word)| Some((count.parse::<i64>().ok()?, unit(word)?)))
        {
            push(span(today, count, length), Some(format!("In {}", plural(count, name))));
            return found;
        }

        // "two weeks", "in three days", "a month", and half typed, "two we".
        if let [Token::Number(n, _), Token::Word(typed)] = &tokens(rest)[..]
            && let Some(n) = i64::try_from(*n).ok().filter(|n| *n > 0)
        {
            let mut any = false;
            for (word, length, name) in [
                ("days", Length::Days(1), "day"),
                ("weeks", Length::Days(7), "week"),
                ("months", Length::Month, "month"),
                ("years", Length::Year, "year"),
            ] {
                if word.starts_with(typed.as_str()) {
                    push(span(today, n, length), Some(format!("In {}", plural(n, name))));
                    any = true;
                }
            }
            if any {
                return found;
            }
        }

        // "9/12", "9-12", "9.12", the day or the month first as it's written here
        // has it, and "2026-12-09".
        let parts: Vec<&str> = text.split(['/', '.', '-']).collect();
        let part = |index: usize, most: usize| {
            parts
                .get(index)
                .filter(|part| (1..=most).contains(&part.len()))
                .and_then(|part| part.parse::<u32>().ok())
        };
        let numeric = parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
        if numeric
            && parts.len() == 3
            && parts[0].len() == 4
            && !text.contains(['/', '.'])
            && let (Some(year), Some(month), Some(date)) = (part(0, 4), part(1, 2), part(2, 2))
        {
            push(Day::new(year as i32, month, date), None);
            return found;
        }
        if numeric
            && (2..=3).contains(&parts.len())
            && matches!(parts.get(2).map_or(2, |year| year.len()), 2 | 4)
            && let (Some(first), Some(second)) = (part(0, 2), part(1, 2))
        {
            // Each way round it's written here, or the only way that's a date.
            let year = parts.get(2).and_then(|year| year.parse::<i32>().ok());
            for (date, month) in crate::days::numeric(first, second, self.locale) {
                match year {
                    Some(year) => push(
                        Day::new(if parts[2].len() == 2 { 2000 + year } else { year }, month, date),
                        None,
                    ),
                    None => push(self.next_month_day(month, date), None),
                }
            }
            return found;
        }

        // A bare number, or one written as a word: that day of the month
        // the next time it comes round, then in that many days, weeks,
        // months and years. "31st" is the day only, the next three times
        // it comes round, in months that have it.
        let as_word = match &tokens(text)[..] {
            [Token::Number(n, said)] if said != "a" && said != "an" => i64::try_from(*n).ok(),
            _ => None,
        };
        let suffixed = ["st", "nd", "rd", "th"].iter().find_map(|suffix| text.strip_suffix(suffix));
        let digits = suffixed.unwrap_or(text);
        let in_figures = ((1..=3).contains(&digits.len())
            && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse::<i64>().ok())
        .flatten();
        if let Some(n) = as_word.or(in_figures) {
            let is_ordinal = as_word.is_none() && suffixed.is_some();
            if (1..=31).contains(&n) {
                let first = today.month_start();
                let days = (0..12).filter_map(|ahead| {
                    let (year, month, _) = first.add_months(ahead)?.ymd();
                    Day::new(year, month, n as u32).filter(|day| *day >= today)
                });
                for day in days.take(if is_ordinal { 3 } else { 1 }) {
                    push(Some(day), None);
                }
            }
            if n > 0 && !is_ordinal {
                for (length, name) in [
                    (Length::Days(1), "day"),
                    (Length::Days(7), "week"),
                    (Length::Month, "month"),
                    (Length::Year, "year"),
                ] {
                    push(span(today, n, length), Some(format!("In {}", plural(n, name))));
                }
            }
            return found;
        }

        // "3rd wed jan", "last fri nov".
        if let (Some(nth), Some(b)) = (ordinal(a), b.filter(|b| !weekdays(b).is_empty())) {
            let year = today.year();
            for weekday in weekdays(b) {
                let of = match c {
                    Some(c) => months(c),
                    None => (0..3)
                        .filter_map(|ahead| Some(today.month_start().add_months(ahead)?.month()))
                        .collect(),
                };
                for month in of {
                    let day = [year, year + 1].into_iter().find_map(|year| {
                        self.nth_weekday(year, month, weekday, nth).filter(|day| *day >= today)
                    });
                    let name =
                        format!("{} {} of {}", capitalise(a), weekday.name(), month_name(month));
                    push(day, Some(name));
                }
            }
            return found;
        }

        // "next fri", "fri next week".
        let next_week = a == "next" && b.is_some_and(|b| !weekdays(b).is_empty());
        let week_after = b == Some("next")
            && c.is_some_and(|c| "week".starts_with(c))
            && !weekdays(a).is_empty();
        if next_week || week_after {
            let monday = today.next_after(self.locale.week_start());
            for weekday in weekdays(if next_week { b.unwrap_or_default() } else { a }) {
                let day = monday.and_then(|first| first.next(weekday));
                push(day, Some(format!("{} next week", weekday.name())));
            }
            return found;
        }

        // "wed dec": every Wednesday in December.
        if let (2, true, Some(b)) = (
            words.len(),
            a.len() >= 2 && !weekdays(a).is_empty(),
            b.filter(|b| !months(b).is_empty()),
        ) {
            for weekday in weekdays(a) {
                for month in months(b) {
                    let mut at =
                        self.next_month_day(month, 1).and_then(|first| first.next(weekday));
                    while let Some(day) = at.filter(|day| day.month() == month) {
                        push(Some(day), None);
                        at = day.add_days(7);
                    }
                }
            }
            return found;
        }

        // "9 dec", "dec 9", "9dec", "dec9".
        let dated = counted(text).or_else(|| {
            let digits = text.len() - text.trim_end_matches(|c: char| c.is_ascii_digit()).len();
            let (word, count) = text.split_at(text.len() - digits);
            let word = word.strip_suffix(' ').unwrap_or(word);
            (digits > 0 && letters(word)).then_some((count, word))
        });
        if let Some((n, name)) = dated.and_then(|(count, word)| Some((figures(count)?, word))) {
            for month in months(name) {
                push(self.next_month_day(month, n), None);
            }
            return found;
        }

        // "next week", "next month", "next year".
        if let ("next", Some(b)) = (a, b) {
            // Next week starts on Monday, and a week today is often meant too.
            if "week".starts_with(b) {
                push(today.next_after(self.locale.week_start()), Some("Next week".to_string()));
                push(today.add_days(7), Some("In a week".to_string()));
            }
            if "month".starts_with(b) {
                push(today.month_start().add_months(1), Some("Next month".to_string()));
            }
            if "year".starts_with(b) {
                push(Day::new(today.year() + 1, 1, 1), Some("Next year".to_string()));
            }
            return found;
        }

        if words.len() == 1 && !a.is_empty() {
            if "today".starts_with(a) {
                push(Some(today), Some("Today".to_string()));
            }
            if "tomorrow".starts_with(a) {
                push(today.add_days(1), Some("Tomorrow".to_string()));
            }
            if a.len() >= 2 && "weekend".starts_with(a) {
                push(today.next(Weekday::Saturday), Some("This weekend".to_string()));
            }
            // The next one after today, as today has its own name.
            for weekday in weekdays(a) {
                push(today.next_after(weekday), Some(weekday.name().to_string()));
            }
            if a.len() >= 3 {
                for month in months(a) {
                    push(self.next_month_day(month, 1), Some(month_name(month).to_string()));
                }
            }
        }
        found
    }
}

/// What's typed, as it's searched: in lower case, with commas as spaces,
/// and one space between words.
pub(crate) fn searched(typed: &str) -> String {
    typed.to_lowercase().replace(',', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words a typo in which is put right, besides the months.
const DATE_WORDS: [&str; 21] = [
    "today",
    "tomorrow",
    "weekend",
    "next",
    "week",
    "weeks",
    "month",
    "months",
    "year",
    "years",
    "days",
    "before",
    "after",
    "from",
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

/// Whether a typed word is one letter off the start of a word: one
/// swapped, missed, extra or wrong.
fn one_off(typed: &[char], word: &[char]) -> bool {
    if typed.len() < 4 {
        return false;
    }
    (typed.len() - 1..=typed.len() + 1)
        .any(|length| length <= word.len() && within_one(typed, &word[..length]))
}

fn within_one(a: &[char], b: &[char]) -> bool {
    if a == b {
        return true;
    }
    let same = a.iter().zip(b).take_while(|(a, b)| a == b).count();
    match a.len().cmp(&b.len()) {
        core::cmp::Ordering::Equal => {
            a[same + 1..] == b[same + 1..]
                || (same + 1 < a.len()
                    && a[same] == b[same + 1]
                    && a[same + 1] == b[same]
                    && a[same + 2..] == b[same + 2..])
        }
        core::cmp::Ordering::Greater => a.len() - b.len() == 1 && a[same + 1..] == b[same..],
        core::cmp::Ordering::Less => b.len() - a.len() == 1 && a[same..] == b[same + 1..],
    }
}

/// What's typed with each word that's one letter off a word dates are
/// said with put right, like "tmorrow" for tomorrow, or `None` when no
/// word is. A word in `keep`, like someone's name, is left as it is, one
/// in `more` is put right too, and one in `long` when nearly all of it was
/// typed.
pub(crate) fn corrected(
    typed: &str,
    more: &[&str],
    long: &[&str],
    keep: &[&str],
) -> Option<String> {
    let text = searched(typed);
    let vocabulary: Vec<&str> = more.iter().copied().chain(DATE_WORDS).chain(MONTH_WORDS).collect();
    let fixed: Vec<String> = text
        .split(' ')
        .map(|word| {
            let kept = keep.contains(&word);
            if kept || vocabulary.iter().chain(long).any(|each| each.starts_with(word)) {
                return word.to_string();
            }
            let letters: Vec<char> = word.chars().collect();
            let near = |each: &&&str| {
                let each: Vec<char> = each.chars().collect();
                each.len() + 1 >= letters.len() && one_off(&letters, &each)
            };
            // A day's own name, like "thanksgiving", only when most of it
            // was typed, as a short word is one letter off the start of
            // many: "call" isn't Halloween.
            let whole = |each: &&&str| letters.len() + 1 >= each.len();
            let meant =
                vocabulary.iter().find(near).or_else(|| long.iter().filter(whole).find(near));
            meant.map_or_else(|| word.to_string(), |meant| meant.to_string())
        })
        .collect();
    let fixed = fixed.join(" ");
    (fixed != text).then_some(fixed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_right_a_word_one_letter_off() {
        let fix = |typed: &str| {
            corrected(typed, &["birthday"], &["thanksgiving", "halloween"], &["mark"])
        };
        assert_eq!(fix("tmorrow").as_deref(), Some("tomorrow"));
        assert_eq!(fix("tomorow").as_deref(), Some("tomorrow"));
        assert_eq!(fix("next fridya").as_deref(), Some("next friday"));
        assert_eq!(fix("9 decmber").as_deref(), Some("9 december"));
        assert_eq!(fix("mums brithday").as_deref(), Some("mums birthday"));
        assert_eq!(fix("mark"), None);
        assert_eq!(fix("bananas"), None);
        assert_eq!(fix("tom"), None);
        assert_eq!(fix("thanksgving").as_deref(), Some("thanksgiving"));
        assert_eq!(fix("call"), None);
        assert_eq!(fix("hallo"), None);
    }
}
