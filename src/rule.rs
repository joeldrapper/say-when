//! How something repeats, and the days it comes round on.
//!
//! A repeat is on a schedule, like every Tuesday or the last Friday of the
//! month, or comes round so long after it was last done, like a week after
//! the plants were watered. Either way its rule is an iCalendar RRULE (RFC
//! 5545, 3.3.10), like `FREQ=WEEKLY;INTERVAL=2;BYDAY=TU`, so calendar apps
//! can read it. Only the shapes people make are read: weekdays every week,
//! days or weekdays of the month, and a month of the year.
//!
//! One part isn't iCalendar's: `X-SKIP-DAYS-OFF=TRUE`, for which the days
//! off in the reader's calendar don't count, so "the last working day of
//! the month" is the last weekday that isn't one. Calendar apps ignore
//! it, as RFC 5545 says to, and show the last weekday. It's also read as
//! `X-BANK-HOLIDAYS=SKIP`, the name it had first.
//!
//! And two for what iCalendar can't say: `X-ON=Mother’s Day`, for a yearly
//! rule on a day in the reader's calendar that falls on a different day
//! each year, and `X-OFFSET=-WEEKEND`, for each day it comes round to be
//! moved by an [`Offset`], like the weekend before each Mother's Day.
//! Calendar apps that don't know them show the rule without them.
//!
//! Things repeat at most once a day, so each time one comes round is known
//! by its day.

use crate::calendar::Working;
use crate::day::{Day, Weekday};
use crate::days::ordinal_name;
use crate::locale::{Locale, month_name};
use crate::named::Called;
use crate::offset::{Offset, Shift};
use core::fmt;

/// The days a repeat comes round on, in order, from
/// [`crate::Reader::repeat_days`].
#[derive(Clone, Debug)]
pub struct RepeatDays<'a> {
    rule: &'a Rule,
    start: Day,
    /// The day its periods are counted from: the start, or for one an
    /// offset moves, the day that moves to it.
    anchor: Day,
    from: Day,
    to: Day,
    calendar: &'a Working,
    skipped: &'a [Day],
    named: Option<&'a Called>,
    /// The next period to look in, and the last there's any need to.
    period: i64,
    last: i64,
    found: std::vec::IntoIter<Day>,
    counted: u32,
    done: bool,
    /// The last day it came round, as an offset can move two to one.
    came: Option<Day>,
}

impl RepeatDays<'_> {
    /// None at all.
    pub(crate) fn none<'a>(rule: &'a Rule, calendar: &'a Working) -> RepeatDays<'a> {
        RepeatDays { done: true, ..rule.each_day(Day::MIN, Day::MIN, Day::MIN, calendar, &[]) }
    }
}

impl Iterator for RepeatDays<'_> {
    type Item = Day;

    fn next(&mut self) -> Option<Day> {
        let rule = self.rule;
        loop {
            let Some(day) = self.found.next() else {
                if self.done || self.period > self.last {
                    return None;
                }
                let found = rule.period_days(self.anchor, self.period, self.calendar, self.named);
                let Some(found) = found else {
                    self.done = true;
                    return None;
                };
                self.found = found.into_iter();
                self.period += 1;
                continue;
            };
            if day < self.start || self.came.is_some_and(|came| day <= came) {
                continue;
            }
            if day > self.to || rule.until.is_some_and(|until| day > until) {
                self.done = true;
                self.found = Vec::new().into_iter();
                return None;
            }
            // Those skipped count, as for iCalendar's EXDATE.
            self.counted += 1;
            self.came = Some(day);
            if rule.count.is_some_and(|count| self.counted >= count) {
                self.done = true;
                self.found = Vec::new().into_iter();
            }
            if day >= self.from && !self.skipped.contains(&day) {
                return Some(day);
            }
            if self.done {
                return None;
            }
        }
    }
}

/// How often a rule comes round.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Frequency {
    fn code(self) -> &'static str {
        match self {
            Frequency::Daily => "DAILY",
            Frequency::Weekly => "WEEKLY",
            Frequency::Monthly => "MONTHLY",
            Frequency::Yearly => "YEARLY",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Frequency::Daily => "day",
            Frequency::Weekly => "week",
            Frequency::Monthly => "month",
            Frequency::Yearly => "year",
        }
    }
}

pub(crate) const CODES: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

/// A weekday, with which of them in the month it is for monthly and yearly
/// rules: 2 for the second, -1 for the last, or 0 for every one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ByDay {
    /// The day of the week.
    pub weekday: Weekday,
    /// Which of them in the month, or 0 for every one.
    pub nth: i32,
}

impl ByDay {
    /// Every one of a day of the week.
    pub fn every(weekday: Weekday) -> ByDay {
        ByDay { weekday, nth: 0 }
    }
}

/// A rule for when something repeats. Make one with [`Rule::every`] or
/// [`Rule::parse`], and set what else it says.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub struct Rule {
    /// How often.
    pub frequency: Frequency,
    /// Every how many: 2 for every other. 0 is taken as 1, as an RRULE
    /// with no INTERVAL is.
    pub interval: u32,
    /// How many times it comes round, if it stops after so many.
    pub count: Option<u32>,
    /// The last day it can come round on.
    pub until: Option<Day>,
    /// The days of the week it's on.
    pub by_day: Vec<ByDay>,
    /// The days of the month it's on, from the end when negative.
    pub by_month_day: Vec<i32>,
    /// The month of the year it's in, from 1 for January.
    pub by_month: Option<u32>,
    /// Which of the days the rest pick out in a month it's on, like -1 for
    /// the last weekday.
    pub by_set_pos: Option<i32>,
    /// Whether the days off in the reader's calendar don't count, as for
    /// "the last working day of the month".
    pub skips_days_off: bool,
    /// The named day in the reader's calendar a yearly rule is on, by any
    /// of its names, like "Mother’s Day": on whichever day it falls each
    /// year, and not in a year the calendar doesn't have it.
    pub on: Option<String>,
    /// Where each day it comes round is moved to, like the weekend before
    /// each Mother's Day. A day moved before the first it's on doesn't
    /// count, and two moved to one are one. A length of time within a day
    /// moves a day nowhere, so a rule isn't read with one.
    pub offset: Option<Offset>,
}

impl Rule {
    /// Every how many, never 0.
    fn interval_or_one(&self) -> u32 {
        self.interval.max(1)
    }

    /// A rule that comes round every one of a unit, with nothing else said.
    pub fn every(frequency: Frequency) -> Rule {
        Rule {
            frequency,
            interval: 1,
            count: None,
            until: None,
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_month: None,
            by_set_pos: None,
            skips_days_off: false,
            on: None,
            offset: None,
        }
    }

    /// A rule written as an RRULE, or `None` for one this can't work out
    /// the days of. An `UNTIL` at an instant ends on the day that is in
    /// UTC.
    pub fn parse(text: &str) -> Option<Rule> {
        let text = text.trim();
        let prefixed = text.get(..6).is_some_and(|start| start.eq_ignore_ascii_case("RRULE:"));
        let text = if prefixed { &text[6..] } else { text };
        let mut rule = Rule::every(Frequency::Daily);
        let mut frequency = None;
        for part in text.split(';').filter(|part| !part.is_empty()) {
            let (name, said) = part.split_once('=')?;
            let (name, value) = (name.to_ascii_uppercase(), said.to_ascii_uppercase());
            if name.is_empty() || value.contains('=') {
                return None;
            }
            let whole = |text: &str| {
                let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
                (!digits.is_empty()
                    && digits.len() <= 9
                    && digits.bytes().all(|b| b.is_ascii_digit()))
                .then(|| text.strip_prefix('+').unwrap_or(text).parse::<i32>().ok())
                .flatten()
            };
            match name.as_str() {
                "FREQ" => {
                    frequency = Some(match value.as_str() {
                        "DAILY" => Frequency::Daily,
                        "WEEKLY" => Frequency::Weekly,
                        "MONTHLY" => Frequency::Monthly,
                        "YEARLY" => Frequency::Yearly,
                        _ => return None,
                    })
                }
                "INTERVAL" => rule.interval = whole(&value).filter(|n| *n >= 1)? as u32,
                "COUNT" => rule.count = Some(whole(&value).filter(|n| *n >= 1)? as u32),
                "UNTIL" => rule.until = Some(until(&value)?),
                // Said twice, the last counts, as for a calendar app.
                "BYDAY" => {
                    rule.by_day.clear();
                    for each in value.split(',') {
                        let code = each.len().checked_sub(2)?;
                        let (nth, code) = each.split_at_checked(code)?;
                        let weekday = Weekday::ALL[CODES.iter().position(|c| *c == code)?];
                        let nth = if nth.is_empty() { 0 } else { whole(nth)? };
                        if nth.abs() > 5 {
                            return None;
                        }
                        rule.by_day.push(ByDay { weekday, nth });
                    }
                }
                "BYMONTHDAY" => {
                    rule.by_month_day.clear();
                    for each in value.split(',') {
                        rule.by_month_day.push(whole(each).filter(|n| *n != 0 && n.abs() <= 31)?);
                    }
                }
                "BYMONTH" => {
                    rule.by_month = Some(whole(&value).filter(|n| (1..=12).contains(n))? as u32)
                }
                "BYSETPOS" => {
                    rule.by_set_pos = Some(whole(&value).filter(|n| *n != 0 && n.abs() <= 31)?)
                }
                "WKST" if value == "MO" => {}
                // The name it was first written with, still read.
                "X-BANK-HOLIDAYS" if value == "SKIP" => rule.skips_days_off = true,
                "X-SKIP-DAYS-OFF" if value == "TRUE" => rule.skips_days_off = true,
                "X-ON" if !said.is_empty() => {
                    let on = said.replace("%3B", ";").replace("%3b", ";");
                    let on = on.replace("%3D", "=").replace("%3d", "=");
                    rule.on = Some(on.replace("%25", "%"));
                }
                "X-OFFSET" => {
                    if rule.offset.is_some() {
                        return None;
                    }
                    let offset: Offset = value.parse().ok()?;
                    if offset.is_time() || offset.at.is_some() {
                        return None;
                    }
                    rule.offset = Some(offset);
                }
                _ => return None,
            }
        }
        rule.frequency = frequency?;
        rule.shaped().then_some(rule)
    }

    /// Whether it's one of the shapes people make.
    fn shaped(&self) -> bool {
        if self.on.is_some() {
            return self.frequency == Frequency::Yearly
                && self.by_day.is_empty()
                && self.by_month_day.is_empty()
                && self.by_month.is_none()
                && self.by_set_pos.is_none();
        }
        let nths = self.by_day.iter().any(|each| each.nth != 0);
        let days = !self.by_day.is_empty() || !self.by_month_day.is_empty();
        match self.frequency {
            Frequency::Daily if days || self.by_set_pos.is_some() => return false,
            Frequency::Weekly
                if nths || !self.by_month_day.is_empty() || self.by_set_pos.is_some() =>
            {
                return false;
            }
            // Days of the year are in a month, not spread over every one.
            Frequency::Yearly if self.by_month.is_none() && days => return false,
            _ => {}
        }
        if self.by_month.is_some() && self.frequency != Frequency::Yearly {
            return false;
        }
        if !self.by_month_day.is_empty() && !self.by_day.is_empty() {
            return false;
        }
        !(self.by_set_pos.is_some() && (self.by_day.is_empty() || nths))
    }

    fn counts(&self, day: Day, calendar: &Working) -> bool {
        !self.skips_days_off || calendar.is_working_day(day)
    }

    /// The days of a month a monthly or yearly rule is on, in order.
    fn in_month(&self, year: i32, month: u32, start: Day, calendar: &Working) -> Vec<Day> {
        let Some(first) = Day::new(year, month, 1) else { return Vec::new() };
        let length = first.days_until(first.month_end()) + 1;
        let days: Vec<Day> = (0..length).filter_map(|offset| first.add_days(offset)).collect();
        let nth = |list: &[Day], n: i32| -> Option<Day> {
            let index = if n > 0 {
                n as usize - 1
            } else {
                list.len().checked_sub(n.unsigned_abs() as usize)?
            };
            list.get(index).copied()
        };
        let mut found: Vec<Day> = Vec::new();
        if !self.by_day.is_empty() {
            for each in &self.by_day {
                let matching: Vec<Day> = days
                    .iter()
                    .copied()
                    .filter(|day| day.weekday() == each.weekday && self.counts(*day, calendar))
                    .collect();
                if each.nth == 0 {
                    found.extend(matching);
                } else {
                    found.extend(nth(&matching, each.nth));
                }
            }
            found.sort();
            found.dedup();
            return match self.by_set_pos {
                None => found,
                Some(position) => nth(&found, position).into_iter().collect(),
            };
        }
        let own = [start.day() as i32];
        let numbers = if self.by_month_day.is_empty() { &own[..] } else { &self.by_month_day[..] };
        found.extend(
            numbers.iter().filter_map(|n| nth(&days, *n)).filter(|day| self.counts(*day, calendar)),
        );
        found.sort();
        found.dedup();
        found
    }

    /// The days in one period of the rule, in order, counting periods from
    /// the one it starts in, or `None` past the last day there is.
    fn period_days(
        &self,
        start: Day,
        period: i64,
        calendar: &Working,
        named: Option<&Called>,
    ) -> Option<Vec<Day>> {
        let Some(offset) = self.offset else {
            return self.unmoved_days(start, period, calendar, named);
        };
        let days = self.base_days(start, period, calendar, named)?;
        let mut days: Vec<Day> =
            days.into_iter().filter_map(|day| Some(offset.from(day, None, calendar)?.0)).collect();
        days.sort();
        days.dedup();
        Some(days)
    }

    /// The days in one period an offset moves from: those it's on, or for
    /// one after a named day that lasts, its last.
    fn base_days(
        &self,
        start: Day,
        period: i64,
        calendar: &Working,
        named: Option<&Called>,
    ) -> Option<Vec<Day>> {
        let Some(offset) = self.offset else {
            return self.unmoved_days(start, period, calendar, named);
        };
        // After a named day that lasts more than one, like Ramadan, is after
        // its last.
        let after = match offset.shift {
            Shift::By { count, .. } => count > 0,
            Shift::Weekday { before, .. } | Shift::Weekend { before } => !before,
        };
        let ends = match (named, after, self.frequency) {
            (Some(named), true, Frequency::Yearly) => {
                let year = i32::try_from(
                    i64::from(start.year())
                        .checked_add(period.checked_mul(i64::from(self.interval_or_one()))?)?,
                )
                .ok()?;
                // None after the last year the calendar has it in.
                if named.last_year().is_some_and(|last| year > last) {
                    return None;
                }
                let last = named.span(year).map(|(_, last)| last);
                Some(last.filter(|day| self.counts(*day, calendar)))
            }
            _ => None,
        };
        Some(match ends {
            Some(last) => last.into_iter().collect(),
            None => self.unmoved_days(start, period, calendar, named)?,
        })
    }

    /// The day an offset moved to the one a schedule starts on, which its
    /// periods are counted from, so "the Monday after every Sunday" that
    /// starts on a Monday counts from the Sunday before, and every other
    /// one keeps to that Sunday's weeks. Looked for as far either way as
    /// the offset goes, nearest first. `None` when no day of its moves to
    /// it.
    fn anchor(
        &self,
        start: Day,
        offset: Offset,
        periods: i64,
        calendar: &Working,
        named: Option<&Called>,
    ) -> Option<Day> {
        let each = Rule { interval: 1, count: None, until: None, ..self.clone() };
        for away in 0..=periods.min(2000) {
            for period in [-away, away] {
                for day in each.base_days(start, period, calendar, named).unwrap_or_default() {
                    if offset.from(day, None, calendar).is_some_and(|(to, _)| to == start) {
                        return Some(day);
                    }
                }
            }
        }
        None
    }

    /// The days in one period before an offset moves them.
    fn unmoved_days(
        &self,
        start: Day,
        period: i64,
        calendar: &Working,
        named: Option<&Called>,
    ) -> Option<Vec<Day>> {
        let step = period.checked_mul(i64::from(self.interval_or_one()))?;
        Some(match self.frequency {
            Frequency::Daily => {
                let day = start.add_days(step)?;
                if self.counts(day, calendar) { vec![day] } else { Vec::new() }
            }
            Frequency::Weekly => {
                let monday = start.week_start(Weekday::Monday)?.add_days(step.checked_mul(7)?)?;
                let mut offsets: Vec<i32> = if self.by_day.is_empty() {
                    vec![start.weekday().from_monday()]
                } else {
                    self.by_day.iter().map(|each| each.weekday.from_monday()).collect()
                };
                offsets.sort_unstable();
                offsets.dedup();
                let mut days = Vec::new();
                for offset in offsets {
                    let day = monday.add_days(i64::from(offset))?;
                    if self.counts(day, calendar) {
                        days.push(day);
                    }
                }
                days
            }
            Frequency::Monthly => {
                let month = start.month_start().add_months(step)?;
                self.in_month(month.year(), month.month(), start, calendar)
            }
            Frequency::Yearly => {
                let year = i32::try_from(i64::from(start.year()).checked_add(step)?).ok()?;
                Day::new(year, 1, 1)?;
                match named {
                    // None after the last year the calendar has it in.
                    Some(named) if named.last_year().is_some_and(|last| year > last) => {
                        return None;
                    }
                    Some(named) => named
                        .day(year)
                        .filter(|day| self.counts(*day, calendar))
                        .into_iter()
                        .collect(),
                    None => {
                        self.in_month(year, self.by_month.unwrap_or(start.month()), start, calendar)
                    }
                }
            }
        })
    }

    /// How many whole periods there are from the one it starts in to the
    /// one a day's in.
    fn periods_to(&self, start: Day, to: Day) -> i64 {
        let periods = match self.frequency {
            Frequency::Daily => start.days_until(to),
            Frequency::Weekly => {
                match (start.week_start(Weekday::Monday), to.week_start(Weekday::Monday)) {
                    (Some(from), Some(to)) => from.days_until(to) / 7,
                    _ => 0,
                }
            }
            Frequency::Monthly => {
                i64::from(to.year() - start.year()) * 12 + i64::from(to.month())
                    - i64::from(start.month())
            }
            Frequency::Yearly => i64::from(to.year() - start.year()),
        };
        periods.div_euclid(i64::from(self.interval_or_one()))
    }

    /// The days a schedule comes round on from its first, `start`, up to
    /// and including `to`, and none before `from`, at most so many.
    pub(crate) fn days(
        &self,
        start: Day,
        from: Day,
        to: Day,
        most: usize,
        calendar: &Working,
    ) -> Vec<Day> {
        self.each_day(start, from, to, calendar, &[]).take(most).collect()
    }

    /// Each day a schedule comes round on from its first, `start`, up to
    /// and including `to`, none before `from`, and none of `skipped`.
    pub(crate) fn each_day<'a>(
        &'a self,
        start: Day,
        from: Day,
        to: Day,
        calendar: &'a Working,
        skipped: &'a [Day],
    ) -> RepeatDays<'a> {
        // With no count to keep, it starts a period before the one the
        // first day asked for is in, rather than going through every one
        // since it began. With one, every period before has to be counted.
        // An offset can move a day out of its period, into another as far
        // off as it goes.
        let back = self.offset.map_or(1, |offset| {
            let period = i64::from(self.interval_or_one()).saturating_mul(match self.frequency {
                Frequency::Daily => 1,
                Frequency::Weekly => 7,
                Frequency::Monthly => 28,
                Frequency::Yearly => 365,
            });
            (offset.most_days(calendar) + period - 1) / period + 1
        });
        // The named day it's on, if it's on one, which isn't on any day if
        // the calendar hasn't got it.
        let named = self.on.as_ref().map(|name| crate::named::find(&calendar.named, name));
        let anchor = self
            .offset
            .and_then(|offset| {
                let periods = back.saturating_mul(i64::from(self.interval_or_one()));
                self.anchor(start, offset, periods, calendar, named.flatten())
            })
            .unwrap_or(start);
        let before = if from > anchor { (self.periods_to(anchor, from) - back).max(0) } else { 0 };
        RepeatDays {
            rule: self,
            start,
            anchor,
            from,
            to,
            calendar,
            skipped,
            named: named.flatten(),
            period: if self.count.is_none() { before } else { 0 },
            last: self.periods_to(anchor, to.max(anchor)).saturating_add(back + 1),
            found: Vec::new().into_iter(),
            counted: 0,
            done: matches!(named, Some(None)),
            came: None,
        }
    }

    /// The first day it comes round on, on or after a day: the first of
    /// its days in any period, since one every other week could start this
    /// week.
    pub(crate) fn first_day(&self, from: Day, calendar: &Working) -> Option<Day> {
        let each = Rule { interval: 1, count: None, until: None, ..self.clone() };
        let to = from.add_months(12 * 30).unwrap_or(Day::MAX);
        each.days(from, from, to, 1, calendar).first().copied()
    }

    /// The rule as it is when one of its days moves to another and the
    /// rest move with it: on Tuesdays, moved from a Tuesday to a
    /// Wednesday, it's on Wednesdays, and on the 2nd Tuesday of the month,
    /// moved to the 3rd Thursday, it's on that. One on a named day, or that
    /// picks its day by where it is in the month, like the last working
    /// day, is as it was. The day it starts on is the caller's to move.
    pub fn moved(&self, from: Day, to: Day) -> Rule {
        let mut rule = self.clone();
        if self.on.is_some() || self.by_set_pos.is_some() {
            return rule;
        }
        let along = to.weekday().from_monday() - from.weekday().from_monday();
        let weekday =
            |weekday: Weekday| Weekday::ALL[(weekday.from_monday() + along).rem_euclid(7) as usize];
        match self.frequency {
            Frequency::Daily => {}
            Frequency::Weekly => {
                for each in &mut rule.by_day {
                    each.weekday = weekday(each.weekday);
                }
                rule.by_day.sort_by_key(|each| each.weekday.from_monday());
            }
            Frequency::Monthly | Frequency::Yearly => {
                if let [only] = self.by_month_day[..] {
                    let from_end = to.month_end().day() as i32 - to.day() as i32;
                    rule.by_month_day =
                        vec![if only < 0 { -1 - from_end } else { to.day() as i32 }];
                } else if !self.by_month_day.is_empty() {
                    let by = to.day() as i32 - from.day() as i32;
                    rule.by_month_day =
                        self.by_month_day
                            .iter()
                            .map(|n| {
                                if *n > 0 { (n + by).clamp(1, 31) } else { (n + by).clamp(-31, -1) }
                            })
                            .collect();
                }
                if let [only] = self.by_day[..] {
                    let from_end = to.month_end().day() - to.day();
                    let nth = match only.nth {
                        0 => 0,
                        nth if nth < 0 => -1 - (from_end / 7) as i32,
                        _ => (to.day() as i32 - 1) / 7 + 1,
                    };
                    rule.by_day = vec![ByDay { weekday: to.weekday(), nth }];
                } else {
                    for each in &mut rule.by_day {
                        each.weekday = weekday(each.weekday);
                    }
                }
                if self.frequency == Frequency::Yearly && self.by_month.is_some() {
                    rule.by_month = Some(to.month());
                }
            }
        }
        rule
    }

    /// The day something that repeats so long after it's done comes round
    /// next, done on a day.
    pub fn after_done(&self, done: Day) -> Option<Day> {
        let n = i64::from(self.interval_or_one());
        match self.frequency {
            Frequency::Daily => done.add_days(n),
            Frequency::Weekly => done.add_days(7 * n),
            Frequency::Monthly => done.add_months(n),
            Frequency::Yearly => done.add_months(12 * n),
        }
    }

    /// The days it's on within a month, as people say them: "the 15th",
    /// "the 1st and 15th", "the last day", "the 2nd Tuesday", "the last
    /// working day".
    fn day_of_month(&self, start: Day, working: &[Weekday]) -> String {
        if !self.by_day.is_empty() {
            if let Some(position) = self.by_set_pos {
                let kind = if let Some(kind) = self.kind_of(&self.by_day, working) {
                    kind.to_string()
                } else {
                    list(self.by_day.iter().map(|each| each.weekday.name().to_string()).collect())
                };
                return format!("the {} {kind}", ordinal_name(i64::from(position)));
            }
            return list(
                self.by_day
                    .iter()
                    .map(|each| match each.nth {
                        0 => format!("every {}", each.weekday.name()),
                        nth => {
                            format!("the {} {}", ordinal_name(i64::from(nth)), each.weekday.name())
                        }
                    })
                    .collect(),
            );
        }
        let own = [start.day() as i32];
        let numbers = if self.by_month_day.is_empty() { &own[..] } else { &self.by_month_day[..] };
        let said = numbers
            .iter()
            .map(|n| if *n == -1 { "last day".to_string() } else { ordinal_name(i64::from(*n)) })
            .collect();
        format!("the {}", list(said))
    }

    /// What some days of the week are as one, if they're one: working
    /// days, those of the working week when days off are skipped,
    /// weekdays, Monday to Friday, when they aren't, or the weekend.
    fn kind_of(&self, days: &[ByDay], working: &[Weekday]) -> Option<&'static str> {
        if self.skips_days_off && same_days(days, working) {
            Some("working day")
        } else if !self.skips_days_off && same_days(days, &WORKING) {
            Some("weekday")
        } else if same_days(days, &[Weekday::Saturday, Weekday::Sunday]) {
            Some("weekend")
        } else {
            None
        }
    }

    /// How a schedule repeats, as people say it, like "Every 2 weeks on
    /// Tuesday" or "Every month on the last working day", with when it
    /// ends if it does. `starts` is its first day, which says the day a
    /// rule leaves out, and the year of `today` is left off days in it.
    /// Working days are Monday to Friday: [`crate::Reader::describe_repeat`]
    /// says them as its calendar works.
    pub fn describe(&self, starts: Day, today: Option<Day>, locale: Locale) -> String {
        self.describe_in(starts, today, locale, &WORKING)
    }

    /// How a schedule repeats, with the days of the week that are worked.
    pub(crate) fn describe_in(
        &self,
        starts: Day,
        today: Option<Day>,
        locale: Locale,
        working: &[Weekday],
    ) -> String {
        let unit = self.frequency.unit();
        let every = if self.interval_or_one() == 1 {
            "Every".to_string()
        } else {
            format!("Every {}", self.interval_or_one())
        };
        let units = if self.interval_or_one() == 1 { unit.to_string() } else { format!("{unit}s") };
        let said = match self.frequency {
            Frequency::Yearly if self.on.is_some() => {
                let on = self.on.as_deref().unwrap_or_default();
                match self.offset {
                    Some(offset) => format!("{every} {units}, {} {on}", offset.describe()),
                    None => format!("{every} {units} on {on}"),
                }
            }
            Frequency::Daily if self.interval_or_one() == 1 && self.skips_days_off => {
                "Every day but bank holidays".to_string()
            }
            Frequency::Daily => format!("{every} {units}"),
            Frequency::Weekly => {
                let own = [ByDay::every(starts.weekday())];
                let weekdays = if self.by_day.is_empty() { &own[..] } else { &self.by_day[..] };
                let kind = self.kind_of(weekdays, working);
                let names =
                    list(weekdays.iter().map(|each| each.weekday.name().to_string()).collect());
                match (self.interval_or_one(), kind) {
                    (1, Some(kind)) => format!("Every {kind}"),
                    (1, None) => format!("Every {names}"),
                    (interval, Some(kind)) => format!("Every {interval} weeks, on {kind}s"),
                    (_, None) => format!("{every} weeks on {names}"),
                }
            }
            Frequency::Monthly => {
                format!("{every} {units} on {}", self.day_of_month(starts, working))
            }
            Frequency::Yearly => {
                let month = self.by_month.unwrap_or(starts.month());
                let plain = self.by_day.is_empty() && self.by_month_day.len() <= 1;
                let n = self.by_month_day.first().copied().unwrap_or(starts.day() as i32);
                if plain && n > 0 {
                    format!("{every} {units} on {}", locale.day_of_month(month, n as u32))
                } else {
                    format!(
                        "{every} {units} on {} of {}",
                        self.day_of_month(starts, working),
                        month_name(month)
                    )
                }
            }
        };
        let said = match self.offset {
            Some(offset) if self.on.is_none() => match said.split_once(" on ") {
                Some((every, on)) => format!("{every}, {} {on}", offset.describe()),
                None => format!("{said}, moved to {}", offset.describe()),
            },
            _ => said,
        };
        if let Some(until) = self.until {
            return format!("{said}, until {}", locale.long(until, today));
        }
        match self.count {
            Some(1) => format!("{said}, once"),
            Some(count) => format!("{said}, {count} times"),
            None => said,
        }
    }

    /// How something that repeats after it's done says so, like "3 weeks
    /// after it’s done".
    pub fn describe_after(&self) -> String {
        let unit = self.frequency.unit();
        match self.interval_or_one() {
            1 => format!("A {unit} after it’s done"),
            interval => format!("{interval} {unit}s after it’s done"),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Rule {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Rule {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Rule, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rule::parse(&text)
            .ok_or_else(|| serde::de::Error::custom("not a rule this can work out the days of"))
    }
}

const WORKING: [Weekday; 5] =
    [Weekday::Monday, Weekday::Tuesday, Weekday::Wednesday, Weekday::Thursday, Weekday::Friday];

fn same_days(by_day: &[ByDay], weekdays: &[Weekday]) -> bool {
    by_day.len() == weekdays.len()
        && by_day.iter().all(|each| each.nth == 0 && weekdays.contains(&each.weekday))
}

/// "a", "a and b", "a, b and c".
fn list(names: Vec<String>) -> String {
    match names.split_last() {
        None => String::new(),
        Some((only, [])) => only.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// The day an `UNTIL` says: `20271225`, or `20271225T120000Z` for the day
/// that instant is in UTC. Only real days and times.
fn until(text: &str) -> Option<Day> {
    let (date, time) = match text.split_once('T') {
        Some((date, time)) => (date, Some(time.strip_suffix('Z').unwrap_or(time))),
        None => (text, None),
    };
    let figures = |text: &str| text.bytes().all(|byte| byte.is_ascii_digit());
    if date.len() != 8 || !figures(date) {
        return None;
    }
    if let Some(time) = time {
        if time.len() != 6 || !figures(time) {
            return None;
        }
        let part = |from: usize| time[from..from + 2].parse::<u32>().ok();
        if part(0)? > 23 || part(2)? > 59 || part(4)? > 59 {
            return None;
        }
    }
    Day::new(date[..4].parse().ok()?, date[4..6].parse().ok()?, date[6..].parse().ok()?)
}

/// Written as an RRULE, in the order calendar apps write them.
impl std::str::FromStr for Rule {
    type Err = crate::day::ParseError;

    /// A rule written as an RRULE, as [`Rule::parse`] reads it.
    fn from_str(text: &str) -> Result<Rule, Self::Err> {
        Rule::parse(text).ok_or(crate::day::ParseError("not a rule this can work out the days of"))
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FREQ={}", self.frequency.code())?;
        if self.interval_or_one() != 1 {
            write!(f, ";INTERVAL={}", self.interval_or_one())?;
        }
        if let Some(month) = self.by_month {
            write!(f, ";BYMONTH={month}")?;
        }
        let joined = |items: Vec<String>| items.join(",");
        if !self.by_month_day.is_empty() {
            write!(
                f,
                ";BYMONTHDAY={}",
                joined(self.by_month_day.iter().map(i32::to_string).collect())
            )?;
        }
        if !self.by_day.is_empty() {
            let days = self.by_day.iter().map(|each| {
                let code = CODES[each.weekday as usize];
                if each.nth == 0 { code.to_string() } else { format!("{}{code}", each.nth) }
            });
            write!(f, ";BYDAY={}", joined(days.collect()))?;
        }
        if let Some(position) = self.by_set_pos {
            write!(f, ";BYSETPOS={position}")?;
        }
        if let Some(until) = self.until {
            let (year, month, day) = until.ymd();
            write!(f, ";UNTIL={year:04}{month:02}{day:02}")?;
        }
        if let Some(count) = self.count {
            write!(f, ";COUNT={count}")?;
        }
        if self.skips_days_off {
            write!(f, ";X-SKIP-DAYS-OFF=TRUE")?;
        }
        if let Some(on) = &self.on {
            // What would end the part, or its name, is written as a URL
            // has it.
            let on = on.replace('%', "%25").replace(';', "%3B").replace('=', "%3D");
            write!(f, ";X-ON={on}")?;
        }
        if let Some(offset) = self.offset {
            write!(f, ";X-OFFSET={offset}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Day {
        Day::parse(text).unwrap()
    }

    fn days(rule: &str, start: &str, most: usize) -> Vec<String> {
        let rule = Rule::parse(rule).unwrap();
        let start = day(start);
        rule.days(start, start, Day::MAX, most, &Working::weekdays())
            .iter()
            .map(Day::to_string)
            .collect()
    }

    #[test]
    fn reads_and_writes_rules_the_same() {
        for rule in [
            "FREQ=DAILY",
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH",
            "FREQ=MONTHLY;BYMONTHDAY=1,15",
            "FREQ=MONTHLY;BYDAY=-1FR",
            "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;X-SKIP-DAYS-OFF=TRUE",
            "FREQ=YEARLY;BYMONTH=12;BYMONTHDAY=9;UNTIL=20301209",
            "FREQ=WEEKLY;BYDAY=WE;COUNT=10",
        ] {
            assert_eq!(Rule::parse(rule).unwrap().to_string(), rule);
        }
        assert_eq!(
            Rule::parse("rrule:freq=daily;interval=3").unwrap().to_string(),
            "FREQ=DAILY;INTERVAL=3"
        );
        // As it was first written.
        assert_eq!(
            Rule::parse("FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;X-BANK-HOLIDAYS=SKIP")
                .unwrap()
                .to_string(),
            "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;X-SKIP-DAYS-OFF=TRUE"
        );
    }

    #[test]
    fn refuses_rules_it_cant_work_out() {
        for rule in [
            "",
            "FREQ=HOURLY",
            "FREQ=DAILY;BYDAY=MO",
            "FREQ=WEEKLY;BYDAY=2MO",
            "FREQ=MONTHLY;BYMONTH=3",
            "FREQ=YEARLY;BYMONTHDAY=3",
            "FREQ=MONTHLY;BYDAY=MO;BYMONTHDAY=3",
            "FREQ=DAILY;INTERVAL=0",
            "FREQ=DAILY;UNTIL=20270230",
            "FREQ=DAILY;WKST=SU",
            "FREQ=DAILY;BYHOUR=3",
            "FREQ=MONTHLY;BYDAY=6MO",
            "FREQ=DAILY=WEEKLY",
            "FREQ=DAILY;COUNT=3=4",
        ] {
            assert_eq!(Rule::parse(rule), None, "{rule}");
        }
    }

    #[test]
    fn counts_every_day_before_one_far_on() {
        let rule = Rule::parse("FREQ=DAILY;COUNT=50000").unwrap();
        let (start, from) = (day("2000-01-01"), day("2060-01-01"));
        let found = rule.days(start, from, Day::MAX, 2, &Working::weekdays());
        assert_eq!(found, [day("2060-01-01"), day("2060-01-02")]);
        // Its 50,000th day is 22 November 2136, and there's none after.
        let last = day("2136-11-22");
        assert_eq!(rule.days(start, last, Day::MAX, 2, &Working::weekdays()), [last]);
    }

    #[test]
    fn works_out_the_days_a_rule_is_on() {
        assert_eq!(
            days("FREQ=DAILY;INTERVAL=3", "2026-10-28", 3),
            ["2026-10-28", "2026-10-31", "2026-11-03"]
        );
        assert_eq!(
            days("FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH", "2026-10-27", 4),
            ["2026-10-27", "2026-10-29", "2026-11-10", "2026-11-12"]
        );
        assert_eq!(
            days("FREQ=MONTHLY;BYMONTHDAY=31", "2026-10-31", 3),
            ["2026-10-31", "2026-12-31", "2027-01-31"]
        );
        assert_eq!(
            days("FREQ=MONTHLY;BYDAY=-1FR", "2026-10-30", 3),
            ["2026-10-30", "2026-11-27", "2026-12-25"]
        );
        assert_eq!(
            days(
                "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;X-SKIP-DAYS-OFF=TRUE",
                "2026-12-01",
                2
            ),
            ["2026-12-31", "2027-01-29"]
        );
        assert_eq!(
            days("FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29", "2026-10-28", 2),
            ["2028-02-29", "2032-02-29"]
        );
        assert_eq!(
            days("FREQ=WEEKLY;BYDAY=WE;COUNT=2", "2026-10-28", 9),
            ["2026-10-28", "2026-11-04"]
        );
        assert_eq!(
            days("FREQ=DAILY;UNTIL=20261029", "2026-10-28", 9),
            ["2026-10-28", "2026-10-29"]
        );
    }

    #[test]
    fn starts_near_the_days_asked_for_however_long_it_has_run() {
        let rule = Rule::parse("FREQ=DAILY").unwrap();
        let found = rule.days(
            day("1990-01-01"),
            day("2026-10-28"),
            day("2026-10-30"),
            99,
            &Working::weekdays(),
        );
        assert_eq!(found.len(), 3);
        // And stops at the last day there is, rather than running on.
        let yearly = Rule::parse("FREQ=YEARLY;INTERVAL=3000").unwrap();
        assert_eq!(
            yearly
                .days(day("2026-10-28"), day("2026-10-28"), Day::MAX, 99, &Working::weekdays())
                .len(),
            3
        );
    }

    #[test]
    fn writes_a_named_day_any_name_reads_back() {
        let rule = Rule { on: Some("50%; a=b".to_string()), ..Rule::every(Frequency::Yearly) };
        let text = rule.to_string();
        assert_eq!(text, "FREQ=YEARLY;X-ON=50%25%3B a%3Db");
        assert_eq!(Rule::parse(&text), Some(rule));
        assert_eq!(Rule::parse("FREQ=YEARLY;X-OFFSET=-P1D;X-OFFSET=-P2D"), None);
    }

    #[test]
    fn moves_far_and_never_twice_to_a_day() {
        let calendar = Working::weekdays();
        let each = |rule: &str, start: &str, from: &str, to: &str| -> Vec<String> {
            Rule::parse(rule)
                .unwrap()
                .days(day(start), day(from), day(to), 99, &calendar)
                .iter()
                .map(Day::to_string)
                .collect()
        };
        // A year back from a Tuesday each week.
        assert_eq!(
            each("FREQ=WEEKLY;BYDAY=TU;X-OFFSET=-P1Y", "2026-01-06", "2026-11-01", "2026-11-14"),
            ["2026-11-02", "2026-11-09"]
        );
        assert_eq!(
            each(
                "FREQ=MONTHLY;BYMONTHDAY=15;X-OFFSET=-P6M",
                "2026-01-15",
                "2026-11-01",
                "2026-12-30"
            )
            .len(),
            2
        );
        // The weekend after each day: once each weekend.
        assert_eq!(
            each("FREQ=DAILY;X-OFFSET=+WEEKEND", "2026-11-02", "2026-11-01", "2026-11-22"),
            ["2026-11-07", "2026-11-14", "2026-11-21"]
        );
    }

    #[test]
    fn moves_with_one_of_its_days() {
        let moved = |rule: &str, from: &str, to: &str| {
            Rule::parse(rule).unwrap().moved(day(from), day(to)).to_string()
        };
        // From a Tuesday to a Wednesday.
        assert_eq!(
            moved("FREQ=WEEKLY;BYDAY=TU", "2026-11-17", "2026-11-18"),
            "FREQ=WEEKLY;BYDAY=WE"
        );
        assert_eq!(
            moved("FREQ=WEEKLY;BYDAY=MO,TH", "2026-11-19", "2026-11-22"),
            "FREQ=WEEKLY;BYDAY=TH,SU"
        );
        assert_eq!(
            moved("FREQ=MONTHLY;BYMONTHDAY=15", "2026-11-15", "2026-11-16"),
            "FREQ=MONTHLY;BYMONTHDAY=16"
        );
        // The last day of the month, moved to the day before it.
        assert_eq!(
            moved("FREQ=MONTHLY;BYMONTHDAY=-1", "2026-11-30", "2026-11-29"),
            "FREQ=MONTHLY;BYMONTHDAY=-2"
        );
        // The 2nd Tuesday to the 3rd Thursday, the last Friday to the last
        // Thursday, and to the 5th Monday.
        assert_eq!(
            moved("FREQ=MONTHLY;BYDAY=2TU", "2026-11-10", "2026-11-19"),
            "FREQ=MONTHLY;BYDAY=3TH"
        );
        assert_eq!(
            moved("FREQ=MONTHLY;BYDAY=-1FR", "2026-11-27", "2026-11-26"),
            "FREQ=MONTHLY;BYDAY=-1TH"
        );
        assert_eq!(
            moved("FREQ=MONTHLY;BYDAY=4MO", "2026-11-23", "2026-11-30"),
            "FREQ=MONTHLY;BYDAY=5MO"
        );
        assert_eq!(
            moved("FREQ=YEARLY;BYMONTH=12;BYMONTHDAY=9", "2026-12-09", "2027-01-02"),
            "FREQ=YEARLY;BYMONTH=1;BYMONTHDAY=2"
        );
        // Picked by where it is in the month, or on a named day, it stays.
        let last = "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1";
        assert_eq!(moved(last, "2026-11-30", "2026-11-27"), last);
        let easter = "FREQ=YEARLY;X-ON=Easter Sunday";
        assert_eq!(moved(easter, "2027-03-28", "2027-03-29"), easter);
    }

    #[test]
    fn says_how_a_rule_repeats() {
        let said = |rule: &str, starts: &str| {
            Rule::parse(rule).unwrap().describe(day(starts), Some(day("2026-10-28")), Locale::Uk)
        };
        assert_eq!(
            said("FREQ=WEEKLY;INTERVAL=2;BYDAY=TU", "2026-11-03"),
            "Every 2 weeks on Tuesday"
        );
        assert_eq!(said("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "2026-10-28"), "Every weekday");
        assert_eq!(
            said("FREQ=MONTHLY;BYMONTHDAY=1,15", "2026-11-01"),
            "Every month on the 1st and 15th"
        );
        assert_eq!(
            said("FREQ=MONTHLY;BYDAY=-2FR", "2026-11-20"),
            "Every month on the 2nd to last Friday"
        );
        assert_eq!(
            said(
                "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;X-SKIP-DAYS-OFF=TRUE",
                "2026-10-30"
            ),
            "Every month on the last working day"
        );
        assert_eq!(
            said("FREQ=YEARLY;BYMONTH=12;BYMONTHDAY=9", "2026-12-09"),
            "Every year on 9 December"
        );
        assert_eq!(
            said("FREQ=YEARLY;BYMONTH=10;BYDAY=-1FR", "2026-10-30"),
            "Every year on the last Friday of October"
        );
        assert_eq!(
            said("FREQ=DAILY;UNTIL=20271225", "2026-10-28"),
            "Every day, until 25 December 2027"
        );
        assert_eq!(said("FREQ=DAILY;COUNT=10", "2026-10-28"), "Every day, 10 times");
        assert_eq!(
            Rule::parse("FREQ=WEEKLY;INTERVAL=3").unwrap().describe_after(),
            "3 weeks after it’s done"
        );
    }
}
