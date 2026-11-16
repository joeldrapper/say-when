//! Reads how something repeats, written out in plain English:
//!
//! - every so often: "daily", "every day", "every 3 days", "every fourth
//!   day", "alternate days", "once a fortnight", "weekly", "fortnightly",
//!   "every other week", "every 3 months", "half yearly", "yearly"
//! - on some weekdays: "every tue", "tuesdays", "mon wed and fri", "every
//!   other thursday", "alternate fridays", "every sixth tuesday",
//!   "weekdays", "weekends", "working days", "every day except sunday",
//!   "weekdays but friday"
//! - on days of the month: "the 15th", "1st and 15th of every month", "the
//!   last day of the month", "the 2nd tuesday", "the last working day"
//! - on a day of the year: "every 9 dec", "yearly on mum's birthday",
//!   "every christmas", "the last friday in october every year"
//! - after it's done: "3 weeks after done", "a month after it's done"
//!
//! each with when it starts or stops, if it does: "from next monday",
//! "until christmas", "for 6 weeks", "10 times".
//!
//! A phrase is a body, which says the rule, then any number of tails,
//! which say when it starts and stops. Every reading that uses every word
//! is kept, so "every other tue" is both this week's Tuesday on and next
//! week's.

use crate::calendar::Working;
use crate::day::{Day, Weekday};
use crate::days::{Context, DayExpr, Parser, Unit};
use crate::offset::Shift;
use crate::options::{WEEKDAY_WORDS, weekday_alias};
use crate::rule::{ByDay, Frequency, Rule};
use crate::words::Token;

/// Whether something repeats on a schedule, whatever's been done, or so
/// long after it was last done.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[allow(missing_docs)]
pub enum RepeatKind {
    Schedule,
    After,
}

/// How something repeats: the kind, the rule, and the first day it's on,
/// or for one after it's done, the day it's counted from.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Repeat {
    /// On a schedule, or after it's done.
    pub kind: RepeatKind,
    /// The rule. One after it's done only says how often and every how
    /// many.
    pub rule: Rule,
    /// The first day it's on.
    pub starts: Day,
    /// The days it doesn't come round on after all, in any order, as
    /// iCalendar's `EXDATE` has them: they still count towards how many
    /// times it does.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub skipped: Vec<Day>,
}

impl Repeat {
    /// One on a schedule, from its first day.
    pub fn schedule(rule: Rule, starts: Day) -> Repeat {
        Repeat { kind: RepeatKind::Schedule, rule, starts, skipped: Vec::new() }
    }

    /// One that comes round so long after it's done, counted from a day.
    pub fn after_done(rule: Rule, from: Day) -> Repeat {
        Repeat { kind: RepeatKind::After, rule, starts: from, skipped: Vec::new() }
    }
}

const WORKING: [Weekday; 5] =
    [Weekday::Monday, Weekday::Tuesday, Weekday::Wednesday, Weekday::Thursday, Weekday::Friday];

/// What a rule leaves out, to be filled in from the day it starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fill {
    Weekday,
    MonthDay,
    YearDay,
}

/// What a phrase says before when it starts and stops.
#[derive(Clone, Debug)]
struct Body {
    at: usize,
    kind: RepeatKind,
    rule: Rule,
    fill: Option<Fill>,
}

/// When it starts and stops.
#[derive(Clone, Copy, Debug, Default)]
struct Tail {
    at: usize,
    starts: Option<Day>,
    until: Option<Day>,
    count: Option<i64>,
    lasts: Option<(i64, Unit)>,
}

impl Tail {
    fn stops(&self) -> bool {
        self.until.is_some() || self.count.is_some() || self.lasts.is_some()
    }
}

/// Some days of the week said together, and whether they're working days,
/// which bank holidays aren't.
#[derive(Clone, Debug)]
struct Weekdays {
    days: Vec<Weekday>,
    working: bool,
    at: usize,
}

/// The days of a month a phrase picks out.
#[derive(Clone, Debug)]
enum MonthDays {
    /// "the 1st and 15th", "the last day".
    Dates(Vec<i32>),
    /// "the 2nd tuesday", "the last friday".
    Nth(ByDay),
    /// "the last working day", "the first weekday".
    Position { days: Vec<Weekday>, position: i32, working: bool },
}

impl MonthDays {
    fn apply(&self, mut rule: Rule) -> Rule {
        match self {
            MonthDays::Dates(dates) => rule.by_month_day = dates.clone(),
            MonthDays::Nth(by_day) => rule.by_day = vec![*by_day],
            MonthDays::Position { days, position, working } => {
                rule.by_day = days.iter().copied().map(ByDay::every).collect();
                rule.by_set_pos = Some(*position);
                rule.skips_days_off = *working;
            }
        }
        rule
    }

    fn weekdays(&self) -> bool {
        !matches!(self, MonthDays::Dates(_))
    }
}

fn frequency(unit: Unit) -> Option<Frequency> {
    match unit {
        Unit::Day => Some(Frequency::Daily),
        Unit::Week => Some(Frequency::Weekly),
        Unit::Month => Some(Frequency::Monthly),
        Unit::Year => Some(Frequency::Yearly),
        // Working days and weekdays are counted in, not repeated by.
        Unit::WorkingDay | Unit::Weekday => None,
    }
}

/// What a rule that's every so often of a unit takes from the day it starts.
fn fill(unit: Frequency) -> Option<Fill> {
    match unit {
        Frequency::Daily => None,
        Frequency::Weekly => Some(Fill::Weekday),
        Frequency::Monthly => Some(Fill::MonthDay),
        Frequency::Yearly => Some(Fill::YearDay),
    }
}

fn every(unit: Frequency, interval: i64) -> Option<Rule> {
    Some(Rule { interval: u32::try_from(interval).ok().filter(|n| *n >= 1)?, ..Rule::every(unit) })
}

pub(crate) struct RepeatParser<'a> {
    pub parser: Parser<'a>,
    pub context: Context<'a>,
}

impl RepeatParser<'_> {
    fn is(&self, at: usize, words: &[&str]) -> bool {
        self.parser.is(at, words)
    }

    /// A day of the week, or several said as one: "tue", "tuesdays",
    /// "weekdays", "weekends", "working days".
    fn weekdays(&self, at: usize) -> Vec<Weekdays> {
        let Some(text) = self.parser.word(at) else { return Vec::new() };
        let mut found = Vec::new();
        let mut add = |days: &[Weekday], working, at| {
            found.push(Weekdays { days: days.to_vec(), working, at })
        };
        // Not "week", which is every week, though it starts both.
        if self.is(at, &["weekday", "weekdays"]) && text != "week" {
            add(&WORKING, false, at + 1);
        }
        if self.is(at, &["weekend", "weekends"]) && text != "week" {
            add(&[Weekday::Saturday, Weekday::Sunday], false, at + 1);
        }
        // The days the calendar works, which needn't be Monday to Friday.
        let working = self.context.calendar.working_week();
        if self.is(at, &["working", "work", "business"]) && self.is(at + 1, &["day", "days"]) {
            add(&working, true, at + 2);
        }
        if self.is(at, &["workday", "workdays"]) {
            add(&working, true, at + 1);
        }
        // "tue", "tues", "tuesday", "tuesdays", or half typed, "tu".
        let one = if text.len() > 3 { text.strip_suffix('s').unwrap_or(text) } else { text };
        let least = if self.parser.last(at) { 2 } else { 3 };
        let weekday = weekday_alias(text).or_else(|| weekday_alias(one)).or_else(|| {
            let mut named = WEEKDAY_WORDS
                .iter()
                .filter(|(name, _)| *name == one || (one.len() >= least && name.starts_with(one)));
            let (_, weekday) = named.next()?;
            named.next().is_none().then_some(*weekday)
        });
        if let Some(weekday) = weekday {
            add(&[weekday], false, at + 1);
        }
        found
    }

    /// Several days of the week: "mon wed and fri", "tue, thu".
    fn weekday_list(&self, at: usize) -> Vec<Weekdays> {
        let mut found = Vec::new();
        for first in self.weekdays(at) {
            found.push(first.clone());
            // Not "weekdays and weekends".
            let [only] = first.days[..] else { continue };
            let next = if self.is(first.at, &["and", "&"]) { first.at + 1 } else { first.at };
            for more in self.weekday_list(next) {
                if !more.working
                    && !more.days.contains(&only)
                    && (more.days.len() == 1 || more.at > next + 1)
                {
                    let days = [&[only][..], &more.days].concat();
                    found.push(Weekdays { days, working: false, at: more.at });
                }
            }
        }
        found
    }

    /// Some days of the week, with any left out after them: "weekdays but
    /// friday", "every day except weekends", "except for sat and sun". Each
    /// with where it ends, and as they are when nothing's left out. Never
    /// all of them, nor a day that wasn't there to leave out.
    fn except(&self, days: Vec<Weekday>, at: usize) -> Vec<(Vec<Weekday>, usize)> {
        let mut found = vec![(days.clone(), at)];
        let from = if self.is(at, &["except", "excluding", "but", "bar", "not"]) {
            let more = self.is(at, &["except", "but"]) && self.is(at + 1, &["for", "not"]);
            if more { at + 2 } else { at + 1 }
        } else if self.is(at, &["apart", "other"]) && self.is(at + 1, &["from", "than"]) {
            at + 2
        } else {
            return found;
        };
        // Nothing after it yet, as it's still being typed.
        if from >= self.parser.tokens.len() {
            found.push((days, from));
            return found;
        }
        for out in self.weekday_list(from) {
            if out.working || !out.days.iter().all(|day| days.contains(day)) {
                continue;
            }
            let left: Vec<Weekday> =
                days.iter().copied().filter(|day| !out.days.contains(day)).collect();
            if !left.is_empty() {
                found.push((left, out.at));
            }
        }
        found
    }

    /// How many of a unit: "day", "3 weeks", "a fortnight", "other month".
    fn every(&self, at: usize) -> Vec<(i64, Frequency, usize)> {
        let other = self.is(at, &["other", "alternate"])
            || matches!(self.parser.tokens.get(at), Some(Token::Ordinal(2)));
        let n = if other { Some(2) } else { self.parser.number(at) };
        let from = if n.is_some() { at + 1 } else { at };
        self.parser
            .unit(from)
            .into_iter()
            .filter_map(|(unit, times, end)| {
                Some((n.unwrap_or(1).checked_mul(times)?, frequency(unit)?, end))
            })
            .collect()
    }

    /// "daily", "weekly", "fortnightly", "monthly", "quarterly", "yearly".
    fn adverb(&self, at: usize) -> Vec<(i64, Frequency, usize)> {
        // "biweekly" and "bimonthly" can mean twice a week or a month, but
        // that's no rule without the days, so they're every two; twice a
        // year can be, and is every six months.
        const SAID: [(&[&str], i64, Frequency); 10] = [
            (&["daily", "nightly"], 1, Frequency::Daily),
            (&["weekly"], 1, Frequency::Weekly),
            (&["fortnightly", "biweekly"], 2, Frequency::Weekly),
            (&["monthly"], 1, Frequency::Monthly),
            (&["bimonthly"], 2, Frequency::Monthly),
            (&["quarterly"], 3, Frequency::Monthly),
            (&["biannually", "biannual", "semiannually", "semiannual"], 6, Frequency::Monthly),
            (&["yearly", "annually"], 1, Frequency::Yearly),
            (&["biennially", "biennial"], 2, Frequency::Yearly),
            (&["triennially", "triennial"], 3, Frequency::Yearly),
        ];
        let mut found: Vec<_> = SAID
            .iter()
            .filter(|(words, ..)| self.is(at, words))
            .map(|(_, n, unit)| (*n, *unit, at + 1))
            .collect();
        // "half yearly", "half-yearly", "six monthly": every six months.
        let half = self.is(at, &["half"]) && self.is(at + 1, &["yearly"]);
        let six = self.parser.number(at) == Some(6) && self.is(at + 1, &["monthly"]);
        if half || six {
            found.push((6, Frequency::Monthly, at + 2));
        }
        found
    }

    /// "the 1st and 15th", "15th", "the last day".
    fn dates(&self, at: usize) -> Vec<(Vec<i32>, usize)> {
        let mut found = Vec::new();
        if let Some(Token::Ordinal(n @ 1..=31)) = self.parser.tokens.get(at) {
            let n = *n as i32;
            found.push((vec![n], at + 1));
            let next = if self.is(at + 1, &["and", "&"]) { at + 2 } else { at + 1 };
            for (more, end) in self.dates(next) {
                found.push(([&[n][..], &more].concat(), end));
            }
        }
        if self.is(at, &["last"]) && self.parser.exactly(at + 1, "day") {
            found.push((vec![-1], at + 2));
        }
        found
    }

    /// Days of the month: "the 15th", "1st and 15th", "the last day", "the
    /// 2nd tuesday", "the last working day", "the first weekday".
    fn month_days(&self, at: usize) -> Vec<(MonthDays, usize)> {
        let mut found = Vec::new();
        for (mut dates, end) in self.dates(at) {
            if self.weekdays(end).is_empty() && !self.is(end, &["day", "days"]) {
                let mut seen = Vec::new();
                dates.retain(|date| {
                    !seen.contains(date) && {
                        seen.push(*date);
                        true
                    }
                });
                found.push((MonthDays::Dates(dates), end));
            }
        }
        if let Some((nth, end)) = self.parser.ordinal(at)
            && nth.abs() <= 5
        {
            let nth = nth as i32;
            for weekdays in self.weekdays(end) {
                // Not "the 3rd weekend": a position picks one day, the 3rd
                // weekend day, and no rule picks a whole weekend each
                // month, as "the 3rd saturday and sunday" can be in two.
                if weekdays.days == [Weekday::Saturday, Weekday::Sunday] {
                    continue;
                }
                let days = match weekdays.days[..] {
                    [weekday] => MonthDays::Nth(ByDay { weekday, nth }),
                    _ => MonthDays::Position {
                        days: weekdays.days,
                        position: nth,
                        working: weekdays.working,
                    },
                };
                found.push((days, weekdays.at));
            }
        }
        found
    }

    /// "of every month", "of the month", "each month", "monthly", "every
    /// month", or nothing: where each ends.
    fn of_month(&self, at: usize) -> Vec<usize> {
        let mut ends = vec![at];
        let of = if self.is(at, &["in"]) { at + 1 } else { at };
        if self.is(of, &["every", "each"]) && self.is(of + 1, &["month"]) {
            ends.push(of + 2);
        }
        if self.is(of, &["month", "monthly"]) {
            ends.push(of + 1);
        }
        ends
    }

    /// "every year", "each year", "yearly", or nothing when "every" was
    /// said before the day, as in "every 9 dec": where each ends.
    fn every_year(&self, at: usize, said: bool) -> Vec<usize> {
        let mut ends = if said { vec![at] } else { Vec::new() };
        if self.is(at, &["every", "each"]) && self.is(at + 1, &["year"]) {
            ends.push(at + 2);
        }
        if self.is(at, &["yearly", "annually"]) {
            ends.push(at + 1);
        }
        ends
    }

    /// A day of the year, the same each year: "9 dec", "mum's birthday",
    /// "christmas". Not one that moves, like Easter, nor one in a year of
    /// its own, like "9 dec 2027".
    fn year_days(&self, at: usize) -> Vec<(u32, u32, usize)> {
        let mut found = Vec::new();
        for (expr, end) in self.parser.atom(at) {
            let on = match expr {
                DayExpr::Date { month, day, year: crate::days::YearRef::Coming } => {
                    Some((month, day))
                }
                DayExpr::Holiday { index, year: crate::days::YearRef::Coming } => {
                    self.context.calendar.named.get(index).and_then(|named| named.fixed())
                }
                DayExpr::Named { index, year: crate::days::YearRef::Coming } => {
                    self.context.named.get(index).map(|named| (named.month, named.day))
                }
                _ => None,
            };
            // A day there's never been, like 31 February, isn't one.
            if let Some((month, day)) =
                on.filter(|(month, day)| Day::new(2000, *month, *day).is_some())
            {
                found.push((month, day, end));
            }
        }
        found
    }

    /// What it says before when it starts and stops.
    fn bodies(&self, at: usize) -> Vec<Body> {
        let mut found = Vec::new();
        // "once a week", "once a fortnight", "once per month", "once every
        // 3 weeks": every so often, and no more than that, so not "once a
        // tuesday".
        let once = self.is(at, &["once"]).then(|| match self.parser.tokens.get(at + 1) {
            Some(Token::Number(1, _)) => Some(at + 1),
            _ if self.is(at + 1, &["per"]) => Some(at + 2),
            _ if self.is(at + 1, &["every", "each"]) => Some(at + 1),
            _ => None,
        });
        if let Some(once) = once {
            let Some(from) = once else { return found };
            if self.is(from, &["every", "each"]) {
                return self.bodies(from);
            }
            return self
                .every(from)
                .into_iter()
                .filter_map(|(n, unit, end)| {
                    let fill = fill(unit);
                    Some(Body { at: end, kind: RepeatKind::Schedule, rule: every(unit, n)?, fill })
                })
                .collect();
        }
        let said = self.is(at, &["every", "each", "on"]);
        // "alternate days", "on alternate tuesdays": "every other", with
        // "every" left unsaid.
        let alternate = !said && self.is(at, &["alternate"]);
        let lead = if said { at + 1 } else { at };
        let schedule =
            |rule: Rule, at: usize| Body { at, kind: RepeatKind::Schedule, rule, fill: None };

        // "every tue", "every other thursday", "tuesdays", "mon wed fri",
        // "weekdays", "every third tuesday", "every weekday but friday".
        let nth = match self.parser.tokens.get(lead) {
            Some(Token::Ordinal(n @ 2..)) => i64::try_from(*n).ok(),
            _ => None,
        };
        let other = self.is(lead, &["other", "alternate"]);
        let counted = if other { Some(2) } else { nth.or(self.parser.number(lead)) }
            .filter(|_| said || alternate);
        let days_at = if counted.is_some() { lead + 1 } else { lead };
        // The 3rd to 5th Tuesday is a day of each month too, and that's
        // what "every third tuesday" says first, as "the third tuesday" of
        // each month; every third week on Tuesday is the one after it.
        let monthly_first = nth.is_some_and(|n| (3..=5).contains(&n));
        let mut weeks_apart = Vec::new();
        if let Some(weekly) = every(Frequency::Weekly, counted.unwrap_or(1)) {
            for days in self.weekday_list(days_at) {
                // Every third weekday is every third working day, if
                // anything, not every three weeks from Monday to Friday.
                let weekdays = days.working || days.days.len() == WORKING.len();
                if nth.is_some_and(|n| n > 2) && weekdays {
                    continue;
                }
                for (left, at) in self.except(days.days, days.at) {
                    let rule = Rule {
                        by_day: left.into_iter().map(ByDay::every).collect(),
                        skips_days_off: days.working,
                        ..weekly.clone()
                    };
                    if monthly_first {
                        weeks_apart.push(schedule(rule, at));
                    } else {
                        found.push(schedule(rule, at));
                    }
                }
            }
        }

        // "every 3 weeks", "every other month", "daily", each optionally
        // on some days: "every 2 weeks on tue", "monthly on the 15th".
        let often = if said || alternate { self.every(lead) } else { Vec::new() };
        for (n, unit, end) in often.into_iter().chain(self.adverb(at)) {
            let Some(base) = every(unit, n) else { continue };
            // "every day except sunday", "daily but weekends": the days of
            // the week that are left.
            if unit == Frequency::Daily && n == 1 {
                let week = [&WORKING[..], &[Weekday::Saturday, Weekday::Sunday]].concat();
                for (days, at) in self.except(week.clone(), end) {
                    // With nothing left out yet, as it's typed, every day.
                    let rule = if days == week {
                        base.clone()
                    } else {
                        Rule {
                            by_day: days.into_iter().map(ByDay::every).collect(),
                            ..Rule::every(Frequency::Weekly)
                        }
                    };
                    if at > end {
                        found.push(schedule(rule, at));
                    }
                }
            }
            found.push(Body {
                at: end,
                kind: RepeatKind::Schedule,
                rule: base.clone(),
                fill: fill(unit),
            });
            // "on" was left out with the other words that always can be.
            match unit {
                Frequency::Daily => {}
                Frequency::Weekly => {
                    for days in self.weekday_list(end) {
                        let rule = Rule {
                            by_day: days.days.iter().copied().map(ByDay::every).collect(),
                            skips_days_off: days.working,
                            ..base.clone()
                        };
                        found.push(schedule(rule, days.at));
                    }
                }
                Frequency::Monthly => {
                    for (days, end) in self.month_days(end) {
                        found.push(schedule(days.apply(base.clone()), end));
                    }
                }
                Frequency::Yearly => {
                    // "every year on mother's day".
                    for (index, end) in self.parser.holidays(end) {
                        match self.context.calendar.named.get(index) {
                            Some(named) if named.fixed().is_none() => {
                                let on = Some(named.name.clone());
                                found.push(schedule(Rule { on, ..base.clone() }, end));
                            }
                            _ => {}
                        }
                    }
                    for (month, day, end) in self.year_days(end) {
                        let rule = Rule {
                            by_month: Some(month),
                            by_month_day: vec![day as i32],
                            ..base.clone()
                        };
                        found.push(schedule(rule, end));
                    }
                    for (days, end) in self.month_days(end) {
                        let month_at = if self.is(end, &["in"]) { end + 1 } else { end };
                        if let (Some(month), true) = (self.parser.month(month_at), days.weekdays())
                        {
                            let rule = Rule { by_month: Some(month), ..days.apply(base.clone()) };
                            found.push(schedule(rule, month_at + 1));
                        }
                    }
                }
            }
            // "every 3 weeks after it's done".
            if let Some(after) = self.after_done(end) {
                found.push(Body { at: after, kind: RepeatKind::After, rule: base, fill: None });
            }
        }

        for (days, end) in self.month_days(lead) {
            // "the 15th", "last fri", "the first weekday of every month".
            for end in self.of_month(end) {
                found.push(schedule(days.apply(Rule::every(Frequency::Monthly)), end));
            }
            // "the last friday in october every year".
            let month_at = if self.is(end, &["in"]) { end + 1 } else { end };
            if let (Some(month), true) = (self.parser.month(month_at), days.weekdays()) {
                for end in self.every_year(month_at + 1, said) {
                    let rule = Rule {
                        by_month: Some(month),
                        ..days.apply(Rule::every(Frequency::Yearly))
                    };
                    found.push(schedule(rule, end));
                }
            }
        }

        // "every 9 dec", "mum's birthday every year", "every christmas".
        for (month, day, end) in self.year_days(lead) {
            for end in self.every_year(end, said) {
                let rule = Rule {
                    by_month: Some(month),
                    by_month_day: vec![day as i32],
                    ..Rule::every(Frequency::Yearly)
                };
                found.push(schedule(rule, end));
            }
        }

        // "every easter", "mother's day every year", "each half term": a
        // day in the calendar that falls on a different day each year.
        // "every other easter" too.
        if let Some(yearly) = every(Frequency::Yearly, counted.unwrap_or(1)) {
            for (index, end) in self.parser.holidays(days_at) {
                let Some(named) = self.context.calendar.named.get(index) else { continue };
                if named.fixed().is_some() {
                    continue;
                }
                let ends = if counted.is_some() { vec![end] } else { self.every_year(end, said) };
                for end in ends {
                    let rule = Rule { on: Some(named.name.clone()), ..yearly.clone() };
                    found.push(schedule(rule, end));
                }
            }
        }

        // "the weekend before each mother's day", "the friday before every
        // christmas", "2 days before the last working day of every month":
        // each day it comes round, moved.
        if at == 0 {
            let most = self.parser.tokens.len().saturating_sub(1).min(5);
            // An offset ends with which way it goes.
            for split in (2..=most).filter(|split| self.is(split - 1, &["before", "after"])) {
                let said = &self.parser.tokens[..split];
                let Some(offset) = crate::offset::shift_of(said, false, &self.parser) else {
                    continue;
                };
                if offset.is_time() || matches!(offset.shift, Shift::By { count: 0, .. }) {
                    continue;
                }
                for body in self.bodies(split) {
                    if body.kind == RepeatKind::Schedule && body.rule.offset.is_none() {
                        found.push(Body {
                            rule: Rule { offset: Some(offset), ..body.rule },
                            ..body
                        });
                    }
                }
            }
        }

        // Every third week on Tuesday, after every month's third Tuesday.
        found.extend(weeks_apart);

        // "3 weeks after done", "a month after it's done".
        if !said && !alternate {
            for (n, unit, end) in self.every(at) {
                if let (Some(after), Some(rule)) = (self.after_done(end), every(unit, n)) {
                    found.push(Body { at: after, kind: RepeatKind::After, rule, fill: None });
                }
            }
        }
        found
    }

    /// "after", "after done", "after it's done", "after i do it", "after
    /// completion": where it ends, or `None` when it doesn't say.
    fn after_done(&self, at: usize) -> Option<usize> {
        if !self.is(at, &["after"]) {
            return None;
        }
        let mut end = at + 1;
        if self.is(end, &["it"]) {
            end += 1;
        } else if self.is(end, &["i"]) && self.is(end + 1, &["do", "did", "finish"]) {
            end += if self.is(end + 2, &["it"]) { 3 } else { 2 };
        }
        if self.is(end, &["is", "was"]) {
            end += 1;
        }
        if self
            .is(end, &["done", "completed", "completion", "finished", "ticked", "checked", "last"])
        {
            end += 1;
        }
        Some(end)
    }

    /// The days the words from a place might mean, each with where it ends.
    fn days(&self, at: usize) -> Vec<(Day, usize)> {
        self.parser
            .day(at, 0)
            .into_iter()
            .filter_map(|(expr, end)| Some((self.context.day(&expr)?.day, end)))
            .collect()
    }

    /// When it starts and stops, after the rest, in any order: "from next
    /// monday", "until christmas", "for 6 weeks", "10 times".
    fn tails(&self, at: usize, seen: Tail, found: &mut Vec<Tail>) {
        found.push(seen);
        if at >= self.parser.tokens.len() {
            return;
        }
        if seen.starts.is_none() && self.is(at, &["from", "starting", "beginning", "start"]) {
            let from = if self.is(at + 1, &["from"]) { at + 2 } else { at + 1 };
            for (day, end) in self.days(from) {
                self.tails(end, Tail { starts: Some(day), at: end, ..seen }, found);
            }
        }
        if !seen.stops() {
            if self.is(at, &["until", "till", "til", "to", "through", "ending", "ends"]) {
                for (day, end) in self.days(at + 1) {
                    self.tails(end, Tail { until: Some(day), at: end, ..seen }, found);
                }
            }
            if let (true, Some(n)) = (self.is(at, &["for"]), self.parser.number(at + 1)) {
                for (unit, times, end) in self.parser.unit(at + 2) {
                    if let (Some(_), Some(n)) = (frequency(unit), n.checked_mul(times)) {
                        self.tails(end, Tail { lasts: Some((n, unit)), at: end, ..seen }, found);
                    }
                }
            }
            // "10 times", which is read as "10 x".
            let count = match self.parser.number(at) {
                Some(n) if n > 0 && self.is(at + 1, &["x"]) => Some((n, at + 2)),
                _ if self.is(at, &["once"]) => Some((1, at + 1)),
                _ if self.is(at, &["twice"]) => Some((2, at + 1)),
                _ => None,
            };
            if let Some((count, end)) = count {
                self.tails(end, Tail { count: Some(count), at: end, ..seen }, found);
            }
        }
        if self.is(at, &["forever", "indefinitely"]) {
            self.tails(at + 1, Tail { at: at + 1, ..seen }, found);
        }
        if self.is(at, &["for"]) && self.is(at + 1, &["ever"]) {
            self.tails(at + 2, Tail { at: at + 2, ..seen }, found);
        }
    }

    /// Every way the words say something repeats. `from` is the day it's
    /// on now, which "every week" and "monthly" take their days from.
    pub fn repeats(&self, from: Day) -> Vec<Repeat> {
        let today = self.context.today;
        let calendar = self.context.calendar;
        let mut found: Vec<Repeat> = Vec::new();
        let mut add = |repeat: Repeat| {
            if !found.contains(&repeat) {
                found.push(repeat);
            }
        };
        for body in self.bodies(0) {
            let mut tails = Vec::new();
            self.tails(body.at, Tail { at: body.at, ..Tail::default() }, &mut tails);
            for tail in tails.into_iter().filter(|tail| tail.at == self.parser.tokens.len()) {
                if body.kind == RepeatKind::After {
                    if !tail.stops() {
                        let starts = tail.starts.unwrap_or(today);
                        add(Repeat::after_done(body.rule.clone(), starts));
                    }
                    continue;
                }
                let base = tail.starts.unwrap_or(from);
                let mut rule = body.rule.clone();
                match body.fill {
                    Some(Fill::Weekday) => rule.by_day = vec![ByDay::every(base.weekday())],
                    Some(Fill::MonthDay) => rule.by_month_day = vec![base.day() as i32],
                    Some(Fill::YearDay) => {
                        rule.by_month = Some(base.month());
                        rule.by_month_day = vec![base.day() as i32];
                    }
                    None => {}
                }
                if let Some(count) = tail.count {
                    let Ok(count) = u32::try_from(count) else { continue };
                    rule.count = Some(count);
                }
                let Some(starts) = rule.first_day(base, calendar) else { continue };
                rule.until =
                    tail.until.or(tail.lasts.and_then(|lasts| end(starts, lasts, calendar)));
                if tail.lasts.is_some() && rule.until.is_none() {
                    continue;
                }
                if rule.until.is_some_and(|until| until < starts) {
                    continue;
                }
                add(Repeat::schedule(rule.clone(), starts));
                // Every other week could be this week's or next week's.
                if rule.frequency == Frequency::Weekly
                    && rule.interval == 2
                    && tail.starts.is_none()
                {
                    let later = starts
                        .week_start(Weekday::Monday)
                        .and_then(|monday| monday.add_days(7))
                        .and_then(|next| rule.first_day(next, calendar))
                        .filter(|later| rule.until.is_none_or(|until| *later <= until));
                    if let Some(starts) = later {
                        add(Repeat::schedule(rule, starts));
                    }
                }
            }
        }
        found
    }
}

/// The last day of something that goes on for so long from a day.
fn end(starts: Day, (n, unit): (i64, Unit), calendar: &Working) -> Option<Day> {
    crate::days::moved(starts, n, unit, calendar)?.add_days(-1)
}

/// The words a unit is said with, as `Parser::unit` reads them, but for
/// working days and weekdays, which aren't repeated by.
const UNIT_WORDS: [&str; 19] = [
    "day",
    "days",
    "d",
    "week",
    "weeks",
    "w",
    "wk",
    "wks",
    "fortnight",
    "fortnights",
    "month",
    "months",
    "mo",
    "mos",
    "year",
    "years",
    "y",
    "yr",
    "yrs",
];

/// Whether a word says a unit to repeat by, or starts one when it's the
/// last word, still being typed. Not one that could as well be a day of
/// the week, like "mo", which "every 3rd mo" might go on to be "monday".
fn unit_word(word: &str, last: bool) -> bool {
    let weekday = weekday_alias(word).is_some()
        || WEEKDAY_WORDS.iter().any(|(name, _)| word.len() >= 2 && name.starts_with(word));
    let typing = last && word.len() >= 2;
    !weekday
        && (UNIT_WORDS.contains(&word)
            || (typing && UNIT_WORDS.iter().any(|unit| unit.starts_with(word))))
}

/// What's typed, as the grammar reads it: without "repeat" or "repeats" at
/// its start, with "10 times" as "10 x", since "times" is one of the words
/// that can always be left out, and with an ordinal right before a unit
/// after "every" as the number it counts: "every fourth day" is "every four
/// day", every four days. Only right before it, since "the" and "of" are
/// left out too, and "every fifth of the month" is a day of each month.
pub(crate) fn said(text: &str) -> String {
    let lower = text.trim_start().to_lowercase();
    let rest = ["repeats", "repeat"]
        .iter()
        .find_map(|word| {
            lower.strip_prefix(word).filter(|rest| !rest.starts_with(char::is_alphanumeric))
        })
        .unwrap_or(&lower);
    // Each word, with what comes after it before the next.
    let mut words: Vec<(String, String)> = vec![(String::new(), String::new())];
    for c in rest.chars() {
        let (word, between) = words.last_mut().expect("there's always one");
        if c.is_alphanumeric() || c == '_' {
            if !between.is_empty() {
                words.push((c.to_string(), String::new()));
            } else {
                word.push(c);
            }
        } else {
            between.push(c);
        }
    }
    let plain = |at: usize| words.get(at).map(|(word, _)| word.as_str());
    let spaced = |at: usize| words.get(at).is_some_and(|(_, between)| between.trim().is_empty());
    let mut out = String::with_capacity(rest.len());
    for (at, (word, between)) in words.iter().enumerate() {
        let counted = || {
            let every = |at: usize| matches!(plain(at), Some("every" | "each"));
            // "every twenty-fourth day", where "twenty" comes between.
            let tens = |at: usize| {
                plain(at)
                    .and_then(crate::words::number_word)
                    .is_some_and(|n| n >= 20 && n % 10 == 0)
            };
            let compound = at > 1 && tens(at - 1) && every(at - 2);
            let after_every = at > 0 && (every(at - 1) || compound);
            let last = at + 2 == words.len();
            let unit = plain(at + 1).is_some_and(|next| unit_word(next, last))
                && spaced(at)
                && plain(at + 2) != Some("of");
            if !after_every || !unit {
                return None;
            }
            // Not "every first day", which is no more than every day.
            crate::words::cardinal(word).filter(|n| compound || (n != "one" && n != "1"))
        };
        match word.as_str() {
            "time" | "times" => out.push('x'),
            _ => match counted() {
                Some(n) => out.push_str(&n),
                None => out.push_str(word),
            },
        }
        out.push_str(between);
    }
    out
}

/// What a picker offers before anything's typed, from the day something
/// is on: every day, weekday, week, month and year as of that day, and a
/// week after it's done.
pub(crate) fn suggestions(from: Day, today: Day) -> Vec<Repeat> {
    let weekday = from.weekday();
    let date = from.day() as i32;
    // Which of its weekday in the month it is: the last, when it's in the
    // last week, or else the 1st to 4th.
    let nth = if from.day() + 7 > from.month_end().day() { -1 } else { (date + 6) / 7 };
    let of = |frequency| Rule::every(frequency);
    let schedules = [
        of(Frequency::Daily),
        Rule {
            by_day: WORKING.iter().copied().map(ByDay::every).collect(),
            ..of(Frequency::Weekly)
        },
        Rule { by_day: vec![ByDay::every(weekday)], ..of(Frequency::Weekly) },
        Rule { by_month_day: vec![date], ..of(Frequency::Monthly) },
        Rule { by_day: vec![ByDay { weekday, nth }], ..of(Frequency::Monthly) },
        Rule { by_month: Some(from.month()), by_month_day: vec![date], ..of(Frequency::Yearly) },
    ];
    schedules
        .into_iter()
        .map(|rule| Repeat::schedule(rule, from))
        .chain([Repeat::after_done(of(Frequency::Weekly), today)])
        .collect()
}
