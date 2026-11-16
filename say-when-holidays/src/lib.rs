//! Public holidays, worked out by rule, for the places it has rules for,
//! as calendars for `say-when`, which has none of its own: the days
//! people don't work are set by governments, and change.
//!
//! That's why this is a crate of its own, with a version of its own. A new
//! holiday, or one moved for a year, is a release of this and nothing
//! else, and a caller with a better source, like a list kept on its own
//! server, needn't use it at all.
//!
//! What it has: the bank holidays of England and Wales, Scotland and
//! Northern Ireland, as gov.uk lists them, and the federal holidays of the
//! United States. Each is right from the year it says, and one-off days,
//! like a coronation, are listed by hand, so the years ahead are only as
//! right as the rules are now.

use say_when::{Calendar, Day, DayOff, Weekday, easter};
use std::ops::RangeInclusive;

/// Whose public holidays.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum Region {
    EnglandAndWales,
    Scotland,
    NorthernIreland,
    /// The federal holidays, which states add to.
    UnitedStates,
}

/// A year, a month and a day of the month.
type Date = (i32, u32, u32);

fn date((year, month, day): Date) -> Option<Day> {
    Day::new(year, month, day)
}

/// The nth of a day of the week in a month, or its last for -1.
fn nth(year: i32, month: u32, weekday: Weekday, nth: i64) -> Option<Day> {
    let first = Day::new(year, month, 1)?;
    if nth < 0 {
        first.month_end().previous(weekday)
    } else {
        first.next(weekday)?.add_days(7 * (nth - 1))
    }
}

// The United Kingdom's days off that were moved or added for a year, from
// gov.uk's list.
const MOVED: [(Date, Date); 2] = [((2020, 5, 4), (2020, 5, 8)), ((2022, 5, 30), (2022, 6, 2))];
const EXTRA: [(Date, &str); 3] = [
    ((2022, 6, 3), "Platinum Jubilee bank holiday"),
    ((2022, 9, 19), "Queen’s state funeral"),
    ((2023, 5, 8), "Coronation bank holiday"),
];

/// A year's bank holidays in part of the United Kingdom. When one falls
/// at a weekend, the next weekday that isn't already a holiday is taken
/// off instead, as gov.uk has it.
fn united_kingdom(year: i32, region: Region) -> Vec<DayOff> {
    let Some(easter) = easter(year) else { return Vec::new() };
    let on = |month, day| Day::new(year, month, day);
    let scotland = region == Region::Scotland;
    let northern_ireland = region == Region::NorthernIreland;

    // Those that move with Easter or to a Monday never fall at a weekend.
    let mut taken: Vec<(Day, String)> = [
        (easter.add_days(-2), "Good Friday", true),
        (easter.add_days(1), "Easter Monday", !scotland),
        (nth(year, 5, Weekday::Monday, 1), "Early May bank holiday", true),
        (nth(year, 5, Weekday::Monday, -1), "Spring bank holiday", true),
        (nth(year, 8, Weekday::Monday, if scotland { 1 } else { -1 }), "Summer bank holiday", true),
    ]
    .into_iter()
    .filter(|(_, _, counts)| *counts)
    .filter_map(|(day, name, _)| Some((day?, name.to_string())))
    .collect();

    let fixed = [
        (on(1, 1), "New Year’s Day", true),
        (on(1, 2), "2nd January", scotland),
        (on(3, 17), "St Patrick’s Day", northern_ireland),
        (on(7, 12), "Battle of the Boyne", northern_ireland),
        (on(11, 30), "St Andrew’s Day", scotland),
        (on(12, 25), "Christmas Day", true),
        (on(12, 26), "Boxing Day", true),
    ];
    // Those on weekdays keep their day, then those at a weekend take the
    // next one free, so Christmas on a Sunday moves past Boxing Day.
    for weekend in [false, true] {
        for (day, name, counts) in fixed {
            let Some(day) = day.filter(|day| counts && day.weekday().is_weekend() == weekend)
            else {
                continue;
            };
            let mut free = day;
            while free.weekday().is_weekend() || taken.iter().any(|(other, _)| *other == free) {
                let Some(next) = free.add_days(1) else { break };
                free = next;
            }
            let name =
                if free == day { name.to_string() } else { format!("{name} (substitute day)") };
            taken.push((free, name));
        }
    }

    let moved = |day: Day| {
        MOVED
            .iter()
            .find(|(from, _)| date(*from) == Some(day))
            .and_then(|(_, to)| date(*to))
            .unwrap_or(day)
    };
    let extra = EXTRA
        .iter()
        .filter(|(day, _)| day.0 == year)
        .filter_map(|(day, name)| Some((date(*day)?, name.to_string())));
    taken.into_iter().map(|(day, name)| (moved(day), name)).chain(extra).map(named).collect()
}

fn named((day, name): (Day, String)) -> DayOff {
    DayOff { day, name: Some(name) }
}

/// The federal holidays that fall in a year, each on the day it's
/// observed: the Friday before one on a Saturday, and the Monday after one
/// on a Sunday. So a year can end with the next one's New Year's Day.
fn united_states(year: i32) -> Vec<DayOff> {
    use Weekday::{Monday, Thursday};
    let of = |year: i32| {
        let on = |month, day| Day::new(year, month, day);
        [
            (on(1, 1), "New Year’s Day", true),
            (nth(year, 1, Monday, 3), "Martin Luther King Jr. Day", year >= 1986),
            (nth(year, 2, Monday, 3), "Presidents’ Day", true),
            (nth(year, 5, Monday, -1), "Memorial Day", true),
            (on(6, 19), "Juneteenth", year >= 2021),
            (on(7, 4), "Independence Day", true),
            (nth(year, 9, Monday, 1), "Labor Day", true),
            (nth(year, 10, Monday, 2), "Columbus Day", true),
            (on(11, 11), "Veterans Day", true),
            (nth(year, 11, Thursday, 4), "Thanksgiving", true),
            (on(12, 25), "Christmas Day", true),
        ]
        .into_iter()
        .filter(|(_, _, counts)| *counts)
        .filter_map(|(day, name, _)| {
            let day = day?;
            let observed = match day.weekday() {
                Weekday::Saturday => day.add_days(-1)?,
                Weekday::Sunday => day.add_days(1)?,
                _ => day,
            };
            let name =
                if observed == day { name.to_string() } else { format!("{name} (observed)") };
            Some((observed, name))
        })
        .collect::<Vec<_>>()
    };
    let mut found = of(year);
    found.extend(of(year + 1));
    found.into_iter().filter(|(day, _)| day.year() == year).map(named).collect()
}

/// The public holidays that fall in a year somewhere, in order.
pub fn holidays(year: i32, region: Region) -> Vec<DayOff> {
    let mut found = match region {
        Region::UnitedStates => united_states(year),
        _ => united_kingdom(year, region),
    };
    found.sort_by_key(|off| off.day);
    found
}

/// A calendar of somewhere's public holidays over some years, with Monday
/// to Friday worked, for a reader to count working days by and to read
/// their names with.
pub fn calendar(region: Region, years: RangeInclusive<i32>) -> Calendar {
    Calendar {
        days_off: years.flat_map(|year| holidays(year, region)).collect(),
        ..Calendar::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn days(year: i32, region: Region) -> Vec<String> {
        holidays(year, region).iter().map(|off| off.day.to_string()).collect()
    }

    fn name(year: i32, region: Region, day: &str) -> Option<String> {
        holidays(year, region)
            .into_iter()
            .find(|off| off.day.to_string() == day)
            .and_then(|off| off.name)
    }

    #[test]
    fn lists_the_united_kingdoms_bank_holidays_as_gov_uk_does() {
        assert_eq!(
            days(2026, Region::EnglandAndWales),
            [
                "2026-01-01",
                "2026-04-03",
                "2026-04-06",
                "2026-05-04",
                "2026-05-25",
                "2026-08-31",
                "2026-12-25",
                "2026-12-28"
            ]
        );
        assert_eq!(
            days(2026, Region::Scotland),
            [
                "2026-01-01",
                "2026-01-02",
                "2026-04-03",
                "2026-05-04",
                "2026-05-25",
                "2026-08-03",
                "2026-11-30",
                "2026-12-25",
                "2026-12-28"
            ]
        );
        assert_eq!(
            days(2026, Region::NorthernIreland),
            [
                "2026-01-01",
                "2026-03-17",
                "2026-04-03",
                "2026-04-06",
                "2026-05-04",
                "2026-05-25",
                "2026-07-13",
                "2026-08-31",
                "2026-12-25",
                "2026-12-28"
            ]
        );
    }

    #[test]
    fn moves_a_holiday_at_a_weekend_past_the_ones_after_it() {
        // Christmas on a Sunday: Boxing Day keeps the Monday, and Christmas
        // takes the Tuesday.
        let region = Region::EnglandAndWales;
        assert_eq!(name(2022, region, "2022-12-26").as_deref(), Some("Boxing Day"));
        assert_eq!(
            name(2022, region, "2022-12-27").as_deref(),
            Some("Christmas Day (substitute day)")
        );
        assert_eq!(name(2022, region, "2022-06-02").as_deref(), Some("Spring bank holiday"));
        assert_eq!(
            name(2022, region, "2022-06-03").as_deref(),
            Some("Platinum Jubilee bank holiday")
        );
        assert_eq!(name(2022, region, "2022-05-30"), None);
    }

    #[test]
    fn lists_the_united_states_federal_holidays_as_observed() {
        // As the Office of Personnel Management lists 2026 and 2027.
        assert_eq!(
            days(2026, Region::UnitedStates),
            [
                "2026-01-01",
                "2026-01-19",
                "2026-02-16",
                "2026-05-25",
                "2026-06-19",
                "2026-07-03",
                "2026-09-07",
                "2026-10-12",
                "2026-11-11",
                "2026-11-26",
                "2026-12-25"
            ]
        );
        assert_eq!(
            name(2026, Region::UnitedStates, "2026-07-03").as_deref(),
            Some("Independence Day (observed)")
        );
        assert_eq!(
            days(2027, Region::UnitedStates),
            [
                "2027-01-01",
                "2027-01-18",
                "2027-02-15",
                "2027-05-31",
                "2027-06-18",
                "2027-07-05",
                "2027-09-06",
                "2027-10-11",
                "2027-11-11",
                "2027-11-25",
                "2027-12-24",
                "2027-12-31"
            ]
        );
        // New Year's Day 2028 is a Saturday, so it's observed in 2027.
        assert_eq!(
            name(2027, Region::UnitedStates, "2027-12-31").as_deref(),
            Some("New Year’s Day (observed)")
        );
        assert!(!days(2020, Region::UnitedStates).contains(&"2020-06-19".to_string()));
    }

    #[test]
    fn makes_a_calendar_a_reader_counts_working_days_by() {
        use say_when::{Locale, Options, Reader};
        let reader = Reader::new(Options::new(Locale::Uk))
            .with_calendar(calendar(Region::EnglandAndWales, 2026..=2027));
        let today = Day::new(2026, 10, 20).unwrap();
        let read = reader.days("3 working days after christmas eve", today);
        assert_eq!(read[0].day.to_string(), "2026-12-31");
        let read = reader.days("the august bank holiday", Day::new(2027, 1, 1).unwrap());
        assert_eq!(
            (read[0].day.to_string().as_str(), read[0].how.as_str()),
            ("2027-08-30", "Summer bank holiday")
        );
    }
}

/// The README's examples, run as tests.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
