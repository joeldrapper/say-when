//! The days people call by name: Christmas Eve, Easter Monday, Mother's
//! Day, Thanksgiving. They're vocabulary, like the names of the months:
//! each falls by a rule that doesn't change, and which are said, and when
//! some fall, depends on where (Mother's Day is in March in Britain and in
//! May in America). They say nothing of who's working: a day that's off is
//! the caller's to say (see [`crate::calendar`]).

use crate::day::{Day, Weekday};
use crate::locale::Locale;

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

/// The day a named day falls on in a year.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Falls {
    /// The same month and day every year.
    On(u32, u32),
    /// So many days after Easter Sunday.
    AfterEaster(i64),
    /// So many days after the nth of a day of the week in a month, or its
    /// last for -1: the 4th Thursday of November.
    Nth(u32, Weekday, i64, i64),
    /// The last of a day of the week before a day of a month.
    Before(u32, u32, Weekday),
}

/// A day with a name, found by its words.
#[derive(Debug)]
pub(crate) struct NamedDay {
    pub words: &'static [&'static str],
    /// A word that can follow, like "day" after "christmas".
    pub optional: Option<&'static str>,
    pub name: &'static str,
    pub falls: Falls,
    /// Where it's said, or everywhere when nowhere's given.
    pub locales: &'static [Locale],
}

impl NamedDay {
    pub fn said_in(&self, locale: Locale) -> bool {
        self.locales.is_empty() || self.locales.contains(&locale)
    }

    pub fn day(&self, year: i32) -> Option<Day> {
        match self.falls {
            Falls::On(month, day) => Day::new(year, month, day),
            Falls::AfterEaster(days) => easter(year)?.add_days(days),
            Falls::Nth(month, weekday, nth, after) => {
                let first = Day::new(year, month, 1)?;
                let day = if nth < 0 {
                    first.month_end().previous(weekday)?
                } else {
                    first.next(weekday)?.add_days(7 * (nth - 1))?
                };
                day.add_days(after)
            }
            Falls::Before(month, day, weekday) => {
                Day::new(year, month, day)?.previous_before(weekday)
            }
        }
    }

    /// Whether it's on the same month and day every year.
    pub fn fixed(&self) -> Option<(u32, u32)> {
        match self.falls {
            Falls::On(month, day) => Some((month, day)),
            _ => None,
        }
    }
}

use Locale::{Australia, Canada, Ireland, NewZealand, Uk, Us};
use Weekday::{Monday, Sunday, Thursday};

const fn day(
    words: &'static [&'static str],
    optional: Option<&'static str>,
    name: &'static str,
    falls: Falls,
    locales: &'static [Locale],
) -> NamedDay {
    NamedDay { words, optional, name, falls, locales }
}

const EVERYWHERE: &[Locale] = &[];
const DAY: Option<&str> = Some("day");

/// The days with names. One whose words start another's comes after it,
/// so "christmas eve" is found before "christmas". A possessive has lost
/// its "'s" by the time it's read, so "mother's day" is "mother day".
pub(crate) const NAMED_DAYS: &[NamedDay] = &[
    day(&["christmas", "eve"], None, "Christmas Eve", Falls::On(12, 24), EVERYWHERE),
    day(&["christmas"], DAY, "Christmas Day", Falls::On(12, 25), EVERYWHERE),
    day(&["xmas"], DAY, "Christmas Day", Falls::On(12, 25), EVERYWHERE),
    day(&["new", "year", "eve"], None, "New Year’s Eve", Falls::On(12, 31), EVERYWHERE),
    day(&["new", "years", "eve"], None, "New Year’s Eve", Falls::On(12, 31), EVERYWHERE),
    day(&["new", "year"], DAY, "New Year’s Day", Falls::On(1, 1), EVERYWHERE),
    day(&["new", "years"], DAY, "New Year’s Day", Falls::On(1, 1), EVERYWHERE),
    day(&["valentine"], DAY, "Valentine’s Day", Falls::On(2, 14), EVERYWHERE),
    day(&["valentines"], DAY, "Valentine’s Day", Falls::On(2, 14), EVERYWHERE),
    day(&["st", "patrick"], DAY, "St Patrick’s Day", Falls::On(3, 17), EVERYWHERE),
    day(&["st", "patricks"], DAY, "St Patrick’s Day", Falls::On(3, 17), EVERYWHERE),
    day(&["halloween"], None, "Halloween", Falls::On(10, 31), EVERYWHERE),
    day(&["good", "friday"], None, "Good Friday", Falls::AfterEaster(-2), EVERYWHERE),
    day(&["easter", "monday"], None, "Easter Monday", Falls::AfterEaster(1), EVERYWHERE),
    day(&["easter"], Some("sunday"), "Easter Sunday", Falls::AfterEaster(0), EVERYWHERE),
    // Not said in the United States.
    day(
        &["boxing", "day"],
        None,
        "Boxing Day",
        Falls::On(12, 26),
        &[Uk, Ireland, Australia, NewZealand, Canada],
    ),
    day(&["st", "stephen"], DAY, "St Stephen’s Day", Falls::On(12, 26), &[Ireland]),
    day(&["st", "stephens"], DAY, "St Stephen’s Day", Falls::On(12, 26), &[Ireland]),
    day(&["st", "brigid"], DAY, "St Brigid’s Day", Falls::On(2, 1), &[Ireland]),
    day(&["bonfire", "night"], None, "Bonfire Night", Falls::On(11, 5), &[Uk]),
    day(&["may", "day"], None, "May Day", Falls::On(5, 1), &[Uk, Ireland]),
    day(
        &["remembrance", "day"],
        None,
        "Remembrance Day",
        Falls::On(11, 11),
        &[Uk, Ireland, Australia, NewZealand, Canada],
    ),
    // Mothering Sunday in Britain and Ireland, three weeks before Easter,
    // and the second Sunday of May elsewhere.
    day(&["mother", "day"], None, "Mother’s Day", Falls::AfterEaster(-21), &[Uk, Ireland]),
    day(&["mothers", "day"], None, "Mother’s Day", Falls::AfterEaster(-21), &[Uk, Ireland]),
    day(&["mothering", "sunday"], None, "Mother’s Day", Falls::AfterEaster(-21), &[Uk, Ireland]),
    day(
        &["mother", "day"],
        None,
        "Mother’s Day",
        Falls::Nth(5, Sunday, 2, 0),
        &[Us, Canada, Australia, NewZealand],
    ),
    day(
        &["mothers", "day"],
        None,
        "Mother’s Day",
        Falls::Nth(5, Sunday, 2, 0),
        &[Us, Canada, Australia, NewZealand],
    ),
    // The third Sunday of June, or the first of September in Australia
    // and New Zealand.
    day(
        &["father", "day"],
        None,
        "Father’s Day",
        Falls::Nth(6, Sunday, 3, 0),
        &[Uk, Ireland, Us, Canada],
    ),
    day(
        &["fathers", "day"],
        None,
        "Father’s Day",
        Falls::Nth(6, Sunday, 3, 0),
        &[Uk, Ireland, Us, Canada],
    ),
    day(
        &["father", "day"],
        None,
        "Father’s Day",
        Falls::Nth(9, Sunday, 1, 0),
        &[Australia, NewZealand],
    ),
    day(
        &["fathers", "day"],
        None,
        "Father’s Day",
        Falls::Nth(9, Sunday, 1, 0),
        &[Australia, NewZealand],
    ),
    // The United States.
    day(&["thanksgiving"], DAY, "Thanksgiving", Falls::Nth(11, Thursday, 4, 0), &[Us]),
    day(&["black", "friday"], None, "Black Friday", Falls::Nth(11, Thursday, 4, 1), &[Us]),
    day(&["memorial", "day"], None, "Memorial Day", Falls::Nth(5, Monday, -1, 0), &[Us]),
    day(&["labor", "day"], None, "Labor Day", Falls::Nth(9, Monday, 1, 0), &[Us]),
    day(&["labour", "day"], None, "Labor Day", Falls::Nth(9, Monday, 1, 0), &[Us]),
    day(&["independence", "day"], None, "Independence Day", Falls::On(7, 4), &[Us]),
    day(
        &["martin", "luther", "king", "jr"],
        DAY,
        "Martin Luther King Jr. Day",
        Falls::Nth(1, Monday, 3, 0),
        &[Us],
    ),
    day(
        &["martin", "luther", "king"],
        DAY,
        "Martin Luther King Jr. Day",
        Falls::Nth(1, Monday, 3, 0),
        &[Us],
    ),
    day(&["mlk"], DAY, "Martin Luther King Jr. Day", Falls::Nth(1, Monday, 3, 0), &[Us]),
    day(&["presidents", "day"], None, "Presidents’ Day", Falls::Nth(2, Monday, 3, 0), &[Us]),
    day(&["president", "day"], None, "Presidents’ Day", Falls::Nth(2, Monday, 3, 0), &[Us]),
    day(&["juneteenth"], None, "Juneteenth", Falls::On(6, 19), &[Us]),
    day(&["veterans", "day"], None, "Veterans Day", Falls::On(11, 11), &[Us]),
    day(&["veteran", "day"], None, "Veterans Day", Falls::On(11, 11), &[Us]),
    day(&["columbus", "day"], None, "Columbus Day", Falls::Nth(10, Monday, 2, 0), &[Us]),
    // Canada.
    day(&["thanksgiving"], DAY, "Thanksgiving", Falls::Nth(10, Monday, 2, 0), &[Canada]),
    day(&["canada", "day"], None, "Canada Day", Falls::On(7, 1), &[Canada]),
    day(&["victoria", "day"], None, "Victoria Day", Falls::Before(5, 25, Monday), &[Canada]),
    day(&["labour", "day"], None, "Labour Day", Falls::Nth(9, Monday, 1, 0), &[Canada]),
    day(&["labor", "day"], None, "Labour Day", Falls::Nth(9, Monday, 1, 0), &[Canada]),
    // Australia and New Zealand.
    day(&["australia", "day"], None, "Australia Day", Falls::On(1, 26), &[Australia]),
    day(&["anzac", "day"], None, "Anzac Day", Falls::On(4, 25), &[Australia, NewZealand]),
    day(&["waitangi", "day"], None, "Waitangi Day", Falls::On(2, 6), &[NewZealand]),
    day(&["labour", "day"], None, "Labour Day", Falls::Nth(10, Monday, 4, 0), &[NewZealand]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn on(name: &str, locale: Locale, year: i32) -> Option<String> {
        NAMED_DAYS
            .iter()
            .find(|day| day.name == name && day.said_in(locale))
            .and_then(|day| day.day(year))
            .map(|day| day.to_string())
    }

    #[test]
    fn works_out_easter() {
        assert_eq!(easter(2026).unwrap().to_string(), "2026-04-05");
        assert_eq!(easter(2027).unwrap().to_string(), "2027-03-28");
        assert_eq!(easter(2038).unwrap().to_string(), "2038-04-25");
    }

    #[test]
    fn the_same_name_falls_where_its_said() {
        assert_eq!(on("Mother’s Day", Uk, 2027).as_deref(), Some("2027-03-07"));
        assert_eq!(on("Mother’s Day", Us, 2027).as_deref(), Some("2027-05-09"));
        assert_eq!(on("Father’s Day", Uk, 2027).as_deref(), Some("2027-06-20"));
        assert_eq!(on("Father’s Day", Australia, 2027).as_deref(), Some("2027-09-05"));
        assert_eq!(on("Thanksgiving", Us, 2026).as_deref(), Some("2026-11-26"));
        assert_eq!(on("Thanksgiving", Canada, 2026).as_deref(), Some("2026-10-12"));
        assert_eq!(on("Thanksgiving", Uk, 2026), None);
        assert_eq!(on("Boxing Day", Us, 2026), None);
    }

    #[test]
    fn works_out_days_that_move() {
        assert_eq!(on("Black Friday", Us, 2026).as_deref(), Some("2026-11-27"));
        assert_eq!(on("Memorial Day", Us, 2026).as_deref(), Some("2026-05-25"));
        assert_eq!(on("Labor Day", Us, 2026).as_deref(), Some("2026-09-07"));
        assert_eq!(on("Martin Luther King Jr. Day", Us, 2027).as_deref(), Some("2027-01-18"));
        assert_eq!(on("Presidents’ Day", Us, 2027).as_deref(), Some("2027-02-15"));
        assert_eq!(on("Victoria Day", Canada, 2026).as_deref(), Some("2026-05-18"));
        assert_eq!(on("Labour Day", NewZealand, 2026).as_deref(), Some("2026-10-26"));
        assert_eq!(on("Good Friday", Us, 2026).as_deref(), Some("2026-04-03"));
    }
}
