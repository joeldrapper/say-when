//! The days people call by name. A few fall on the same day every year
//! and are said everywhere English is: Christmas, New Year, Valentine's
//! Day, St Patrick's Day, Halloween. Those are built in, like the names of
//! the months.
//!
//! The rest are the caller's to give, in its [`Calendar`]'s named days: which are said
//! depends on where (Thanksgiving is in November in America and October in
//! Canada, and not at all in Britain), some move from year to year (Easter
//! and the days around it, Mother's Day), and whoever's reading may have
//! days of their own. A caller gives each as the days it falls on, with
//! what it's called, so the library needn't know how any of them is
//! worked out, and [`easter`] is here to help work some out.
//!
//! [`Calendar`]: crate::Calendar

use crate::day::Day;
use crate::words::name_words;

/// Easter Sunday, by the anonymous Gregorian algorithm.
pub fn easter(year: i32) -> Option<Day> {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    Day::new(year, month as u32, day as u32)
}

/// The days a named day falls on.
#[derive(Clone, Debug)]
pub(crate) enum Falls {
    /// The same month and day every year.
    On(u32, u32),
    /// The days the caller gave, in order.
    Listed(Vec<Day>),
}

/// A day with a name, found by its words.
#[derive(Clone, Debug)]
pub(crate) struct Called {
    pub words: Vec<String>,
    /// A word that can follow, like "day" after "christmas".
    pub optional: Option<&'static str>,
    pub name: String,
    pub falls: Falls,
}

impl Called {
    /// The day it falls on in a year, the first if there are more.
    pub fn day(&self, year: i32) -> Option<Day> {
        match &self.falls {
            Falls::On(month, day) => Day::new(year, *month, *day),
            Falls::Listed(days) => {
                let first = Day::new(year, 1, 1)?;
                let at = days.partition_point(|day| *day < first);
                days.get(at).copied().filter(|day| day.year() == year)
            }
        }
    }

    /// Whether it's on the same month and day every year: one that's
    /// built in, or one whose every day the caller gave is.
    pub fn fixed(&self) -> Option<(u32, u32)> {
        match &self.falls {
            Falls::On(month, day) => Some((*month, *day)),
            Falls::Listed(days) => {
                let (_, month, date) = days.first()?.ymd();
                days.iter()
                    .all(|day| (day.month(), day.day()) == (month, date))
                    .then_some((month, date))
            }
        }
    }
}

const DAY: Option<&str> = Some("day");

/// The days with names said everywhere, on the same day every year. One
/// whose words start another's comes after it, so "christmas eve" is found
/// before "christmas". A possessive has lost its "'s" by the time it's
/// read, so "new year's eve" is "new year eve".
/// A day built in: its words, a word that can follow them, its name, and
/// its month and day.
type BuiltIn = (&'static [&'static str], Option<&'static str>, &'static str, u32, u32);

const BUILT_IN: [BuiltIn; 12] = [
    (&["christmas", "eve"], None, "Christmas Eve", 12, 24),
    (&["christmas"], DAY, "Christmas Day", 12, 25),
    (&["xmas"], DAY, "Christmas Day", 12, 25),
    (&["new", "year", "eve"], None, "New Year’s Eve", 12, 31),
    (&["new", "years", "eve"], None, "New Year’s Eve", 12, 31),
    (&["new", "year"], DAY, "New Year’s Day", 1, 1),
    (&["new", "years"], DAY, "New Year’s Day", 1, 1),
    (&["valentine"], DAY, "Valentine’s Day", 2, 14),
    (&["valentines"], DAY, "Valentine’s Day", 2, 14),
    (&["st", "patrick"], DAY, "St Patrick’s Day", 3, 17),
    (&["st", "patricks"], DAY, "St Patrick’s Day", 3, 17),
    (&["halloween"], None, "Halloween", 10, 31),
];

/// The days built in, and then the caller's: each of a day's names as
/// the words it's said with, its first as it's written back. The
/// caller's are given in any order, and found longest first, so "easter
/// monday" is found before "easter".
pub(crate) fn called<'a>(given: impl Iterator<Item = (Day, &'a [String])>) -> Vec<Called> {
    let mut found: Vec<Called> = BUILT_IN
        .iter()
        .map(|(words, optional, name, month, day)| Called {
            words: words.iter().map(|word| word.to_string()).collect(),
            optional: *optional,
            name: name.to_string(),
            falls: Falls::On(*month, *day),
        })
        .collect();
    // Each name the caller has, with every day it's on.
    let mut theirs: Vec<(String, Vec<Vec<String>>, Vec<Day>)> = Vec::new();
    for (day, names) in given {
        let Some(name) = names.first() else { continue };
        match theirs.iter_mut().find(|(written, ..)| written == name) {
            Some((_, _, days)) => days.push(day),
            None => {
                let mut said: Vec<Vec<String>> = Vec::new();
                for each in names {
                    // "mothers day" as well as "mother's day".
                    let plain = each.replace(['’', '\''], "");
                    for words in [name_words(each), name_words(&plain)].into_iter().flatten() {
                        if !said.contains(&words) {
                            said.push(words);
                        }
                    }
                }
                theirs.push((name.clone(), said, vec![day]));
            }
        }
    }
    let mut listed: Vec<Called> = Vec::new();
    for (name, said, mut days) in theirs {
        days.sort();
        days.dedup();
        for words in said {
            listed.push(Called {
                words,
                optional: None,
                name: name.clone(),
                falls: Falls::Listed(days.clone()),
            });
        }
    }
    listed.sort_by_key(|day| std::cmp::Reverse(day.words.len()));
    found.extend(listed);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn works_out_easter() {
        assert_eq!(easter(2026).unwrap().to_string(), "2026-04-05");
        assert_eq!(easter(2027).unwrap().to_string(), "2027-03-28");
        assert_eq!(easter(2038).unwrap().to_string(), "2038-04-25");
    }

    #[test]
    fn keeps_the_days_a_caller_gives() {
        let day = |text| Day::parse(text).unwrap();
        let names = ["Mother’s Day".to_string(), "Mothering Sunday".to_string()];
        let given = [(day("2027-03-07"), &names[..]), (day("2026-03-15"), &names[..])];
        let all = called(given.into_iter());
        let mothers: Vec<_> = all.iter().filter(|named| named.name == "Mother’s Day").collect();
        let words: Vec<String> = mothers.iter().map(|named| named.words.join(" ")).collect();
        assert_eq!(words, ["mother day", "mothers day", "mothering sunday"]);
        assert_eq!(mothers[0].day(2026), Some(day("2026-03-15")));
        assert_eq!(mothers[0].day(2027), Some(day("2027-03-07")));
        assert_eq!(mothers[0].day(2028), None);
        assert_eq!(mothers[0].fixed(), None);

        let independence = ["Independence Day".to_string()];
        let given =
            [(day("2026-07-04"), &independence[..]), (day("2027-07-04"), &independence[..])];
        let all = called(given.into_iter());
        assert_eq!(all.last().unwrap().fixed(), Some((7, 4)));
    }
}
