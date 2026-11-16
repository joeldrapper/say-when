//! Reads how something repeats, written out in plain English:
//!
//! - every so often: "daily", "every day", "every 3 days", "weekly",
//!   "fortnightly", "every other week", "every 3 months", "yearly"
//! - on some weekdays: "every tue", "tuesdays", "mon wed and fri", "every
//!   other thursday", "weekdays", "weekends", "working days"
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

use crate::day::{Day, Weekday};
use crate::days::{Context, DayExpr, Parser, Unit};
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
                rule.skips_bank_holidays = *working;
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
        if self.is(at, &["working", "work", "business"]) && self.is(at + 1, &["day", "days"]) {
            add(&WORKING, true, at + 2);
        }
        if self.is(at, &["workday", "workdays"]) {
            add(&WORKING, true, at + 1);
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
        const SAID: [(&[&str], i64, Frequency); 6] = [
            (&["daily"], 1, Frequency::Daily),
            (&["weekly"], 1, Frequency::Weekly),
            (&["fortnightly"], 2, Frequency::Weekly),
            (&["monthly"], 1, Frequency::Monthly),
            (&["quarterly"], 3, Frequency::Monthly),
            (&["yearly", "annually"], 1, Frequency::Yearly),
        ];
        SAID.iter()
            .filter(|(words, ..)| self.is(at, words))
            .map(|(_, n, unit)| (*n, *unit, at + 1))
            .collect()
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
                    crate::named::NAMED_DAYS[index].fixed()
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
        let said = self.is(at, &["every", "each", "on"]);
        let lead = if said { at + 1 } else { at };
        let schedule =
            |rule: Rule, at: usize| Body { at, kind: RepeatKind::Schedule, rule, fill: None };

        // "every tue", "every other thursday", "tuesdays", "mon wed fri",
        // "weekdays".
        let other = self.is(lead, &["other", "alternate"])
            || matches!(self.parser.tokens.get(lead), Some(Token::Ordinal(2)));
        let counted = if other { Some(2) } else { self.parser.number(lead) }.filter(|_| said);
        let days_at = if counted.is_some() { lead + 1 } else { lead };
        if let Some(weekly) = every(Frequency::Weekly, counted.unwrap_or(1)) {
            for days in self.weekday_list(days_at) {
                let rule = Rule {
                    by_day: days.days.iter().copied().map(ByDay::every).collect(),
                    skips_bank_holidays: days.working,
                    ..weekly.clone()
                };
                found.push(schedule(rule, days.at));
            }
        }

        // "every 3 weeks", "every other month", "daily", each optionally
        // on some days: "every 2 weeks on tue", "monthly on the 15th".
        let often = if said { self.every(lead) } else { Vec::new() };
        for (n, unit, end) in often.into_iter().chain(self.adverb(at)) {
            let Some(base) = every(unit, n) else { continue };
            let fill = match unit {
                Frequency::Daily => None,
                Frequency::Weekly => Some(Fill::Weekday),
                Frequency::Monthly => Some(Fill::MonthDay),
                Frequency::Yearly => Some(Fill::YearDay),
            };
            found.push(Body { at: end, kind: RepeatKind::Schedule, rule: base.clone(), fill });
            // "on" was left out with the other words that always can be.
            match unit {
                Frequency::Daily => {}
                Frequency::Weekly => {
                    for days in self.weekday_list(end) {
                        let rule = Rule {
                            by_day: days.days.iter().copied().map(ByDay::every).collect(),
                            skips_bank_holidays: days.working,
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

        // "3 weeks after done", "a month after it's done".
        if !said {
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
                        add(Repeat { kind: RepeatKind::After, rule: body.rule.clone(), starts });
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
                rule.until = tail.until.or(tail.lasts.and_then(|lasts| end(starts, lasts)));
                if tail.lasts.is_some() && rule.until.is_none() {
                    continue;
                }
                if rule.until.is_some_and(|until| until < starts) {
                    continue;
                }
                add(Repeat { kind: RepeatKind::Schedule, rule: rule.clone(), starts });
                // Every other week could be this week's or next week's.
                if rule.frequency == Frequency::Weekly
                    && rule.interval == 2
                    && tail.starts.is_none()
                {
                    let later = starts
                        .week_start()
                        .and_then(|monday| monday.add_days(7))
                        .and_then(|next| rule.first_day(next, calendar))
                        .filter(|later| rule.until.is_none_or(|until| *later <= until));
                    if let Some(starts) = later {
                        add(Repeat { kind: RepeatKind::Schedule, rule, starts });
                    }
                }
            }
        }
        found
    }
}

/// The last day of something that goes on for so long from a day.
fn end(starts: Day, (n, unit): (i64, Unit)) -> Option<Day> {
    crate::days::moved(starts, n, unit, &crate::calendar::Working::weekdays())?.add_days(-1)
}

/// What's typed, as the grammar reads it: without "repeat" or "repeats" at
/// its start, and with "10 times" as "10 x", since "times" is one of the
/// words that can always be left out.
pub(crate) fn said(text: &str) -> String {
    let lower = text.trim_start().to_lowercase();
    let rest = ["repeats", "repeat"]
        .iter()
        .find_map(|word| {
            lower.strip_prefix(word).filter(|rest| !rest.starts_with(char::is_alphanumeric))
        })
        .unwrap_or(&lower);
    let mut out = String::with_capacity(rest.len());
    let mut word = String::new();
    for c in rest.chars().chain([' ']) {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        out.push_str(if word == "time" || word == "times" { "x" } else { &word });
        word.clear();
        out.push(c);
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
        .map(|rule| Repeat { kind: RepeatKind::Schedule, rule, starts: from })
        .chain([Repeat { kind: RepeatKind::After, rule: of(Frequency::Weekly), starts: today }])
        .collect()
}
