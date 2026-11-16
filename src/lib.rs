#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

mod calendar;
mod clock;
mod day;
mod days;
mod event;
mod locale;
mod named;
mod options;
mod repeat;
mod rule;
mod suggest;
mod words;

pub use calendar::{Calendar, DayOff};
pub use clock::read_clock;
pub use day::{Day, Time, Weekday, days_in_month, is_leap_year};
pub use days::SpanKind;
pub use event::{EventContext, EventRead, EventReading, EventRepeat};
pub use locale::Locale;
pub use named::easter;
pub use options::{NamedDate, Options, ZoneNames};
pub use repeat::{Repeat, RepeatKind};
pub use rule::{ByDay, Frequency, Rule};
pub use words::name_words;

use calendar::Working;
use days::{Context, Parser};
use options::Named;

/// A day a phrase might mean.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DayReading {
    /// The day.
    pub day: Day,
    /// How it was read, like "the day before Thu 30 Nov 2028, the last
    /// working day of November 2028", or nothing for a day on its own.
    pub how: String,
    /// Whose day it was read from, as it would be said, like "2 weeks
    /// before Eli Jones’s birthday", if it was read from a named date.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub about: Option<String>,
}

/// A span of days a phrase names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SpanReading {
    /// Its first day.
    pub first: Day,
    /// Its last day.
    pub last: Day,
    /// What kind of span it is.
    pub unit: SpanKind,
}

/// A day what's typed in a date picker's search might mean.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DaySuggestion {
    /// The day.
    pub day: Day,
    /// What to call it, like "Friday" or "In 3 days", or nothing when the
    /// day's own label says it best.
    pub name: String,
    /// Whose day it was worked out from, like "2 weeks before Eli Jones’s
    /// birthday".
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub about: Option<String>,
}

/// A way a phrase might say something repeats.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RepeatReading {
    /// How it repeats.
    pub repeat: Repeat,
    /// How it was read, like "Every 2 weeks on Tuesday".
    pub how: String,
    /// The next few days it comes round on, or for one after it's done,
    /// the day it would next if it were done today.
    pub next: Vec<Day>,
}

/// Reads what's typed, for somewhere and someone: see [`Options`].
#[derive(Clone, Debug, Default)]
pub struct Reader {
    options: Options,
    calendar: Working,
    named: Vec<Named>,
    pub(crate) zones: ZoneNames,
    /// Every word of every zone's name, for telling a word that could be
    /// part of when something is from one that couldn't.
    pub(crate) zone_words: std::collections::HashSet<String>,
}

impl Reader {
    /// A reader for somewhere.
    pub fn new(options: Options) -> Reader {
        Reader {
            options,
            calendar: Working::weekdays(),
            named: Vec::new(),
            zones: ZoneNames::default(),
            zone_words: Default::default(),
        }
    }

    /// With the days that aren't worked, which only the caller knows:
    /// public holidays, each with what it's called, and which days of
    /// the week are worked. Without one, a working day is Monday to
    /// Friday.
    pub fn with_calendar(mut self, calendar: Calendar) -> Reader {
        self.calendar = Working::from(calendar);
        self
    }

    /// Whether a day is worked, by the reader's calendar.
    pub fn is_working_day(&self, day: Day) -> bool {
        self.calendar.is_working_day(day)
    }

    /// With days that have names only the caller knows, like birthdays.
    pub fn with_named_dates(mut self, dates: &[NamedDate]) -> Reader {
        self.named = dates.iter().map(Named::from).collect();
        self
    }

    /// With the names time zones go by, for events to be set in.
    pub fn with_zones(mut self, zones: ZoneNames) -> Reader {
        let names = zones.before_time.iter().chain(&zones.after_time);
        self.zone_words = names.flat_map(|(name, _)| name.split(' ')).map(str::to_string).collect();
        self.zones = zones;
        self
    }

    pub(crate) fn parser<'a>(&'a self, tokens: &'a [words::Token]) -> Parser<'a> {
        Parser { tokens, named: &self.named, locale: self.options.locale, calendar: &self.calendar }
    }

    fn context(&self, today: Day) -> Context<'_> {
        Context { today, locale: self.options.locale, calendar: &self.calendar, named: &self.named }
    }

    /// A reader that also reads "his", "her" and "their" as whoever some
    /// text names, like the title of the thing a day's being picked for:
    /// about "Plan Eli's party", "2 weeks before his birthday" is before
    /// Eli's. Whether the word fits the person isn't checked.
    pub fn about(&self, text: &str) -> Reader {
        let said: Vec<String> = words::tokens(text)
            .into_iter()
            .map(|token| match token {
                words::Token::Word(word) | words::Token::Number(_, word) => word,
                words::Token::Ordinal(_) => String::new(),
            })
            .collect();
        let mut reader = self.clone();
        for named in &self.named {
            // "My", "me" and "our" are the reader's own, not who it's about.
            let is_said = named.names.iter().any(|name| {
                !matches!(name[0].as_str(), "my" | "me" | "our")
                    && said.windows(name.len()).any(|words| words == name.as_slice())
            });
            if is_said {
                let names =
                    ["his", "her", "their"].iter().map(|word| vec![word.to_string()]).collect();
                reader.named.push(Named { names, bare: false, ..named.clone() });
            }
        }
        reader
    }

    /// Every day a phrase might mean, by every reading that uses all its
    /// words, the likeliest first. A day read more than one way is given
    /// once, as it was read first.
    pub fn days(&self, text: &str, today: Day) -> Vec<DayReading> {
        let tokens = words::tokens(text);
        let parser = self.parser(&tokens);
        let context = self.context(today);
        let mut readings: Vec<DayReading> = Vec::new();
        for (expr, end) in parser.day(0, 0) {
            if end != tokens.len() {
                continue;
            }
            let Some(found) = context.day(&expr) else { continue };
            if !readings.iter().any(|reading| reading.day == found.day) {
                readings.push(DayReading { day: found.day, how: found.how, about: found.about });
            }
        }
        readings
    }

    /// The span of days a phrase names, if it's one, like "next week",
    /// "the weekend", "week of 9 nov" or "november".
    pub fn span(&self, text: &str, today: Day) -> Option<SpanReading> {
        let tokens = words::tokens(text);
        let parser = self.parser(&tokens);
        let context = self.context(today);
        parser.span(0, true, 0).into_iter().filter(|(_, end)| *end == tokens.len()).find_map(
            |(expr, _)| {
                let span = context.span(&expr)?;
                Some(SpanReading { first: span.first, last: span.last, unit: span.kind })
            },
        )
    }

    fn reading(&self, repeat: Repeat, how: String, today: Day) -> RepeatReading {
        let from = if repeat.kind == RepeatKind::After { today } else { repeat.starts };
        let next = self.next_days(&repeat, from, 4);
        RepeatReading { repeat, how, next }
    }

    /// Every way a phrase might say something repeats, by every reading
    /// that uses all its words. `from` is the day the thing is on now,
    /// which "every week" and "monthly" take their days from.
    pub fn repeats(&self, text: &str, from: Day, today: Day) -> Vec<RepeatReading> {
        let tokens = words::tokens(&repeat::said(text));
        if tokens.is_empty() {
            return Vec::new();
        }
        let reader =
            repeat::RepeatParser { parser: self.parser(&tokens), context: self.context(today) };
        let mut readings: Vec<RepeatReading> = Vec::new();
        for repeat in reader.repeats(from) {
            let mut how = self.describe_repeat(&repeat, Some(today));
            // Every other week from next week reads the same as from this
            // one but for the day it starts.
            if readings.iter().any(|reading| reading.how == how) {
                how =
                    format!("{how}, from {}", self.options.locale.long(repeat.starts, Some(today)));
            }
            readings.push(self.reading(repeat, how, today));
        }
        readings
    }

    /// What a picker offers before anything's typed, from the day the
    /// thing is on: every day, weekday, week, month and year as of that
    /// day, and a week after it's done.
    pub fn suggest_repeats(&self, from: Day, today: Day) -> Vec<RepeatReading> {
        repeat::suggestions(from, today)
            .into_iter()
            .map(|repeat| {
                let how = self.describe_repeat(&repeat, Some(today));
                self.reading(repeat, how, today)
            })
            .collect()
    }

    /// How something repeats, as people say it, like "Every 2 weeks on
    /// Tuesday" or "3 weeks after it’s done". The year of `today` is left
    /// off the day it ends.
    pub fn describe_repeat(&self, repeat: &Repeat, today: Option<Day>) -> String {
        match repeat.kind {
            RepeatKind::After => repeat.rule.describe_after(),
            RepeatKind::Schedule => repeat.rule.describe(repeat.starts, today, self.options.locale),
        }
    }

    /// The next few days a repeat comes round on, from a day on: for one
    /// after it's done, the day it would be if it were done that day.
    pub fn next_days(&self, repeat: &Repeat, from: Day, many: usize) -> Vec<Day> {
        match repeat.kind {
            RepeatKind::After => repeat.rule.after_done(from).into_iter().collect(),
            // Far enough for any rule to have come round that many times,
            // short of one every few years.
            RepeatKind::Schedule => {
                let to = from.add_months(12 * 30).unwrap_or(Day::MAX);
                repeat.rule.days(repeat.starts, from, to, many, &self.calendar)
            }
        }
    }

    /// The days a reading comes round on, as a line under how it was read:
    /// "Tue 3 Nov, 17 Nov, 1 Dec", with the weekday only on the first when
    /// they're all on the same one.
    pub fn next_line(&self, reading: &RepeatReading, today: Day) -> String {
        let style = self.options.locale;
        let Some(first) = reading.next.first() else { return "No more days".to_string() };
        if reading.repeat.kind == RepeatKind::After {
            return format!("Next on {}, if it’s done today", style.short(*first, today));
        }
        let same = reading.next.iter().all(|day| day.weekday() == first.weekday());
        let said = reading.next.iter().enumerate().map(|(index, day)| {
            if same && index > 0 { style.short(*day, today) } else { style.label(*day, today) }
        });
        said.collect::<Vec<_>>().join(", ")
    }

    /// Reads an event typed out as a sentence, like "Lunch with Priya
    /// tomorrow at 1 at Monmouth": when it is, how it repeats, where it
    /// is, who's mentioned, and the title that's left.
    pub fn event(&self, text: &str, context: &EventContext) -> EventReading {
        event::read(self, text, context)
    }

    /// The days what's typed in a date picker's search might mean, the
    /// likeliest first, as it's typed: "t" is Today, Tomorrow, Tuesday and
    /// Thursday, "3" the 3rd and in three days, weeks, months and years.
    /// Never a day before today.
    pub fn suggest_days(&self, typed: &str, today: Day) -> Vec<DaySuggestion> {
        let text = suggest::searched(typed);
        if text.is_empty() {
            return Vec::new();
        }
        // The short forms first, which know half-typed words best, then
        // whatever else a longer phrase might mean.
        let short = suggest::Suggester { today, locale: self.options.locale }.days(&text);
        let read = self.days(&text, today).into_iter().filter(|reading| reading.day >= today);
        let mut found: Vec<DaySuggestion> = Vec::new();
        let all = short
            .into_iter()
            .map(|(day, name)| (day, name, None))
            .chain(read.map(|reading| (reading.day, None, reading.about)));
        for (day, name, about) in all {
            if found.iter().any(|each| each.day == day) {
                continue;
            }
            let name = if day == today { "Today".to_string() } else { name.unwrap_or_default() };
            found.push(DaySuggestion { day, name, about });
        }
        found
    }

    /// What's typed with each word that's one letter off a word dates are
    /// said with put right, like "tmorrow" for tomorrow, for searching
    /// again when nothing was found. `None` when no word is. The names of
    /// named dates are left as they are, so Mark isn't March, and the
    /// words in `more` are put right too.
    pub fn corrected(&self, typed: &str, more: &[&str]) -> Option<String> {
        let names = self.named.iter().flat_map(|named| named.names.iter().flatten());
        let keep: Vec<&str> = names.map(String::as_str).collect();
        let whats = self.named.iter().flat_map(|named| named.what.iter().map(String::as_str));
        // And the words of the days with names here, like "thanksgiving".
        let days = named::NAMED_DAYS
            .iter()
            .filter(|day| day.said_in(self.options.locale))
            .flat_map(|day| day.words.iter().copied())
            .filter(|word| word.len() >= 8);
        let more: Vec<&str> = more.iter().copied().chain(whats).collect();
        suggest::corrected(typed, &more, &days.collect::<Vec<_>>(), &keep)
    }

    /// Whether a word on its own is a day's name here, like "halloween"
    /// or "thanksgiving".
    pub(crate) fn names_a_day(&self, word: &str) -> bool {
        named::NAMED_DAYS.iter().any(|day| day.said_in(self.options.locale) && day.words == [word])
    }
}
