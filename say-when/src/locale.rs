//! Where English is being read and written: the order of a date in
//! figures, the day a week starts on, how a day is labelled, and the days
//! that have names there. Everything that differs from one place to
//! another is here or in [`crate::named`], as data, so the rules that read
//! a phrase are the same everywhere.

use crate::day::{Day, Weekday};

/// Where English is being read and written.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[allow(missing_docs)]
pub enum Locale {
    #[default]
    Uk,
    Us,
    Ireland,
    Australia,
    NewZealand,
    Canada,
}

/// Which comes first in a date written in figures, like "3/4".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Order {
    DayFirst,
    MonthFirst,
    /// Both are in common use, so both are read, the month first.
    Either,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A month's name, from 1 for January.
pub(crate) fn month_name(month: u32) -> &'static str {
    MONTHS[(month as usize + 11) % 12]
}

impl Locale {
    /// Every locale there is.
    pub const ALL: [Locale; 6] = [
        Locale::Uk,
        Locale::Us,
        Locale::Ireland,
        Locale::Australia,
        Locale::NewZealand,
        Locale::Canada,
    ];

    pub(crate) fn order(self) -> Order {
        match self {
            Locale::Us => Order::MonthFirst,
            Locale::Canada => Order::Either,
            _ => Order::DayFirst,
        }
    }

    /// The day a week starts on: Sunday in the United States and Canada,
    /// and Monday elsewhere.
    pub fn week_start(self) -> Weekday {
        match self {
            Locale::Us | Locale::Canada => Weekday::Sunday,
            _ => Weekday::Monday,
        }
    }

    /// Whether a day is said with its month first: "November 30".
    fn month_first(self) -> bool {
        matches!(self, Locale::Us | Locale::Canada)
    }

    /// A month's name cut short, from 1 for January. September is "Sept"
    /// in Britain and Australia, as their own style has it.
    fn short_month(self, month: u32) -> &'static str {
        match (self, month) {
            (Locale::Uk | Locale::Australia, 9) => "Sept",
            _ => &month_name(month)[..3],
        }
    }

    /// A day as a list shows it, like "Tue 20 Oct", or "Tue, Oct 20"
    /// where the month's said first, with its year when that isn't the
    /// year it is today.
    pub fn label(self, day: Day, today: Day) -> String {
        let (year, month, date) = day.ymd();
        let weekday = &day.weekday().name()[..3];
        let month = self.short_month(month);
        let mut label = if self.month_first() {
            format!("{weekday}, {month} {date}")
        } else {
            format!("{weekday} {date} {month}")
        };
        if year != today.year() {
            label.push_str(if self.month_first() { ", " } else { " " });
            label.push_str(&year.to_string());
        }
        label
    }

    /// A day of a month said in full, like "9 December" or "December 9".
    pub(crate) fn day_of_month(self, month: u32, date: u32) -> String {
        if self.month_first() {
            format!("{} {date}", month_name(month))
        } else {
            format!("{date} {}", month_name(month))
        }
    }

    /// A day said in full, like "9 December", with its year when that
    /// isn't the year it is today.
    pub(crate) fn long(self, day: Day, today: Option<Day>) -> String {
        let (year, month, date) = day.ymd();
        let said = self.day_of_month(month, date);
        if today.is_some_and(|today| today.year() == year) {
            return said;
        }
        if self.month_first() { format!("{said}, {year}") } else { format!("{said} {year}") }
    }

    /// A day without its weekday, like "17 Nov", with its year when that
    /// isn't the year it is today.
    pub(crate) fn short(self, day: Day, today: Day) -> String {
        let label = self.label(day, today);
        label[if self.month_first() { 5 } else { 4 }..].to_string()
    }

    /// The Monday or Sunday of the week a day is in, as weeks run here.
    pub(crate) fn week_of(self, day: Day) -> Option<Day> {
        day.previous(self.week_start())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_days_each_way() {
        let today = Day::parse("2026-10-20").unwrap();
        let day = Day::parse("2026-09-12").unwrap();
        let later = Day::parse("2028-11-30").unwrap();
        assert_eq!(Locale::Uk.label(day, today), "Sat 12 Sept");
        assert_eq!(Locale::Ireland.label(day, today), "Sat 12 Sep");
        assert_eq!(Locale::Uk.label(later, today), "Thu 30 Nov 2028");
        assert_eq!(Locale::Us.label(day, today), "Sat, Sep 12");
        assert_eq!(Locale::Us.label(later, today), "Thu, Nov 30, 2028");
        assert_eq!(Locale::Canada.label(later, today), "Thu, Nov 30, 2028");
        assert_eq!(Locale::Uk.long(later, Some(today)), "30 November 2028");
        assert_eq!(Locale::Us.long(later, Some(today)), "November 30, 2028");
        assert_eq!(Locale::Uk.short(day, today), "12 Sept");
        assert_eq!(Locale::Us.short(later, today), "Nov 30, 2028");
    }

    #[test]
    fn weeks_start_where_they_do() {
        let tuesday = Day::parse("2026-10-20").unwrap();
        assert_eq!(Locale::Uk.week_of(tuesday).unwrap().to_string(), "2026-10-19");
        assert_eq!(Locale::Us.week_of(tuesday).unwrap().to_string(), "2026-10-18");
    }
}
