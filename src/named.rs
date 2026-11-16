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
    /// The times the caller gave, in order, each from its first day to
    /// its last: the same day for one that's a day long.
    Listed(Vec<(Day, Day)>),
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
    /// The days it's on, first and last, the first time it starts in a
    /// year.
    pub fn span(&self, year: i32) -> Option<(Day, Day)> {
        match &self.falls {
            Falls::On(month, day) => Day::new(year, *month, *day).map(|day| (day, day)),
            Falls::Listed(times) => {
                let first = Day::new(year, 1, 1)?;
                let at = times.partition_point(|(day, _)| *day < first);
                times.get(at).copied().filter(|(day, _)| day.year() == year)
            }
        }
    }

    /// The day it starts on, the first time it does in a year.
    pub fn day(&self, year: i32) -> Option<Day> {
        self.span(year).map(|(first, _)| first)
    }

    /// The first time it's on, or still on, on or after a day: like Ramadan
    /// while it is.
    pub fn on_or_after(&self, from: Day) -> Option<(Day, Day)> {
        match &self.falls {
            Falls::On(..) => {
                let year = from.year();
                (year..=year.saturating_add(8))
                    .find_map(|year| self.span(year).filter(|(day, _)| *day >= from))
            }
            Falls::Listed(times) => {
                times.get(times.partition_point(|(_, last)| *last < from)).copied()
            }
        }
    }

    /// The last year it starts in, if it's not on the same day every year.
    pub fn last_year(&self) -> Option<i32> {
        match &self.falls {
            Falls::On(..) => None,
            Falls::Listed(times) => times.last().map(|(first, _)| first.year()),
        }
    }

    /// Whether it lasts more than a day, any of the times it's on.
    pub fn lasts(&self) -> bool {
        matches!(&self.falls, Falls::Listed(times) if times.iter().any(|(first, last)| first != last))
    }

    /// Whether it's on the same month and day every year: one that's
    /// built in, or one whose every day the caller gave is.
    pub fn fixed(&self) -> Option<(u32, u32)> {
        match &self.falls {
            Falls::On(month, day) => Some((*month, *day)),
            Falls::Listed(times) => {
                let (_, month, date) = times.first()?.0.ymd();
                times
                    .iter()
                    .all(|(day, last)| day == last && (day.month(), day.day()) == (month, date))
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

const BUILT_IN: [BuiltIn; 14] = [
    (&["christmas", "eve"], None, "Christmas Eve", 12, 24),
    (&["christmas"], DAY, "Christmas Day", 12, 25),
    (&["xmas", "eve"], None, "Christmas Eve", 12, 24),
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
    (&["guy", "fawkes"], Some("night"), "Guy Fawkes Night", 11, 5),
];

/// The other names days go by everywhere English is, for a day a
/// calendar gives by the first: a phrase can say any of the rest. Only
/// one way round, as the US Mother's Day isn't Mothering Sunday. A name
/// two days in a calendar would go by, like "Eid", is said for neither.
const ALSO: [(&str, &[&str]); 26] = [
    ("Mothering Sunday", &["Mother’s Day"]),
    ("Easter Sunday", &["Easter", "Easter Day"]),
    ("Easter Day", &["Easter", "Easter Sunday"]),
    ("Shrove Tuesday", &["Pancake Day"]),
    ("Pancake Day", &["Shrove Tuesday"]),
    ("Independence Day", &["Fourth of July", "4th of July", "July 4th"]),
    ("Washington’s Birthday", &["Presidents’ Day", "President’s Day"]),
    ("Presidents’ Day", &["Washington’s Birthday", "President’s Day"]),
    ("Martin Luther King Jr. Day", &["Martin Luther King Day", "MLK Day", "MLK"]),
    ("Birthday of Martin Luther King, Jr.", &["Martin Luther King Day", "MLK Day", "MLK"]),
    ("Juneteenth National Independence Day", &["Juneteenth"]),
    ("Remembrance Day", &["Armistice Day"]),
    ("Labour Day", &["Labor Day"]),
    ("Labor Day", &["Labour Day"]),
    ("Thanksgiving Day", &["Thanksgiving"]),
    ("Thanksgiving", &["Thanksgiving Day"]),
    ("St Stephen’s Day", &["Boxing Day"]),
    ("Diwali", &["Deepavali"]),
    ("Deepavali", &["Diwali"]),
    ("Lunar New Year", &["Chinese New Year"]),
    ("Chinese New Year", &["Lunar New Year"]),
    ("Hanukkah", &["Chanukah", "Hanukah"]),
    ("Passover", &["Pesach"]),
    ("Vaisakhi", &["Baisakhi"]),
    ("Eid al-Fitr", &["Eid"]),
    ("Eid al-Adha", &["Eid"]),
];

/// Each way a name is said: its words, and without its apostrophes, so
/// "mothers day" as well as "mother's day".
fn said(name: &str) -> impl Iterator<Item = Vec<String>> {
    let plain = name.replace(['’', '\''], "");
    [name_words(name), name_words(&plain)].into_iter().flatten()
}

/// The day with a name, said any of the ways it can be.
pub(crate) fn find<'a>(all: &'a [Called], name: &str) -> Option<&'a Called> {
    let says: Vec<Vec<String>> = said(name).collect();
    all.iter()
        .find(|called| same(&called.name, name))
        .or_else(|| all.iter().find(|called| says.contains(&called.words)))
}

/// Whether two names are the same said, whatever their case and
/// apostrophes.
pub(crate) fn same(one: &str, other: &str) -> bool {
    let plain = |name: &str| name_words(&name.replace(['’', '\''], ""));
    plain(one).is_some() && plain(one) == plain(other)
}

/// The first and last days of a time a named day's on.
type Times = (Day, Day);

/// The days built in, and then the caller's: each of a day's names as
/// the words it's said with, its first as it's written back. The
/// caller's are given in any order, and found longest first, so "easter
/// monday" is found before "easter".
pub(crate) fn called<'a>(given: impl Iterator<Item = ((Day, Day), &'a [String])>) -> Vec<Called> {
    let mut found: Vec<Called> = BUILT_IN
        .iter()
        .map(|(words, optional, name, month, day)| Called {
            words: words.iter().map(|word| word.to_string()).collect(),
            optional: *optional,
            name: name.to_string(),
            falls: Falls::On(*month, *day),
        })
        .collect();
    // Each day the caller has, by the first of its names, with every way
    // it's said and every day it's on.
    let mut theirs: Vec<(String, Vec<Vec<String>>, Vec<Times>)> = Vec::new();
    for (day, names) in given {
        let Some(name) = names.first() else { continue };
        let at = match theirs.iter().position(|(written, ..)| same(written, name)) {
            Some(at) => at,
            None => {
                theirs.push((name.clone(), Vec::new(), Vec::new()));
                theirs.len() - 1
            }
        };
        let (_, says, days) = &mut theirs[at];
        days.push(day);
        for words in names.iter().flat_map(|name| said(name)) {
            if !says.contains(&words) {
                says.push(words);
            }
        }
    }
    // Then the other names each goes by, unless a day's said that way
    // already.
    let mut also: Vec<(usize, Vec<String>)> = Vec::new();
    let mut both: Vec<Vec<String>> = Vec::new();
    for (at, (_, says, _)) in theirs.iter().enumerate() {
        let names = ALSO.iter().filter(|(name, _)| said(name).any(|words| says.contains(&words)));
        for words in names.flat_map(|(_, others)| others.iter().flat_map(|other| said(other))) {
            if theirs.iter().any(|(_, says, _)| says.contains(&words)) {
                continue;
            }
            match also.iter().find(|(_, said)| *said == words) {
                Some((of, _)) if *of != at => both.push(words),
                Some(_) => {}
                None => also.push((at, words)),
            }
        }
    }
    also.retain(|(_, words)| !both.contains(words));
    let mut listed: Vec<Called> = Vec::new();
    let mut others: Vec<Called> = Vec::new();
    for (at, (name, says, days)) in theirs.iter_mut().enumerate() {
        days.sort();
        days.dedup();
        let falls = Falls::Listed(days.clone());
        for words in says.drain(..) {
            listed.push(Called { words, optional: None, name: name.clone(), falls: falls.clone() });
        }
        for (_, words) in also.iter().filter(|(of, _)| *of == at) {
            others.push(Called {
                words: words.clone(),
                optional: None,
                name: name.clone(),
                falls: falls.clone(),
            });
        }
    }
    listed.sort_by_key(|day| std::cmp::Reverse(day.words.len()));
    others.sort_by_key(|day| std::cmp::Reverse(day.words.len()));
    found.extend(listed);
    found.extend(others);
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
        let given = [
            ((day("2027-03-07"), day("2027-03-07")), &names[..]),
            ((day("2026-03-15"), day("2026-03-15")), &names[..]),
        ];
        let all = called(given.into_iter());
        let mothers: Vec<_> = all.iter().filter(|named| named.name == "Mother’s Day").collect();
        let words: Vec<String> = mothers.iter().map(|named| named.words.join(" ")).collect();
        assert_eq!(words, ["mother day", "mothers day", "mothering sunday"]);
        assert_eq!(mothers[0].day(2026), Some(day("2026-03-15")));
        assert_eq!(mothers[0].day(2027), Some(day("2027-03-07")));
        assert_eq!(mothers[0].day(2028), None);
        assert_eq!(mothers[0].fixed(), None);

        let independence = ["Independence Day".to_string()];
        let given = [
            ((day("2026-07-04"), day("2026-07-04")), &independence[..]),
            ((day("2027-07-04"), day("2027-07-04")), &independence[..]),
        ];
        let all = called(given.into_iter());
        assert_eq!(all.last().unwrap().fixed(), Some((7, 4)));
    }

    #[test]
    fn says_a_day_by_the_other_names_it_goes_by() {
        let day = |text| Day::parse(text).unwrap();
        let mothering = ["Mothering Sunday".to_string()];
        let mothers = ["Mother's Day".to_string()];
        let words = |all: &[Called]| -> Vec<String> {
            all.iter()
                .filter(|named| matches!(named.falls, Falls::Listed(_)))
                .map(|named| format!("{}: {}", named.words.join(" "), named.name))
                .collect()
        };
        // Given one way and then another, it's the same day.
        let given = [
            ((day("2026-03-15"), day("2026-03-15")), &mothering[..]),
            ((day("2027-03-07"), day("2027-03-07")), &mothering[..]),
        ];
        let all = called(given.into_iter());
        assert_eq!(
            words(&all),
            [
                "mothering sunday: Mothering Sunday",
                "mother day: Mothering Sunday",
                "mothers day: Mothering Sunday"
            ]
        );
        assert_eq!(all.last().unwrap().day(2027), Some(day("2027-03-07")));
        // A day said that way already keeps it.
        let given = [
            ((day("2026-03-15"), day("2026-03-15")), &mothering[..]),
            ((day("2026-05-10"), day("2026-05-10")), &mothers[..]),
        ];
        let all = called(given.into_iter());
        assert_eq!(
            words(&all),
            [
                "mothering sunday: Mothering Sunday",
                "mother day: Mother's Day",
                "mothers day: Mother's Day"
            ]
        );
        // And the US Mother's Day isn't Mothering Sunday.
        let all = called([((day("2026-05-10"), day("2026-05-10")), &mothers[..])].into_iter());
        assert_eq!(words(&all), ["mother day: Mother's Day", "mothers day: Mother's Day"]);
        // Spelt with other apostrophes or capitals, it's one day.
        let other = ["mother’s day".to_string()];
        let given = [
            ((day("2026-05-10"), day("2026-05-10")), &mothers[..]),
            ((day("2027-05-09"), day("2027-05-09")), &other[..]),
        ];
        let all = called(given.into_iter());
        assert_eq!(all.last().unwrap().day(2027), Some(day("2027-05-09")));
        // "Eid" is Eid al-Fitr, unless there's Eid al-Adha too.
        let fitr = ["Eid al-Fitr".to_string()];
        let adha = ["Eid al-Adha".to_string()];
        let all = called([((day("2026-03-20"), day("2026-03-20")), &fitr[..])].into_iter());
        assert!(all.iter().any(|named| named.words == ["eid"]));
        let given = [
            ((day("2026-03-20"), day("2026-03-20")), &fitr[..]),
            ((day("2026-05-27"), day("2026-05-27")), &adha[..]),
        ];
        let all = called(given.into_iter());
        assert!(!all.iter().any(|named| named.words == ["eid"]));
    }

    #[test]
    fn keeps_how_long_a_day_lasts() {
        let day = |text| Day::parse(text).unwrap();
        let ramadan = ["Ramadan".to_string()];
        let given = [
            ((day("2027-02-08"), day("2027-03-09")), &ramadan[..]),
            ((day("2026-02-18"), day("2026-03-19")), &ramadan[..]),
        ];
        let all = called(given.into_iter());
        let ramadan = all.last().unwrap();
        assert!(ramadan.lasts());
        assert_eq!(ramadan.fixed(), None);
        assert_eq!(ramadan.span(2027), Some((day("2027-02-08"), day("2027-03-09"))));
        // While it's on, it's the one that's on.
        assert_eq!(
            ramadan.on_or_after(day("2026-03-01")),
            Some((day("2026-02-18"), day("2026-03-19")))
        );
        assert_eq!(
            ramadan.on_or_after(day("2026-03-20")).map(|(first, _)| first),
            Some(day("2027-02-08"))
        );
        assert_eq!(ramadan.on_or_after(day("2027-03-10")), None);
    }
}
