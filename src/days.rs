//! Reads a day written out in plain English, however it's built up: "the
//! day before the last working day of november in two years time", "3
//! working days before christmas", "a fortnight after next friday".
//!
//! It works in two steps. The words are parsed into a small tree, a
//! [`DayExpr`], which says what was meant without knowing what day it is:
//! every phrase is made of a few kinds of piece, each of which can go
//! inside another:
//!
//! - a day: "today", "fri", "9 dec", "christmas", "next tuesday"
//! - a span of days ([`SpanExpr`]): "next week", "november", "q1 2027"
//! - a day picked out of a span: "the last working day of november", "the
//!   end of next month", "friday next week"
//! - a day moved by so many days, weeks, months, years or working days:
//!   "2 weeks after", "the day before", "+ 10d"
//!
//! Then the tree is worked out for the day it is today, giving the day and
//! how it was read. A phrase can parse more than one way, and every way
//! that uses all its words is kept: "next friday" said on a Tuesday is the
//! Friday of next week, or the one three days off.
//!
//! The last word can be half typed, and a day that can't be written, like
//! one in the year 12000, is no reading at all.

use crate::calendar::{Said, Working};
use crate::day::{Day, Weekday};
use crate::locale::{Locale, Order, month_name};
use crate::named::Called;
use crate::options::{MONTH_WORDS, Named, WEEKDAY_WORDS, weekday_alias};
use crate::words::Token;

/// A parse: what was read, and the place of the word after it.
pub(crate) type Parsed<T> = (T, usize);

/// What a day can be moved by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Unit {
    Day,
    Week,
    Month,
    Year,
    WorkingDay,
    Weekday,
}

impl Unit {
    fn name(self) -> &'static str {
        match self {
            Unit::Day => "day",
            Unit::Week => "week",
            Unit::Month => "month",
            Unit::Year => "year",
            Unit::WorkingDay => "working day",
            Unit::Weekday => "weekday",
        }
    }
}

/// Which days in a span count, for picking one out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sort {
    Day,
    WorkingDay,
    Weekday,
    WeekendDay,
    On(Weekday),
}

impl Sort {
    fn name(self) -> &'static str {
        match self {
            Sort::Day => "day",
            Sort::WorkingDay => "working day",
            Sort::Weekday => "weekday",
            Sort::WeekendDay => "weekend day",
            Sort::On(weekday) => weekday.name(),
        }
    }

    fn counts(self, day: Day, calendar: &Working) -> bool {
        match self {
            Sort::Day => true,
            Sort::WorkingDay => calendar.is_working_day(day),
            Sort::Weekday => !day.weekday().is_weekend(),
            Sort::WeekendDay => day.weekday().is_weekend(),
            Sort::On(weekday) => day.weekday() == weekday,
        }
    }
}

/// Which year something said without one is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum YearRef {
    /// The next time it comes round.
    Coming,
    /// A year written out, like 2027.
    Exact(i32),
    /// So many years on from this one: "next year", "in two years time".
    Ahead(i64),
}

/// Which of something: this one, the next or the last, or not said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Which {
    Unsaid,
    This,
    Next,
    Last,
}

impl Which {
    fn ahead(self) -> i64 {
        match self {
            Which::Unsaid | Which::This => 0,
            Which::Next => 1,
            Which::Last => -1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Edge {
    Start,
    Middle,
    End,
}

/// A span of days, as it was said.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SpanExpr {
    /// "this week", "next month", "the year after next".
    Period { unit: Unit, ahead: i64 },
    /// "week of 9 nov", "week commencing monday": the week a day's in.
    WeekOf(Box<DayExpr>),
    /// "the weekend", "next weekend".
    Weekend(Which),
    /// "november", "november 2027", "next november".
    Month { month: u32, which: Which, year: YearRef },
    /// "q1", "q3 2027", from 0 for the first.
    Quarter { quarter: u32, year: YearRef },
    /// "2027".
    Year(i32),
    /// "the weekend before christmas", "the weekend after easter": the
    /// first that's all before a day, or all after it.
    WeekendBeside { before: bool, base: Box<DayExpr> },
    /// "ramadan", "half term 2027": one of the calendar's named days that
    /// lasts more than a day.
    Named { index: usize, year: YearRef },
}

/// A day, as it was said.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum DayExpr {
    /// "today", "tomorrow", "yesterday": so many days from today.
    FromToday(i64),
    /// "fri", "next fri", "last fri".
    Weekday { weekday: Weekday, which: Which },
    /// "next fri" said early in the week, for this week's.
    WeekdayThisWeek(Weekday),
    /// "9 dec", "dec 9 2027", "9/12".
    Date { month: u32, day: u32, year: YearRef },
    /// "the 3rd": the next time that day of the month comes round.
    DayOfMonth(u32),
    /// "christmas", "easter 2027": one of the calendar's named days.
    Holiday { index: usize, year: YearRef },
    /// "mum's birthday": one of the caller's named dates.
    Named { index: usize, year: YearRef },
    /// "2027-03-01".
    Exact(Option<Day>),
    /// "the summer bank holiday", "the next bank holiday", "the august
    /// bank holiday": a day off in the caller's calendar, in a month if
    /// one's said.
    DayOff { said: Said, month: Option<u32>, year: YearRef },
    /// "3 working days before christmas", "the day after tomorrow".
    Moved { count: i64, unit: Unit, the: bool, before: bool, base: Box<DayExpr> },
    /// "in 3 weeks", "2 days ago".
    FromNow { count: i64, unit: Unit, ago: bool },
    /// "a week on friday", "christmas + 3d".
    After { count: i64, unit: Unit, base: Box<DayExpr>, tells: bool },
    /// "the last working day of november", or of this month with no span.
    Pick { nth: i64, sort: Sort, within: Option<SpanExpr> },
    /// "the end of next month", "mid november".
    Edge { edge: Edge, within: SpanExpr },
    /// "the end of the week" as its last working day, and "the start of
    /// the week" as the Sunday before it.
    WeekEdge { edge: Edge, within: SpanExpr },
    /// "friday week", "friday fortnight".
    WeekdayWeeks { weekday: Weekday, weeks: i64 },
    /// "monday after next".
    WeekdayAfterNext(Weekday),
    /// "the friday before christmas".
    WeekdayBeside { weekday: Weekday, before: bool, base: Box<DayExpr> },
    /// "friday next week", "monday in november".
    WeekdayIn { weekday: Weekday, within: SpanExpr },
    /// "wednesday in 2 weeks", "friday 3 months from now": the one nearest
    /// that long from today.
    WeekdayNear { weekday: Weekday, count: i64, unit: Unit, ago: bool },
    /// "next week", "november": a span, for its first day.
    StartOf(SpanExpr),
}

/// How deep days can go inside one another, as in "the day before the day
/// before…", which is as deep as anyone says and keeps reading bounded.
const DEEPEST: usize = 6;

/// Reads the words of a phrase.
pub(crate) struct Parser<'a> {
    pub tokens: &'a [Token],
    pub named: &'a [Named],
    pub locale: Locale,
    pub calendar: &'a Working,
}

impl<'a> Parser<'a> {
    pub fn word(&self, at: usize) -> Option<&'a str> {
        match self.tokens.get(at)? {
            Token::Word(word) => Some(word),
            _ => None,
        }
    }

    pub fn last(&self, at: usize) -> bool {
        at + 1 == self.tokens.len()
    }

    /// Whether the word at a place is one of some words, or the start of
    /// one when it's the last word, still being typed.
    pub fn is(&self, at: usize, words: &[&str]) -> bool {
        let Some(text) = self.word(at) else { return false };
        let typing = self.last(at) && text.len() >= 2;
        words.iter().any(|word| *word == text || (typing && word.starts_with(text)))
    }

    /// Whether the word at a place is this word, in full.
    pub fn exactly(&self, at: usize, word: &str) -> bool {
        self.word(at) == Some(word)
    }

    pub fn number(&self, at: usize) -> Option<i64> {
        match self.tokens.get(at)? {
            Token::Number(n, _) => i64::try_from(*n).ok(),
            _ => None,
        }
    }

    /// "3rd", "third", "last", and "2nd to last" or "second last", as -2.
    pub fn ordinal(&self, at: usize) -> Option<Parsed<i64>> {
        let n = match self.tokens.get(at)? {
            Token::Ordinal(n) => i64::try_from(*n).ok()?,
            _ if self.is(at, &["last", "final"]) => -1,
            _ if self.is(at, &["penultimate"]) => -2,
            _ => return None,
        };
        if n > 0 && self.is(at + 1, &["to"]) && self.is(at + 2, &["last"]) {
            Some((-n, at + 3))
        } else if n > 0 && self.is(at + 1, &["last"]) {
            Some((-n, at + 2))
        } else {
            Some((n, at + 1))
        }
    }

    /// A day of the week: its name, the start of it, at least three
    /// letters or two for the last word, or one of the other ways it's cut
    /// short. Not a start more than one day has, like "t".
    pub fn weekday(&self, at: usize) -> Option<Weekday> {
        let text = self.word(at)?;
        if let Some(weekday) = weekday_alias(text) {
            return Some(weekday);
        }
        let least = if self.last(at) { 2 } else { 3 };
        let mut starting = WEEKDAY_WORDS.iter().filter(|(name, _)| name.starts_with(text));
        let (name, weekday) = starting.next()?;
        (starting.next().is_none() && (*name == text || text.len() >= least)).then_some(*weekday)
    }

    /// A month, from 1 for January: its name or the start of it, at least
    /// three letters.
    pub fn month(&self, at: usize) -> Option<u32> {
        let text = self.word(at)?;
        if text == "sept" {
            return Some(9);
        }
        if text.len() < 3 {
            return None;
        }
        let mut starting =
            MONTH_WORDS.iter().enumerate().filter(|(_, name)| name.starts_with(text));
        let (index, _) = starting.next()?;
        starting.next().is_none().then_some(index as u32 + 1)
    }

    /// A unit to move by, and how many of it the word is, when it's more
    /// than one, like a fortnight. Plural or not, and cut short, like "d".
    pub fn unit(&self, at: usize) -> Vec<(Unit, i64, usize)> {
        const UNITS: [(Unit, i64, &[&str]); 7] = [
            (Unit::WorkingDay, 1, &["workday", "workdays", "working"]),
            (Unit::Weekday, 1, &["weekday", "weekdays"]),
            (Unit::Day, 1, &["day", "days", "d"]),
            (Unit::Week, 1, &["week", "weeks", "w", "wk", "wks"]),
            (Unit::Week, 2, &["fortnight", "fortnights"]),
            (Unit::Month, 1, &["month", "months", "mo", "mos"]),
            (Unit::Year, 1, &["year", "years", "y", "yr", "yrs"]),
        ];
        let mut found: Vec<(Unit, i64, usize, bool)> = Vec::new();
        let mut add = |unit, times, last: usize, words: &[&str]| {
            if self.is(last, words)
                && !found.iter().any(|(u, _, end, _)| *u == unit && *end == last + 1)
            {
                let whole = self.word(last).is_some_and(|text| words.contains(&text));
                found.push((unit, times, last + 1, whole));
            }
        };
        if self.is(at, &["working", "work", "business"]) {
            add(Unit::WorkingDay, 1, at + 1, &["day", "days"]);
        }
        for (unit, times, words) in UNITS {
            add(unit, times, at, words);
        }
        // A whole word is that unit only, so "week" isn't also the start
        // of "weekday".
        let ends: Vec<usize> = found.iter().filter(|each| each.3).map(|each| each.2).collect();
        found.retain(|each| each.3 || !ends.contains(&each.2));
        found.into_iter().map(|(unit, times, end, _)| (unit, times, end)).collect()
    }

    /// Which days count, for picking one out: "working day", "weekday",
    /// "weekend day", "monday", or plain "day".
    pub fn sort(&self, at: usize) -> Vec<Parsed<Sort>> {
        let mut found = Vec::new();
        for (unit, times, end) in self.unit(at) {
            let sort = match unit {
                Unit::Day => Sort::Day,
                Unit::WorkingDay => Sort::WorkingDay,
                Unit::Weekday => Sort::Weekday,
                _ => continue,
            };
            if times == 1 {
                found.push((sort, end));
            }
        }
        if self.is(at, &["weekend"]) && self.is(at + 1, &["day", "days"]) {
            found.push((Sort::WeekendDay, at + 2));
        }
        if let Some(weekday) = self.weekday(at) {
            found.push((Sort::On(weekday), at + 1));
        }
        found
    }

    /// How many years on, for something said without a year: "next year",
    /// "in two years time", "2 years from now", "the year after next".
    fn years_on(&self, at: usize) -> Vec<Parsed<i64>> {
        let mut found = Vec::new();
        if self.is(at, &["next"]) && self.is(at + 1, &["year"]) {
            found.push((1, at + 2));
        }
        if self.is(at, &["this"]) && self.is(at + 1, &["year"]) {
            found.push((0, at + 2));
        }
        if self.is(at, &["year"]) && self.is(at + 1, &["after"]) && self.is(at + 2, &["next"]) {
            found.push((2, at + 3));
        }
        let said_in = self.is(at, &["in"]);
        let from = if said_in { at + 1 } else { at };
        if let Some(n) = self.number(from) {
            for (unit, _, end) in self.unit(from + 1) {
                if unit != Unit::Year {
                    continue;
                }
                if said_in {
                    found.push((n, end));
                }
                if self.is(end, &["from"]) && self.is(end + 1, &["now"]) {
                    found.push((n, end + 2));
                }
                if self.is(end, &["later", "hence"]) {
                    found.push((n, end + 1));
                }
            }
        }
        found
    }

    /// A year written out, like 2027.
    fn year(&self, at: usize) -> Option<i32> {
        self.number(at).filter(|n| (1900..=2200).contains(n)).map(|n| n as i32)
    }

    /// Which year something's in, by what follows it: nothing, a year
    /// written out, or how many years on.
    fn in_year(&self, at: usize) -> Vec<Parsed<YearRef>> {
        let mut found = vec![(YearRef::Coming, at)];
        if let Some(year) = self.year(at) {
            found.push((YearRef::Exact(year), at + 1));
        }
        found
            .extend(self.years_on(at).into_iter().map(|(years, end)| (YearRef::Ahead(years), end)));
        found
    }

    /// A span of days. With `bare`, "month" on its own is this one, as in
    /// "the last friday of the month".
    pub fn span(&self, at: usize, bare: bool, depth: usize) -> Vec<Parsed<SpanExpr>> {
        let mut found = Vec::new();
        let which = if self.is(at, &["this", "current"]) {
            Which::This
        } else if self.is(at, &["next", "coming"]) {
            Which::Next
        } else if self.is(at, &["last", "previous"]) {
            Which::Last
        } else {
            Which::Unsaid
        };
        let said = which != Which::Unsaid;
        let start = if said { at + 1 } else { at };

        // "this week", "next month", "last year", "the week after next".
        for (unit, times, end) in self.unit(start) {
            if times != 1 || !matches!(unit, Unit::Week | Unit::Month | Unit::Year) {
                continue;
            }
            if self.is(end, &["after"]) && self.is(end + 1, &["next"]) {
                found.push((SpanExpr::Period { unit, ahead: 2 }, end + 2));
            } else if said || bare {
                found.push((SpanExpr::Period { unit, ahead: which.ahead() }, end));
            }
        }

        // "week of 9 nov", "week commencing monday".
        if !said && self.exactly(start, "week") {
            let of = start + 1;
            let of =
                if self.is(of, &["commencing", "beginning", "starting"]) { of + 1 } else { of };
            for (day, end) in self.day(of, depth + 1) {
                found.push((SpanExpr::WeekOf(Box::new(day)), end));
            }
        }

        // "the weekend before christmas", "the weekend after easter".
        if !said && self.is(start, &["weekend"]) && self.is(start + 1, &["before", "after"]) {
            let before = self.is(start + 1, &["before"]);
            for (base, end) in self.day(start + 2, depth + 1) {
                found.push((SpanExpr::WeekendBeside { before, base: Box::new(base) }, end));
            }
        }

        // "the weekend", "next weekend". Not "week", which is a word of
        // its own, half typed or not, nor "weekend days".
        if self.is(start, &["weekend"])
            && !self.exactly(start, "week")
            && !self.is(start + 1, &["day", "days"])
        {
            found.push((SpanExpr::Weekend(which), start + 1));
        }

        // "november", "november 2027", "november next year", "next november".
        if let Some(month) = self.month(start) {
            for (year, end) in self.in_year(start + 1) {
                found.push((SpanExpr::Month { month, which, year }, end));
            }
        }

        // "q1", "q3 2027".
        let quarter = self.word(start).and_then(|text| match text {
            "q1" => Some(0),
            "q2" => Some(1),
            "q3" => Some(2),
            "q4" => Some(3),
            _ => None,
        });
        if let (Some(quarter), true) = (quarter, which != Which::Last) {
            for (year, end) in self.in_year(start + 1) {
                found.push((SpanExpr::Quarter { quarter, year }, end));
            }
        }

        // "2027".
        if let (Some(year), false) = (self.year(start), said) {
            found.push((SpanExpr::Year(year), start + 1));
        }

        // A day with a name that lasts more than one, like "ramadan" or
        // "half term", with a year if it's said.
        if !said {
            for (index, end) in self.holidays(start) {
                if self.calendar.named[index].lasts() {
                    for (year, end) in self.in_year(end) {
                        found.push((SpanExpr::Named { index, year }, end));
                    }
                }
            }
        }
        found
    }

    /// One day.
    pub fn day(&self, at: usize, depth: usize) -> Vec<Parsed<DayExpr>> {
        if depth > DEEPEST || at >= self.tokens.len() {
            return Vec::new();
        }
        let mut found = Vec::new();

        // "the last working day before christmas", "the final friday before
        // it", "the first monday after easter": the nearest one, as with
        // neither word.
        let nearest = match self.tokens.get(at) {
            Some(Token::Ordinal(1)) => Some(false),
            _ if self.exactly(at, "last") || self.exactly(at, "final") => Some(true),
            _ => None,
        };
        if let Some(before) = nearest {
            for (day, end) in self.day(at + 1, depth + 1) {
                // Of days, not weeks or months, which have a first and
                // last of their own.
                let beside = match &day {
                    DayExpr::Moved { count: 1, the: true, before: said, unit, .. } => {
                        *said == before
                            && matches!(unit, Unit::Day | Unit::WorkingDay | Unit::Weekday)
                    }
                    DayExpr::WeekdayBeside { before: said, .. } => *said == before,
                    _ => false,
                };
                if beside {
                    found.push((day, end));
                }
            }
        }

        // "3 working days before christmas", "2 weeks after next friday",
        // "a fortnight from tuesday", and with no number, "the day before".
        let n = self.number(at);
        for (unit, times, end) in self.unit(if n.is_some() { at + 1 } else { at }) {
            let Some(count) = n.unwrap_or(1).checked_mul(times) else { continue };
            if self.is(end, &["before", "after", "from"]) {
                let before = self.is(end, &["before"]);
                let the = n.is_none() && times == 1;
                for (base, end) in self.day(end + 1, depth + 1) {
                    found.push((
                        DayExpr::Moved { count, unit, the, before, base: Box::new(base) },
                        end,
                    ));
                }
            }
            // "in 3 weeks", "3 weeks from now", "2 days later", "3 days ago".
            let later = if self.is(end, &["from"]) && self.is(end + 1, &["now"]) {
                Some(end + 2)
            } else if self.is(end, &["later", "hence"]) {
                Some(end + 1)
            } else {
                None
            };
            let ago = self.is(end, &["ago"]).then_some(end + 1);
            let said_in = at > 0 && self.is(at - 1, &["in"]);
            if n.is_some() && (said_in || later.is_some() || ago.is_some()) {
                let end = later.or(ago).unwrap_or(end);
                found.push((DayExpr::FromNow { count, unit, ago: ago.is_some() }, end));
            }
        }
        // "a week on friday", "a week tomorrow", "3 weeks today", as said
        // in Britain: so long after a day close by.
        if let Some(n) = n {
            for (unit, times, end) in self.unit(at + 1) {
                let close =
                    self.is(end, &["today", "tomorrow", "tmrw"]) || self.weekday(end).is_some();
                if !close || !matches!(unit, Unit::Day | Unit::Week) {
                    continue;
                }
                for (base, after) in self.atom(end) {
                    if after == end + 1 {
                        let base = Box::new(base);
                        found.push((
                            DayExpr::After { count: n * times, unit, base, tells: false },
                            after,
                        ));
                    }
                }
            }
        }
        // "the fourth day after christmas", "the 3rd working day before
        // it": counted as "4 days after", since the first day after is the
        // day after. Days only: "the second week after easter" is a week.
        if let Some(Token::Ordinal(nth @ 2..)) = self.tokens.get(at)
            && let Ok(count) = i64::try_from(*nth)
        {
            for (unit, times, end) in self.unit(at + 1) {
                let days = matches!(unit, Unit::Day | Unit::WorkingDay | Unit::Weekday);
                if times != 1 || !days || !self.is(end, &["before", "after", "from"]) {
                    continue;
                }
                let before = self.is(end, &["before"]);
                for (base, end) in self.day(end + 1, depth + 1) {
                    found.push((
                        DayExpr::Moved { count, unit, the: false, before, base: Box::new(base) },
                        end,
                    ));
                }
            }
        }
        if self.is(at, &["in"]) && self.number(at + 1).is_some() {
            found.extend(self.day(at + 1, depth + 1));
        }

        // "christmas + 3d".
        let atoms = self.atom(at);
        for (base, end) in &atoms {
            let (base, end) = (base, *end);
            let (true, Some(count)) = (self.is(end, &["+"]), self.number(end + 1)) else {
                continue;
            };
            for (unit, times, end) in self.unit(end + 2) {
                let base = Box::new(base.clone());
                found.push((DayExpr::After { count: count * times, unit, base, tells: true }, end));
            }
        }

        // "the last working day of november", "the 2nd tuesday in march",
        // "the second to last friday of the month".
        if let Some((nth, end)) = self.ordinal(at) {
            for (sort, end) in self.sort(end) {
                let within = if self.is(end, &["in", "from"]) { end + 1 } else { end };
                // With no span, this month's. An "in" with nothing after
                // it yet is one still being typed.
                found.push((DayExpr::Pick { nth, sort, within: None }, end));
                if within != end && within == self.tokens.len() {
                    found.push((DayExpr::Pick { nth, sort, within: None }, within));
                }
                for (span, end) in self.span(within, true, depth) {
                    found.push((DayExpr::Pick { nth, sort, within: Some(span) }, end));
                }
            }
        }

        // "the start of next month", "the end of the year", "mid november".
        // A week's end might also be its last working day, the Friday,
        // which comes first, and its start the Sunday before, as some
        // calendars have it, which comes second.
        let edge = if self.is(at, &["start", "beginning"]) {
            Some(Edge::Start)
        } else if self.is(at, &["end"]) {
            Some(Edge::End)
        } else if self.is(at, &["middle", "mid"]) {
            Some(Edge::Middle)
        } else {
            None
        };
        if let Some(edge) = edge {
            for (within, end) in self.span(at + 1, true, depth) {
                let plain = (DayExpr::Edge { edge, within: within.clone() }, end);
                let week = (DayExpr::WeekEdge { edge, within }, end);
                match edge {
                    Edge::End => found.extend([week, plain]),
                    Edge::Start => found.extend([plain, week]),
                    Edge::Middle => found.push(plain),
                }
            }
        }

        if let Some(weekday) = self.weekday(at) {
            // "friday week" and "friday fortnight", as said in Britain: a
            // week or two after the next one.
            for (unit, times, end) in self.unit(at + 1) {
                if unit == Unit::Week
                    && (self.exactly(at + 1, "week") || self.exactly(at + 1, "fortnight"))
                {
                    found.push((DayExpr::WeekdayWeeks { weekday, weeks: times }, end));
                }
            }
            // "monday after next": the one the week after next.
            if self.is(at + 1, &["after"]) && self.exactly(at + 2, "next") {
                found.push((DayExpr::WeekdayAfterNext(weekday), at + 3));
            }
            // "the friday before christmas", "the monday after easter".
            if self.is(at + 1, &["before", "after"]) {
                let before = self.is(at + 1, &["before"]);
                for (base, end) in self.day(at + 2, depth + 1) {
                    found.push((
                        DayExpr::WeekdayBeside { weekday, before, base: Box::new(base) },
                        end,
                    ));
                }
            }
            // "thursday 5th", "fri 6 nov": the date, whatever day it says.
            let dated =
                matches!(self.tokens.get(at + 1), Some(Token::Number(..) | Token::Ordinal(_)));
            if dated || self.month(at + 1).is_some() {
                found.extend(self.atom(at + 1));
            }
            // "wednesday in 2 weeks", "friday 3 months from now", "monday 2
            // weeks ago": the one nearest that long from today.
            let far = if self.is(at + 1, &["in"]) { at + 2 } else { at + 1 };
            if self.number(far).is_some() {
                for (day, end) in self.day(far, depth + 1) {
                    if let DayExpr::FromNow { count, unit, ago } = day
                        && matches!(unit, Unit::Week | Unit::Month | Unit::Year)
                    {
                        found.push((DayExpr::WeekdayNear { weekday, count, unit, ago }, end));
                    }
                }
            }
            // "friday next week", "monday in november".
            let within = if self.is(at + 1, &["in"]) { at + 2 } else { at + 1 };
            for (span, end) in self.span(within, false, depth) {
                found.push((DayExpr::WeekdayIn { weekday, within: span }, end));
            }
        }

        // "next week", "november", taken as their first day.
        for (span, end) in self.span(at, false, depth) {
            found.push((DayExpr::StartOf(span), end));
        }

        found.extend(atoms);
        found
    }

    /// The days with names said from a place, each by its index in the
    /// calendar, with where it ends. Only where the word here could start
    /// one, which few words do.
    pub fn holidays(&self, at: usize) -> Vec<(usize, usize)> {
        let first = self.word(at);
        let starts = |word: &str| {
            first.is_some_and(|typed| {
                word == typed || (self.last(at) && typed.len() >= 2 && word.starts_with(typed))
            })
        };
        let mut found = Vec::new();
        for (index, holiday) in self.calendar.named.iter().enumerate() {
            if !starts(&holiday.words[0]) {
                continue;
            }
            let said = holiday
                .words
                .iter()
                .enumerate()
                .all(|(offset, word)| self.is(at + offset, &[word.as_str()]));
            if !said {
                continue;
            }
            // "christmas day" as well as "christmas".
            let mut end = at + holiday.words.len();
            if holiday.optional.is_some_and(|word| self.is(end, &[word])) {
                end += 1;
            }
            found.push((index, end));
        }
        found
    }

    /// A day on its own: "today", "tomorrow", "fri", "next fri", "9 dec",
    /// "christmas", "the 3rd".
    pub fn atom(&self, at: usize) -> Vec<Parsed<DayExpr>> {
        let mut found = Vec::new();
        if self.is(at, &["today", "now"]) {
            found.push((DayExpr::FromToday(0), at + 1));
        }
        if self.is(at, &["tomorrow", "tom", "tmrw"]) {
            found.push((DayExpr::FromToday(1), at + 1));
        }
        if self.is(at, &["yesterday"]) {
            found.push((DayExpr::FromToday(-1), at + 1));
        }

        let which = if self.is(at, &["next"]) {
            Which::Next
        } else if self.is(at, &["this", "coming"]) {
            Which::This
        } else if self.is(at, &["last"]) {
            Which::Last
        } else {
            Which::Unsaid
        };
        let start = if which == Which::Unsaid { at } else { at + 1 };
        if let Some(weekday) = self.weekday(start) {
            found.push((DayExpr::Weekday { weekday, which }, start + 1));
            // "next friday" is Friday next week, as Things has it, or,
            // said early in the week, this week's, which comes second.
            if which == Which::Next {
                found.push((DayExpr::WeekdayThisWeek(weekday), start + 1));
            }
        }

        // "9 dec", "dec 9", "9th december 2027", "december the 9th".
        let date = |at: usize| match self.tokens.get(at) {
            Some(Token::Number(n, _) | Token::Ordinal(n)) if (1..=31).contains(n) => {
                Some(*n as u32)
            }
            _ => None,
        };
        let pairs = [
            date(at).zip(self.month(at + 1)),
            self.month(at).zip(date(at + 1)).map(|(month, day)| (day, month)),
        ];
        for (day, month) in pairs.into_iter().flatten() {
            let end = at + 2;
            match self.year(end) {
                Some(year) => {
                    found.push((DayExpr::Date { month, day, year: YearRef::Exact(year) }, end + 1))
                }
                None => found.push((DayExpr::Date { month, day, year: YearRef::Coming }, end)),
            }
            for (years, end) in self.years_on(end) {
                found.push((DayExpr::Date { month, day, year: YearRef::Ahead(years) }, end));
            }
        }

        // "the 3rd": the next time that day of the month comes round.
        if let Some(Token::Ordinal(n)) = self.tokens.get(at)
            && (1..=31).contains(n)
            && self.sort(at + 1).is_empty()
        {
            found.push((DayExpr::DayOfMonth(*n as u32), at + 1));
        }

        // Holidays and other days with names: "christmas", "boxing day",
        // "easter", "may bank holiday", with "next year" or a year after.
        for (index, end) in self.holidays(at) {
            for (year, end) in self.in_year(end) {
                found.push((DayExpr::Holiday { index, year }, end));
            }
        }

        // The caller's own days: "mum's birthday", "my mum's birthday",
        // "Ann Smith's birthday next year", "our anniversary".
        for (index, named) in self.named.iter().enumerate() {
            let whats: Vec<&str> = named.what.iter().map(String::as_str).collect();
            let mut ends = Vec::new();
            if named.bare {
                ends.push(at);
            }
            for name in &named.names {
                let mine = self.is(at, &["my"]) && !matches!(name[0].as_str(), "my" | "our");
                let start = if mine { at + 1 } else { at };
                // "anns birthday" as well as "ann's birthday".
                let said = name.iter().enumerate().all(|(offset, word)| {
                    let typed = self.word(start + offset);
                    typed == Some(word.as_str())
                        || (offset + 1 == name.len()
                            && typed.and_then(|typed| typed.strip_suffix('s'))
                                == Some(word.as_str()))
                });
                if said {
                    ends.push(start + name.len());
                }
            }
            for end in ends {
                if self.is(end, &whats) {
                    for (year, end) in self.in_year(end + 1) {
                        found.push((DayExpr::Named { index, year }, end));
                    }
                }
            }
        }

        // "2027-03-01", "9/12", "9/12/27".
        if let Some(text) = self.word(at) {
            found.extend(written(text, self.locale).into_iter().map(|expr| (expr, at + 1)));
        }

        // Days off, by what the caller's calendar calls them: "the summer
        // bank holiday", "the next bank holiday", "the august bank holiday".
        // Not by words that are a day's own name here, like "boxing day".
        let named: Vec<usize> = found
            .iter()
            .filter(|(expr, _)| matches!(expr, DayExpr::Holiday { year: YearRef::Coming, .. }))
            .map(|(_, end)| *end)
            .collect();
        let next = usize::from(self.is(at, &["next"]));
        let starts = [Some((at + next, None)), self.month(at).map(|month| (at + 1, Some(month)))];
        for (start, month) in starts.into_iter().flatten() {
            // Only from a word a day off is said with, or one half typed.
            let typing = (start..start + self.calendar.longest()).any(|at| self.last(at));
            if !self.word(start).is_some_and(|word| typing || self.calendar.says(word)) {
                continue;
            }
            for length in 1..=self.calendar.longest() {
                let end = start + length;
                let Some(words) = (start..end).map(|at| self.word(at)).collect::<Option<Vec<_>>>()
                else {
                    break;
                };
                if named.contains(&end) && start == at {
                    continue;
                }
                if let Some(said) = self.calendar.naming(&words, self.last(end - 1)) {
                    for (year, end) in self.in_year(end) {
                        found.push((DayExpr::DayOff { said: said.clone(), month, year }, end));
                    }
                }
            }
        }
        found
    }
}

/// A date written in figures: "2027-03-01", or "9/12", "9.12.27" and
/// "9/12/2027", with the day or the month first as the style has it.
fn written(text: &str, locale: Locale) -> Vec<DayExpr> {
    let figures = |part: &str, most: usize| {
        (!part.is_empty() && part.len() <= most && part.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    // The year first is the same everywhere.
    let iso: Vec<&str> = text.split('-').collect();
    if let [year, month, day] = iso[..]
        && year.len() == 4
    {
        let (Some(year), Some(month), Some(day)) =
            (figures(year, 4), figures(month, 2), figures(day, 2))
        else {
            return Vec::new();
        };
        return vec![DayExpr::Exact(Day::new(year as i32, month, day))];
    }
    let parts: Vec<&str> = text.split(['/', '.']).collect();
    let (first, second, year) = match parts[..] {
        [first, second] => (first, second, Some(YearRef::Coming)),
        [first, second, year] if year.len() == 2 => {
            (first, second, figures(year, 2).map(|year| YearRef::Exact(2000 + year as i32)))
        }
        [first, second, year] if year.len() == 4 => {
            (first, second, figures(year, 4).map(|year| YearRef::Exact(year as i32)))
        }
        _ => return Vec::new(),
    };
    let (Some(first), Some(second), Some(year)) = (figures(first, 2), figures(second, 2), year)
    else {
        return Vec::new();
    };
    numeric(first, second, locale)
        .into_iter()
        .map(|(day, month)| DayExpr::Date { month, day, year })
        .collect()
}

/// The day and month two figures might be, in the order they're written
/// where the reader is: the day first, the month first, or either where
/// both are in use. Where only the other order makes a date, as "13/4"
/// can only be the 13th of April, it's that.
pub(crate) fn numeric(first: u32, second: u32, locale: Locale) -> Vec<(u32, u32)> {
    let real = |(day, month): (u32, u32)| (1..=12).contains(&month) && (1..=31).contains(&day);
    let (day_first, month_first) = ((first, second), (second, first));
    let orders = match locale.order() {
        Order::DayFirst => [day_first, month_first],
        Order::MonthFirst | Order::Either => [month_first, day_first],
    };
    let either = locale.order() == Order::Either && first != second;
    let mut read: Vec<(u32, u32)> = orders.into_iter().filter(|order| real(*order)).collect();
    if !either {
        read.truncate(1);
    }
    read
}

/// A day worked out, with how it was read, like "the day before Thu 30 Nov
/// 2028, the last working day of November 2028", and whose day it was read
/// from, if it was someone's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Found {
    pub day: Day,
    pub how: String,
    pub about: Option<String>,
    /// The last day of one that lasts more than one, like Ramadan, which
    /// what's after it counts from.
    pub through: Option<Day>,
}

impl Found {
    fn plain(day: Day) -> Found {
        Found { day, how: String::new(), about: None, through: None }
    }

    /// For what's after it, its last day, if it has one: "the end of
    /// Ramadan".
    fn end_if(self, after: bool) -> Found {
        match self.through.filter(|_| after) {
            Some(last) => {
                Found { day: last, how: format!("the end of {}", self.how), through: None, ..self }
            }
            None => self,
        }
    }

    fn how(day: Day, how: String) -> Found {
        Found { day, how, about: None, through: None }
    }
}

/// What kind of span one is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum SpanKind {
    Week,
    Weekend,
    Month,
    /// A quarter of a year.
    Quarter,
    Year,
    /// A day with a name that lasts more than one, like Ramadan or half
    /// term.
    Named,
}

/// A span of days worked out: first to last, and what it's called.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Span {
    pub first: Day,
    pub last: Day,
    pub name: String,
    pub kind: SpanKind,
}

/// What a tree is worked out for: the day it is, and where.
pub(crate) struct Context<'a> {
    pub today: Day,
    pub locale: Locale,
    pub calendar: &'a Working,
    pub named: &'a [Named],
}

pub(crate) fn plural(count: i64, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

/// "1st", "22nd", "last", "2nd to last".
pub(crate) fn ordinal_name(n: i64) -> String {
    if n == -1 {
        return "last".to_string();
    }
    if n < 0 {
        return format!("{} to last", ordinal_name(-n));
    }
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn join(how: String, more: &str) -> String {
    if more.is_empty() { how } else { format!("{how}, {more}") }
}

impl Context<'_> {
    fn label(&self, day: Day) -> String {
        self.locale.label(day, self.today)
    }

    /// The year something said without one is in, given the year its next
    /// time round would be.
    fn year(&self, year: YearRef) -> Option<i32> {
        match year {
            YearRef::Coming => Some(self.today.year()),
            YearRef::Exact(year) => Some(year),
            YearRef::Ahead(years) => {
                i32::try_from(i64::from(self.today.year()).checked_add(years)?).ok()
            }
        }
    }

    /// The span some weeks, months or years from the one today is in.
    fn period(&self, unit: Unit, ahead: i64) -> Option<Span> {
        let today = self.today;
        match unit {
            Unit::Week => {
                let first = self.locale.week_of(today)?.add_days(ahead.checked_mul(7)?)?;
                self.week(first)
            }
            Unit::Month => {
                let first = today.month_start().add_months(ahead)?;
                Some(month_span(first))
            }
            _ => year_span(i32::try_from(i64::from(today.year()).checked_add(ahead)?).ok()?),
        }
    }

    /// The week that starts on a day.
    fn week(&self, first: Day) -> Option<Span> {
        let name = format!("the week of {}", self.label(first));
        Some(Span { first, last: first.add_days(6)?, name, kind: SpanKind::Week })
    }

    pub fn span(&self, expr: &SpanExpr) -> Option<Span> {
        let today = self.today;
        match expr {
            SpanExpr::Period { unit, ahead } => self.period(*unit, *ahead),
            SpanExpr::WeekOf(day) => self.week(self.locale.week_of(self.day(day)?.day)?),
            SpanExpr::Weekend(which) => {
                // The coming Saturday, or on a Sunday, yesterday's: the
                // weekend it is, whichever day a week starts on.
                let from =
                    if today.weekday() == Weekday::Sunday { today.add_days(-1)? } else { today };
                let saturday = from.next(Weekday::Saturday)?.add_days(7 * which.ahead())?;
                let name = format!("the weekend of {}", self.label(saturday));
                Some(Span {
                    first: saturday,
                    last: saturday.add_days(1)?,
                    name,
                    kind: SpanKind::Weekend,
                })
            }
            SpanExpr::Month { month, which, year } => {
                let mut at = self.year(*year)?;
                // Its next time round, or its last, unless a year was given.
                if *year == YearRef::Coming {
                    let now = today.month();
                    match which {
                        Which::Unsaid if *month < now => at += 1,
                        Which::Next if *month <= now => at += 1,
                        Which::Last if *month >= now => at -= 1,
                        _ => {}
                    }
                }
                Some(month_span(Day::new(at, *month, 1)?))
            }
            SpanExpr::Quarter { quarter, year } => {
                let mut at = self.year(*year)?;
                // The next one, once this year's is over.
                if *year == YearRef::Coming && (quarter + 1) * 3 < today.month() {
                    at += 1;
                }
                let first = Day::new(at, quarter * 3 + 1, 1)?;
                let last = Day::new(at, quarter * 3 + 3, 1)?.month_end();
                Some(Span {
                    first,
                    last,
                    name: format!("Q{} {at}", quarter + 1),
                    kind: SpanKind::Quarter,
                })
            }
            SpanExpr::Year(year) => year_span(*year),
            SpanExpr::WeekendBeside { before, base } => {
                let from = self.day(base)?.end_if(!*before);
                let saturday = weekend_beside(from.day, *before)?;
                let words = if *before { "before" } else { "after" };
                Some(Span {
                    first: saturday,
                    last: saturday.add_days(1)?,
                    name: format!("the weekend {words} {}", self.label(from.day)),
                    kind: SpanKind::Weekend,
                })
            }
            SpanExpr::Named { index, year } => {
                let holiday = self.calendar.named.get(*index)?;
                let (first, last) = self.held(holiday, *year)?;
                Some(Span { first, last, name: holiday.name.clone(), kind: SpanKind::Named })
            }
        }
    }

    /// The days of a named day that can last more than one: the time
    /// that's on or next, or the one that starts in a year.
    fn held(&self, holiday: &Called, year: YearRef) -> Option<(Day, Day)> {
        match year {
            YearRef::Coming => holiday.on_or_after(self.today),
            _ => holiday.span(self.year(year)?),
        }
    }

    /// A day that comes round each year, the next time it does, or in a
    /// year given, or so many years on.
    fn yearly(&self, year: YearRef, on: impl Fn(i32) -> Option<Day>) -> Option<Day> {
        match year {
            YearRef::Coming => {
                // Far enough on for the next 29 February.
                let this = self.today.year();
                (this..=this.saturating_add(8))
                    .find_map(|year| on(year).filter(|day| *day >= self.today))
            }
            _ => on(self.year(year)?),
        }
    }

    pub fn day(&self, expr: &DayExpr) -> Option<Found> {
        let today = self.today;
        let calendar = self.calendar;
        Some(match expr {
            DayExpr::FromToday(days) => Found::plain(today.add_days(*days)?),
            DayExpr::Weekday { weekday, which } => Found::plain(match which {
                Which::Last => today.previous_before(*weekday)?,
                Which::Next => self.locale.week_of(today)?.add_days(7)?.next(*weekday)?,
                Which::Unsaid | Which::This => today.next_after(*weekday)?,
            }),
            DayExpr::WeekdayThisWeek(weekday) => {
                let day = today.next_after(*weekday)?;
                if day >= self.locale.week_of(today)?.add_days(7)? {
                    return None;
                }
                Found::plain(day)
            }
            DayExpr::Date { month, day, year } => {
                Found::plain(self.yearly(*year, |year| Day::new(year, *month, *day))?)
            }
            DayExpr::DayOfMonth(date) => {
                let first = today.month_start();
                Found::plain((0..12).find_map(|ahead| {
                    let (year, month, _) = first.add_months(ahead)?.ymd();
                    Day::new(year, month, *date).filter(|day| *day >= today)
                })?)
            }
            DayExpr::Holiday { index, year } => {
                let holiday = calendar.named.get(*index)?;
                if holiday.lasts() {
                    let (first, last) = self.held(holiday, *year)?;
                    let through = (last != first).then_some(last);
                    Found { through, ..Found::how(first, holiday.name.clone()) }
                } else {
                    let day = self.yearly(*year, |year| holiday.day(year))?;
                    Found::how(day, holiday.name.clone())
                }
            }
            DayExpr::Named { index, year } => {
                let named = self.named.get(*index)?;
                let day =
                    self.yearly(*year, |year| Day::new_within_month(year, named.month, named.day))?;
                Found { day, how: String::new(), about: named.label.clone(), through: None }
            }
            DayExpr::Exact(day) => Found::plain((*day)?),
            DayExpr::DayOff { said, month, year } => {
                // The first in the year, or the month of it, that's said.
                let coming = matches!(year, YearRef::Coming);
                let within = |year: i32| {
                    let first = Day::new(year, month.unwrap_or(1), 1)?;
                    let last = Day::new(year, month.unwrap_or(12), 1)?.month_end();
                    let from = if coming { first.max(today) } else { first };
                    calendar.find(said, from, Some(last))
                };
                let (day, name) = match year {
                    YearRef::Coming => {
                        let this = today.year();
                        (this..=this.saturating_add(8)).find_map(within)?
                    }
                    _ => within(self.year(*year)?)?,
                };
                Found::how(day, name.to_string())
            }
            DayExpr::Moved { count, unit, the, before, base } => {
                let from = self.day(base)?;
                // After one that lasts more than a day is after its last.
                let from = from.end_if(!*before);
                let day = moved(from.day, if *before { -*count } else { *count }, *unit, calendar)?;
                let length =
                    if *the { format!("the {}", unit.name()) } else { plural(*count, unit.name()) };
                let words = format!("{length} {}", if *before { "before" } else { "after" });
                Found {
                    day,
                    how: join(format!("{words} {}", self.label(from.day)), &from.how),
                    about: from.about.map(|about| format!("{words} {about}")),
                    through: None,
                }
            }
            DayExpr::FromNow { count, unit, ago } => {
                let day = moved(today, if *ago { -*count } else { *count }, *unit, calendar)?;
                let length = plural(*count, unit.name());
                Found::how(day, if *ago { format!("{length} ago") } else { format!("in {length}") })
            }
            DayExpr::After { count, unit, base, tells } => {
                let from = self.day(base)?;
                let day = moved(from.day, *count, *unit, calendar)?;
                let words = format!("{} after", plural(*count, unit.name()));
                let how = format!("{words} {}", self.label(from.day));
                if *tells {
                    Found {
                        day,
                        how: join(how, &from.how),
                        about: from.about.map(|about| format!("{words} {about}")),
                        through: None,
                    }
                } else {
                    Found::how(day, how)
                }
            }
            DayExpr::Pick { nth, sort, within } => {
                let counts = |day: Day| sort.counts(day, calendar);
                let (day, within) = match within {
                    Some(within) => {
                        let within = self.span(within)?;
                        (pick(&within, *nth, counts)?, within)
                    }
                    // This month's, or next month's once this month's has gone.
                    None => [0, 1].into_iter().find_map(|ahead| {
                        let within = self.period(Unit::Month, ahead)?;
                        let day = pick(&within, *nth, counts).filter(|day| *day >= today)?;
                        Some((day, within))
                    })?,
                };
                Found::how(
                    day,
                    format!("the {} {} of {}", ordinal_name(*nth), sort.name(), within.name),
                )
            }
            DayExpr::Edge { edge, within } => {
                let within = self.span(within)?;
                let (day, word) = match edge {
                    Edge::Start => (within.first, "start"),
                    Edge::End => (within.last, "end"),
                    Edge::Middle => {
                        (within.first.add_days(within.first.days_until(within.last) / 2)?, "middle")
                    }
                };
                Found::how(day, format!("the {word} of {}", within.name))
            }
            DayExpr::WeekEdge { edge, within } => {
                let within = self.span(within).filter(|span| span.kind == SpanKind::Week)?;
                match edge {
                    // The other day a week's taken to start on: the Sunday
                    // before one that starts on Monday, and the Monday of
                    // one that starts on Sunday.
                    Edge::Start if within.first.weekday() == Weekday::Monday => Found::how(
                        within.first.add_days(-1)?,
                        format!("the Sunday before {}", within.name),
                    ),
                    Edge::Start => Found::how(
                        within.first.next(Weekday::Monday)?,
                        format!("the Monday of {}", within.name),
                    ),
                    _ => {
                        let day = pick(&within, -1, |day| calendar.is_working_day(day))?;
                        Found::how(day, format!("the last working day of {}", within.name))
                    }
                }
            }
            DayExpr::WeekdayWeeks { weekday, weeks } => {
                Found::plain(today.next_after(*weekday)?.add_days(weeks.checked_mul(7)?)?)
            }
            DayExpr::WeekdayAfterNext(weekday) => {
                Found::plain(self.locale.week_of(today)?.add_days(14)?.next(*weekday)?)
            }
            DayExpr::WeekdayBeside { weekday, before, base } => {
                let from = self.day(base)?;
                let from = from.end_if(!*before);
                let day = if *before {
                    from.day.previous_before(*weekday)?
                } else {
                    from.day.next_after(*weekday)?
                };
                let words =
                    format!("the {} {}", weekday.name(), if *before { "before" } else { "after" });
                Found {
                    day,
                    how: join(format!("{words} {}", self.label(from.day)), &from.how),
                    about: from.about.map(|about| format!("{words} {about}")),
                    through: None,
                }
            }
            DayExpr::WeekdayIn { weekday, within } => {
                let within = self.span(within)?;
                Found::plain(pick(&within, 1, |day| day.weekday() == *weekday)?)
            }
            DayExpr::WeekdayNear { weekday, count, unit, ago } => {
                let about = moved(today, if *ago { -*count } else { *count }, *unit, calendar)?;
                let day = about.nearest(*weekday)?;
                let length = plural(*count, unit.name());
                let far = if *ago { format!("{length} ago") } else { format!("in {length}") };
                Found::how(day, format!("the {} {far}", weekday.name()))
            }
            // One with a name is read as its day, and not again here.
            DayExpr::StartOf(SpanExpr::Named { .. }) => return None,
            DayExpr::StartOf(within) => {
                let within = self.span(within)?;
                // A week, or a weekend, says its first day well enough itself.
                if matches!(within.kind, SpanKind::Week | SpanKind::Weekend) {
                    Found::plain(within.first)
                } else {
                    Found::how(within.first, format!("the start of {}", within.name))
                }
            }
        })
    }
}

fn month_span(first: Day) -> Span {
    let name = format!("{} {}", month_name(first.month()), first.year());
    Span { first, last: first.month_end(), name, kind: SpanKind::Month }
}

fn year_span(year: i32) -> Option<Span> {
    let (first, last) = (Day::new(year, 1, 1)?, Day::new(year, 12, 31)?);
    Some(Span { first, last, name: year.to_string(), kind: SpanKind::Year })
}

/// The nth day in a span that counts, or the nth from the end for -n.
fn pick(span: &Span, nth: i64, counts: impl Fn(Day) -> bool) -> Option<Day> {
    let length = span.first.days_until(span.last);
    let mut left = nth.abs();
    for offset in 0..=length {
        let day =
            if nth > 0 { span.first.add_days(offset)? } else { span.last.add_days(-offset)? };
        if counts(day) {
            left -= 1;
            if left == 0 {
                return Some(day);
            }
        }
    }
    None
}

/// The Saturday of the first weekend all before a day, or all after it.
pub(crate) fn weekend_beside(day: Day, before: bool) -> Option<Day> {
    if before {
        let saturday = day.previous_before(Weekday::Saturday)?;
        if saturday.add_days(1)? < day { Some(saturday) } else { saturday.add_days(-7) }
    } else {
        day.next_after(Weekday::Saturday)
    }
}

/// A day moved by so many of a unit, or so many working days or weekdays,
/// skipping the rest.
pub(crate) fn moved(from: Day, by: i64, unit: Unit, calendar: &Working) -> Option<Day> {
    match unit {
        Unit::Day => from.add_days(by),
        Unit::Week => from.add_days(by.checked_mul(7)?),
        Unit::Month => from.add_months(by),
        Unit::Year => from.add_months(by.checked_mul(12)?),
        Unit::WorkingDay => calendar.moved(from, by, true),
        Unit::Weekday => Working::weekdays().moved(from, by, false),
    }
}
