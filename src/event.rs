//! Reads an event typed out as a sentence: "Lunch with Priya tomorrow at 1
//! at Monmouth", "Standup every weekday at 9:30 for 15 mins", "Trip to
//! Lisbon 6-8 nov". Out of it come
//!
//! - when: a day, as the day reader reads days, a time, like "1pm",
//!   "13:30", "at 7", "half three" or "noon", or between two, like "2-4pm"
//!   or "from 1 to 3", and how long, like "for 2 hours", "30 min call" or
//!   "for 3 days". A day with no time is over the whole day. Days can run
//!   from one to another, like "6-8 nov", "fri to sun" or "fri 6pm to sun
//!   5pm", or over a span, like "this weekend" or "all of november".
//! - how it repeats, as the repeat reader reads it: "every other friday".
//! - where: what follows "at" that isn't a time.
//! - who: the mentions the caller marks, however they're written.
//! - the time zone its times are in, when one's named: "9am New York
//!   time", "10:00 PT".
//!
//! and the title is what's left. Each word says what it was read as, so
//! what's typed can show it.
//!
//! When something is can be read on its own, with the same passes but for
//! how it repeats and where it is, and nothing assumed: no time when none
//! is said, and no length for one. It's nothing if any word is left over.
//!
//! Each word of the sentence starts as part of the title. One pass after
//! another takes the words it reads as something else, in an order that
//! settles what's ambiguous: the zone before the place, so a city isn't
//! where it is; ranges of days before single days; times before days, so
//! "friday 3" is at three, not the 3rd.
//!
//! A bare number is only a time after "at" or "from", or beside a day or
//! what the event is, like "friday 3" or "breakfast 6". With no am or pm
//! it's in the hours what the event is is usually in, so "breakfast 6" is
//! at 6am and "dinner 6" at 6pm. A word that's a day on its own only when
//! it's half typed, or a month, like "may" or "the sun", is only read as
//! one beside others, like "may 5".

use crate::clock::{
    Clock, Hours, clock_at, clock_time, figures, hour_word, plain_word, range_start,
};
use crate::day::{Day, Time};
use crate::options::{MONTH_WORDS, WEEKDAY_WORDS, ZoneNames};
use crate::repeat::{Repeat, RepeatKind};
use crate::words::{Token, tokens_of};
use crate::{DayReading, Reader, SpanKind};

/// What an event's sentence is read with.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventContext {
    /// The day it is.
    pub today: Day,
    /// The day it's on if nothing says.
    pub fallback_day: Day,
    /// The time it's at if nothing says.
    pub fallback_time: Time,
    /// How long it lasts, in minutes, when it's at a time and nothing
    /// says for how long, or what it is doesn't. An hour unless said.
    #[cfg_attr(feature = "serde", serde(default = "an_hour"))]
    pub default_length: u32,
    /// Who's mentioned, in the order they're in the sentence.
    #[cfg_attr(feature = "serde", serde(default))]
    pub mentions: Vec<Mention>,
}

#[cfg(feature = "serde")]
fn an_hour() -> u32 {
    60
}

impl EventContext {
    /// For a day, with nothing said: an event on that day, at a time,
    /// for an hour, mentioning no one.
    pub fn new(today: Day, fallback_time: Time) -> EventContext {
        EventContext {
            today,
            fallback_day: today,
            fallback_time,
            default_length: 60,
            mentions: Vec::new(),
        }
    }
}

/// Someone or something mentioned in an event's sentence, as the caller
/// marks them, however that is: `#Priya Raman` or `@Priya`.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mention {
    /// As it's written in the sentence, like "#Priya Raman". Matched
    /// whatever its case, and with "'s" after it or not.
    pub written: String,
    /// As the title says it, like "Priya Raman".
    pub said: String,
}

impl Mention {
    /// A mention written one way and said another.
    pub fn new(written: impl Into<String>, said: impl Into<String>) -> Mention {
        Mention { written: written.into(), said: said.into() }
    }
}

/// What was read from what was typed, rather than taken as it is when
/// nothing says.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct EventRead {
    /// Whether a day was said.
    pub day: bool,
    /// Whether a time was said.
    pub time: bool,
    /// Whether how long it lasts was said.
    pub length: bool,
    /// The meal its time was taken from, if no time was given.
    pub meal: Option<Meal>,
}

/// A meal, which says when something is and for how long when nothing
/// else does: "Lunch with Priya" is at noon, for an hour.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[non_exhaustive]
pub enum Meal {
    /// At 8, for an hour.
    Breakfast,
    /// At 11, for an hour and a half.
    Brunch,
    /// At noon, for an hour: "lunch", "lunchtime".
    Lunch,
    /// At 6pm, for two hours.
    Drinks,
    /// At 7pm, for two hours: "dinner", "supper".
    Dinner,
}

/// How an event repeats, and how that was read.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct EventRepeat {
    /// How it repeats, from the day the event starts.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub repeat: Repeat,
    /// How it was read, like "Every 2 weeks on Tuesday".
    pub how: String,
}

/// An event, as its sentence says it.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct EventReading {
    /// What it's called: what's left of the sentence.
    pub title: String,
    /// Which of the mentions given were found, by their place among them,
    /// in the order they're in.
    pub mentioned: Vec<usize>,
    /// The day it starts.
    pub start_day: Day,
    /// The day it ends.
    pub end_day: Day,
    /// The time it starts, or `None` over whole days.
    pub start_time: Option<Time>,
    /// The time it ends, or `None` over whole days.
    pub end_time: Option<Time>,
    /// Where it is, if it's said.
    pub location: Option<String>,
    /// How it repeats, if it does.
    pub repeat: Option<EventRepeat>,
    /// The time zone its times are in, when one's named.
    pub time_zone: Option<String>,
    /// What was read, rather than taken as it is when nothing says.
    pub read: EventRead,
    /// Other days what was typed might start on, like the end of the
    /// week's Sunday after its Friday.
    pub others: Vec<Day>,
    /// Each word typed, and what it was read as, with a mention's words
    /// as one.
    pub words: Vec<ReadWord>,
}

impl EventReading {
    /// Whether it's over whole days, with no times.
    pub fn is_all_day(&self) -> bool {
        self.start_time.is_none()
    }
}

/// When something is, said with nothing else: "tomorrow 2-4pm", "fri 6pm
/// to sun 5pm", "next week", "at 3 for an hour", read as an event's
/// sentence would be read.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct WhenReading {
    /// The day it starts: today, if only a time is said.
    pub start_day: Day,
    /// The day it ends.
    pub end_day: Day,
    /// The time it starts, or `None` over whole days.
    pub start_time: Option<Time>,
    /// The time it ends, or `None` over whole days, or when it's at a
    /// time and not said until when, like "at 3", as nothing's assumed.
    pub end_time: Option<Time>,
    /// The time zone its times are in, when one's named.
    pub time_zone: Option<String>,
    /// Other days it might start on, like the end of the week's Sunday
    /// after its Friday.
    pub others: Vec<Day>,
    /// Each word typed, and what it was read as.
    pub words: Vec<ReadWord>,
}

impl WhenReading {
    /// Whether it's over whole days, with no times.
    pub fn is_all_day(&self) -> bool {
        self.start_time.is_none()
    }
}

/// A word of what was typed, and what it was read as, for showing which
/// words said what as they're typed, like the day and time in a title
/// field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ReadWord {
    /// Where it starts in the text, in bytes, as `&text[start..end]`.
    pub start: usize,
    /// Where it ends in the text, in bytes.
    pub end: usize,
    /// What it was read as.
    pub role: WordRole,
}

/// What a word was read as. A word that leads to what it's part of, like
/// "on" in "on friday", "for" in "for 2 hours" or "at" before a place,
/// is read as what it leads to, and punctuation on its own after what
/// was read, like "," in "tomorrow , 3pm", as what it follows.
///
/// With serde, each is its name in snake case, like `"day"` or
/// `"time_zone"`, but a mention, which is `{"mention": 0}`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum WordRole {
    /// Part of the title: what's left.
    Title,
    /// One of the mentions given, by its place among them: "#Priya", all
    /// of its words as one.
    Mention(usize),
    /// A day, or days: "tomorrow", "next friday", "6-8 nov", "this
    /// weekend", "all of november".
    Day,
    /// A time, or between two, or a part of the day: "at 3", "2-4pm",
    /// "half three", "tomorrow morning"'s "morning".
    Time,
    /// How long: "for 2 hours", "30 min", "all day", "for 3 nights".
    Length,
    /// How it repeats: "every other friday", "weekly until christmas".
    Repeat,
    /// The time zone its times are in: "New York time", "PT".
    TimeZone,
    /// Where it is: "at Dishoom".
    Place,
}

impl WordRole {
    /// Whether it says when.
    fn is_when(self) -> bool {
        matches!(
            self,
            WordRole::Day
                | WordRole::Time
                | WordRole::Length
                | WordRole::Repeat
                | WordRole::TimeZone
        )
    }
}

use WordRole as Role;

#[derive(Clone, Debug)]
struct Word {
    /// As typed, for the title and the place.
    text: String,
    /// In lower case, without what punctuation ends it, to read.
    plain: String,
    role: Role,
    /// Where it is in what was typed, in bytes.
    start: usize,
    end: usize,
}

const EVENING: Hours = Hours::new(17, 24);

/// Words that are a day on their own. Any other word read as one, like
/// "may", "march", "sun" or "to", half typed for "today", has to be beside
/// another to count, unless it's still being typed.
const DAY_WORDS: [&str; 26] = [
    "today",
    "tonight",
    "tomorrow",
    "yesterday",
    "tmrw",
    "tmr",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "mon",
    "tue",
    "tues",
    "wed",
    "weds",
    "thu",
    "thur",
    "thurs",
    "fri",
    "christmas",
    "xmas",
    "easter",
    "weekend",
];
/// Words that are part of a day, but say nothing on their own.
const FILLERS: [&str; 5] = ["on", "the", "this", "next", "by"];
/// Words that start saying how it repeats.
const REPEATS: [&str; 33] = [
    "every",
    "each",
    "alternate",
    "daily",
    "nightly",
    "weekly",
    "fortnightly",
    "biweekly",
    "monthly",
    "bimonthly",
    "quarterly",
    "biannually",
    "biannual",
    "semiannually",
    "semiannual",
    "half-yearly",
    "six-monthly",
    "yearly",
    "annually",
    "biennially",
    "biennial",
    "triennially",
    "triennial",
    "weekdays",
    "weekends",
    "workdays",
    "mondays",
    "tuesdays",
    "wednesdays",
    "thursdays",
    "fridays",
    "saturdays",
    "sundays",
];
/// Whether the words from a place start to say how something repeats: one
/// of the words that always do, or "once" or "half" with the word that
/// makes them, as in "once a week" and "half yearly", since "once" and
/// "half" are said for much else, as in "half three".
fn says_repeat(words: &[&str]) -> bool {
    match words {
        [word, ..] if REPEATS.contains(word) => true,
        ["once", next, ..] => matches!(*next, "a" | "an" | "per" | "every" | "each"),
        ["half", "yearly", ..] => true,
        _ => false,
    }
}

/// Words left behind by what was read after them, like "on" in "on friday".
const JOINERS: [&str; 9] =
    ["on", "at", "from", "between", "starting", "by", "for", "over", "during"];
/// Words before a time that say it's about then, like "around 9".
const ABOUT: [&str; 7] = ["around", "about", "approx", "approximately", "roughly", "circa", "~"];
/// Words before when something ends.
const UNTIL: [&str; 6] = ["until", "till", "til", "untill", "through", "thru"];

fn leads(word: &str) -> bool {
    matches!(word, "at" | "@" | "from" | "between") || ABOUT.contains(&word)
}

/// Words between the times or days something's from and to.
fn joins(word: &str) -> bool {
    matches!(word, "to" | "-" | "–" | "—") || UNTIL.contains(&word)
}

impl Meal {
    /// The meal a word names.
    fn named(word: &str) -> Option<Meal> {
        Some(match word {
            "breakfast" => Meal::Breakfast,
            "brunch" => Meal::Brunch,
            "lunch" | "lunchtime" => Meal::Lunch,
            "drinks" => Meal::Drinks,
            "dinner" | "supper" => Meal::Dinner,
            _ => return None,
        })
    }

    /// When it usually is, and for how many minutes.
    fn usual(self) -> (Time, i64) {
        let (hour, minutes) = match self {
            Meal::Breakfast => (8, 60),
            Meal::Brunch => (11, 90),
            Meal::Lunch => (12, 60),
            Meal::Drinks => (18, 120),
            Meal::Dinner => (19, 120),
        };
        (Time::new(hour, 0).expect("a time"), minutes)
    }
}

/// What an event is, and the hours it's usually in, so a bare hour said
/// with it is the one in them.
fn kind(word: &str) -> Option<Hours> {
    Some(match word {
        "breakfast" => Hours::new(5, 12),
        "brunch" => Hours::new(9, 15),
        "lunch" | "lunchtime" => Hours::new(11, 16),
        "tea" => Hours::new(14, 20),
        "dinner" | "supper" | "drinks" | "party" | "gig" | "concert" | "theatre" | "theater"
        | "pub" | "quiz" => EVENING,
        _ => return None,
    })
}

/// A part of the day: the hours it is, and when something in it is when
/// nothing says, like "Call mum tomorrow morning".
#[derive(Clone, Copy, Debug)]
struct Part {
    hours: Hours,
    time: Option<Time>,
}

fn part(word: &str) -> Option<Part> {
    let (hours, time) = match word {
        "morning" => (Hours::new(4, 12), Time::new(9, 0)),
        "afternoon" => (Hours::new(12, 18), Time::new(14, 0)),
        "evening" | "tonight" => (EVENING, Time::new(19, 0)),
        "night" => (Hours::new(18, 6), None),
        _ => return None,
    };
    Some(Part { hours, time })
}

/// Whether a word could be part of saying when something is, other than
/// as part of a day: a number, or a word a time, a part of the day or how
/// long is said with. Any word that isn't, and isn't part of a day or a
/// time zone's name, ends the search for a range of days at it.
fn could_say_when(word: &str) -> bool {
    const WORDS: [&str; 62] = [
        "at",
        "@",
        "from",
        "between",
        "to",
        "and",
        "am",
        "pm",
        "a.m",
        "p.m",
        "o'clock",
        "oclock",
        "noon",
        "midday",
        "midnight",
        "half",
        "quarter",
        "past",
        "after",
        "of",
        "til",
        "till",
        "until",
        "untill",
        "through",
        "thru",
        "twenty",
        "twenty-five",
        "fifteen",
        "thirty",
        "forty",
        "forty-five",
        "morning",
        "afternoon",
        "evening",
        "tonight",
        "night",
        "in",
        "the",
        "this",
        "on",
        "by",
        "for",
        "over",
        "during",
        "starting",
        "an",
        "a",
        "couple",
        "pair",
        "few",
        "all",
        "whole",
        "all-day",
        "allday",
        "throughout",
        "rest",
        "next",
        "time",
        "fortnight",
        "-",
        "–",
    ];
    word.starts_with(|c: char| c.is_ascii_digit())
        || WORDS.contains(&word)
        || ABOUT.contains(&word)
        || hour_word(word).is_some()
        || is_unit(word)
        || word == "—"
        || word == "m"
}

/// A month, in full or cut short, like "nov" or "sept".
fn is_month(word: &str) -> bool {
    word.len() >= 3 && (word == "sept" || MONTH_WORDS.iter().any(|month| month.starts_with(word)))
}

/// A word for how long, after a number.
fn is_unit(word: &str) -> bool {
    matches!(
        word,
        "min"
            | "mins"
            | "minute"
            | "minutes"
            | "hour"
            | "hours"
            | "hr"
            | "hrs"
            | "h"
            | "day"
            | "days"
            | "week"
            | "weeks"
            | "night"
            | "nights"
    )
}

fn is_minutes(word: &str) -> bool {
    matches!(word, "min" | "mins" | "minute" | "minutes")
}

fn is_hours(word: &str) -> bool {
    matches!(word, "hour" | "hours" | "hr" | "hrs" | "h")
}

fn ordinal_suffix(word: &str) -> Option<&str> {
    ["st", "nd", "rd", "th"].iter().find_map(|suffix| word.strip_suffix(suffix))
}

/// The day of a month, as in "5" or "5th".
fn is_date(word: &str) -> bool {
    figures(ordinal_suffix(word).unwrap_or(word)).is_some()
}

fn all_figures(word: &str, least: usize, most: usize) -> bool {
    (least..=most).contains(&word.len()) && word.bytes().all(|byte| byte.is_ascii_digit())
}

/// A day written in figures alone, like "5th", "2027-03-01", "9/12" or
/// "5.11.2026".
fn is_written_day(word: &str) -> bool {
    if ordinal_suffix(word).is_some_and(|number| all_figures(number, 1, 2)) {
        return true;
    }
    let parts = |separator: char| word.split(separator).collect::<Vec<_>>();
    match (&parts('-')[..], &parts('/')[..], &parts('.')[..]) {
        ([year, month, day], ..) => {
            all_figures(year, 4, 4) && all_figures(month, 1, 2) && all_figures(day, 1, 2)
        }
        (_, [day, month], _) => all_figures(day, 1, 2) && all_figures(month, 1, 2),
        (_, [day, month, year], _) | (_, _, [day, month, year]) => {
            all_figures(day, 1, 2) && all_figures(month, 1, 2) && all_figures(year, 2, 4)
        }
        _ => false,
    }
}

/// A number, with a fraction or not, like "2" or "1.5".
fn number(word: &str) -> Option<f64> {
    let (whole, fraction) = word.split_once('.').unwrap_or((word, "0"));
    if !all_figures(whole, 1, 9) || !all_figures(fraction, 1, 9) {
        return None;
    }
    // Put together from its figures, which is all a length of time needs,
    // rather than by the standard library's reader of any number at all.
    let figures =
        |text: &str| text.bytes().fold(0.0, |sum, byte| sum * 10.0 + f64::from(byte - b'0'));
    Some(figures(whole) + figures(fraction) / 10f64.powi(fraction.len() as i32))
}

/// A time in figures, like "3", "3pm" or "9.30a": whether it has minutes,
/// and what it ends with, of "am", "pm", "a" and "p".
fn time_figures(word: &str) -> Option<(bool, &str)> {
    let digits = word.len() - word.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let (hour, rest) = word.split_at(digits);
    if !all_figures(hour, 1, 2) {
        return None;
    }
    let (minutes, suffix) = match rest.strip_prefix([':', '.']) {
        Some(rest) => {
            let (minutes, suffix) = rest.split_at_checked(2)?;
            if !all_figures(minutes, 2, 2) {
                return None;
            }
            (true, suffix)
        }
        None => (false, rest),
    };
    matches!(suffix, "" | "am" | "pm" | "a" | "p").then_some((minutes, suffix))
}

/// The two sides of a word with one dash in it, like "2-4pm" or "fri-sun".
fn dashed(word: &str) -> Option<(&str, &str)> {
    let is_dash = |c: char| matches!(c, '-' | '–' | '—');
    let (left, right) = word.split_once(is_dash)?;
    (!left.is_empty() && !right.is_empty() && !right.contains(is_dash)).then_some((left, right))
}

type Times = (Option<Time>, Option<EndTime>, bool);

/// When something ends: at a time, or at a bare hour that's the first
/// after whenever it starts.
#[derive(Clone, Copy, Debug)]
enum EndTime {
    At(Time),
    After(Clock),
}

impl EndTime {
    fn after(self, start: Time) -> Time {
        match self {
            EndTime::At(time) => time,
            EndTime::After(clock) => clock_time(clock, Some(start), None),
        }
    }
}

/// From one day to another, with times if both ends have them.
#[derive(Clone, Copy, Debug)]
struct Range {
    start_day: Day,
    end_day: Day,
    times: Option<(Time, Time)>,
}

/// One side of a range: a day, and maybe a time, and nothing else.
#[derive(Clone, Copy, Debug)]
struct End {
    day: Day,
    time: Option<Time>,
    span: Option<(Day, Day)>,
}

struct Sentence<'a> {
    reader: &'a Reader,
    context: &'a EventContext,
    words: Vec<Word>,
    /// Whether it's an event's, rather than only when.
    event: bool,
}

impl Sentence<'_> {
    fn plain(&self, index: usize) -> Option<&str> {
        self.words.get(index).map(|word| word.plain.as_str())
    }

    fn before(&self, index: usize, back: usize) -> Option<&str> {
        index.checked_sub(back).and_then(|at| self.plain(at))
    }

    fn free(&self, index: usize) -> bool {
        self.words.get(index).is_some_and(|word| word.role == Role::Title)
    }

    fn role(&self, index: usize) -> Option<Role> {
        self.words.get(index).map(|word| word.role)
    }

    fn take(&mut self, from: usize, to: usize, role: Role) {
        let to = to.min(self.words.len()).max(from);
        for word in &mut self.words[from..to] {
            word.role = role;
        }
    }

    fn plains(&self) -> Vec<&str> {
        self.words.iter().map(|word| word.plain.as_str()).collect()
    }

    fn said(&self, from: usize, to: usize) -> String {
        self.plains()[from..to].join(" ")
    }

    /// What's left for the title, each mention as the caller says it.
    fn title(&self) -> String {
        let words: Vec<&str> = self
            .words
            .iter()
            .filter(|word| matches!(word.role, Role::Title | Role::Mention(_)))
            .map(|word| word.text.as_str())
            .collect();
        words.join(" ").trim_end_matches([' ', ',', ';', ':', '–', '-']).to_string()
    }

    /// Each mention given, in the order they're in, as it's written, with
    /// "'s" after it or not, as in "#Harry's". Its words become one, said
    /// as the caller says it, with whatever followed it.
    fn mentions(&mut self) -> Vec<usize> {
        let mut found = Vec::new();
        let labels: Vec<(usize, Vec<String>)> = self
            .context
            .mentions
            .iter()
            .enumerate()
            .map(|(index, mention)| {
                (index, mention.written.split_whitespace().map(plain_word).collect::<Vec<_>>())
            })
            .filter(|(_, parts)| !parts.is_empty())
            .collect();
        let mut next = 0;
        let mut index = 0;
        while index < self.words.len() && next < labels.len() {
            let (which, parts) = &labels[next];
            let said = parts.iter().enumerate().all(|(offset, part)| {
                let Some(word) = self.plain(index + offset) else { return false };
                let possessive = word.strip_suffix("'s").unwrap_or(word);
                word == part || (offset + 1 == parts.len() && possessive == part)
            });
            if said {
                next += 1;
                let end = index + parts.len();
                // What followed its last word, like "'s" or a comma.
                let last = &self.words[end - 1].text;
                let after: String =
                    last.chars().skip(parts[parts.len() - 1].chars().count()).collect();
                self.words[index].text = format!("{}{after}", self.context.mentions[*which].said);
                self.words[index].role = Role::Mention(*which);
                self.words[index].end = self.words[end - 1].end;
                self.words.drain(index + 1..end);
                found.push(*which);
            }
            index += 1;
        }
        found
    }

    /// Whether what comes before a place is a time a zone could follow,
    /// as "3pm" in "3pm Tokyo": a time with its am or pm or minutes, a
    /// range of them, or noon or midnight. A bare hour only counts after
    /// "at" or "from", since "team of 5 london" isn't a time in London.
    fn time_before(&self, index: usize) -> bool {
        let Some(word) = self.before(index, 1) else { return false };
        let single = time_figures(word)
            .is_some_and(|(minutes, suffix)| minutes && suffix.is_empty() || !suffix.is_empty());
        let range = dashed(word).is_some_and(|(from, to)| {
            let side = |word| {
                time_figures(word).is_some_and(|(_, suffix)| matches!(suffix, "" | "am" | "pm"))
            };
            side(from) && side(to)
        });
        if single || range || matches!(word, "noon" | "midnight" | "o'clock" | "oclock") {
            return true;
        }
        let digit = |word: Option<&str>| {
            word.is_some_and(|word| word.starts_with(|c: char| c.is_ascii_digit()))
        };
        if matches!(word, "am" | "pm") {
            return digit(self.before(index, 2));
        }
        figures(word).is_some() && self.before(index, 2).is_some_and(leads)
    }

    /// The time zone the sentence names for its times, which it takes.
    fn zone(&mut self, zones: &ZoneNames) -> Option<String> {
        let free = |sentence: &Sentence, from: usize, to: usize| {
            to <= sentence.words.len() && (from..to).all(|index| sentence.free(index))
        };
        for index in 0..self.words.len() {
            // Before "time", by any name: "9am New York time".
            if self.plain(index) == Some("time") {
                for size in (1..=index.min(3)).rev() {
                    let from = index - size;
                    if !free(self, from, index + 1) {
                        continue;
                    }
                    if let Some(zone) =
                        ZoneNames::find(&zones.before_word_time, &self.said(from, index))
                    {
                        let zone = zone.to_string();
                        self.take(from, index + 1, Role::TimeZone);
                        return Some(zone);
                    }
                }
            }
            // Right after a time: "10:00 PT", "at 3pm Tokyo", with "time"
            // after it taken too.
            if self.time_before(index) {
                for size in (1..=3).rev() {
                    if !free(self, index, index + size) {
                        continue;
                    }
                    if let Some(zone) =
                        ZoneNames::find(&zones.after_a_time, &self.said(index, index + size))
                    {
                        let zone = zone.to_string();
                        let time = self.plain(index + size) == Some("time");
                        self.take(index, index + size + usize::from(time), Role::TimeZone);
                        return Some(zone);
                    }
                }
            }
        }
        None
    }

    /// "all day", "all-day", "the whole day": over the whole day, whatever
    /// else it is, even a meal.
    fn all_day(&mut self) -> bool {
        for index in 0..self.words.len() {
            if !self.free(index) {
                continue;
            }
            let word = self.plain(index).unwrap_or_default();
            if matches!(word, "all-day" | "allday") {
                self.take(index, index + 1, Role::Length);
                return true;
            }
            if matches!(word, "all" | "whole") && self.plain(index + 1) == Some("day") {
                let the = word == "whole" && self.before(index, 1) == Some("the");
                self.take(index - usize::from(the), index + 2, Role::Length);
                return true;
            }
        }
        false
    }

    /// The part of the day it's in, if it's said: with a day or a time,
    /// like "tomorrow morning", "this evening", "tonight" or "11 at
    /// night", which say when and leave the title, or as what the event
    /// is, like "Morning run", which stays in it. "Tonight" and "this
    /// evening" are today. One that says when wins over one in the title.
    fn part_of_day(&mut self) -> (Option<Part>, bool) {
        let (mut said, mut named, mut today) = (None, None, false);
        for index in 0..self.words.len() {
            let word = self.words[index].plain.clone();
            let Some(found) = part(&word).filter(|_| self.free(index)) else { continue };
            let free =
                |sentence: &Sentence, back: usize| index >= back && sentence.free(index - back);
            if word == "tonight" {
                self.take(index, index + 1, Role::Time);
                today = true;
            } else if self.before(index, 1) == Some("this") && free(self, 1) && word != "night" {
                self.take(index - 1, index + 1, Role::Time);
                today = true;
            } else if self.before(index, 1) == Some("the")
                && self.before(index, 2) == Some("in")
                && free(self, 2)
            {
                self.take(index - 2, index + 1, Role::Time);
            } else if self.before(index, 1) == Some("at") && word == "night" && free(self, 1) {
                self.take(index - 1, index + 1, Role::Time);
            } else if free(self, 1)
                && self.before(index, 1).is_some_and(|before| DAY_WORDS.contains(&before))
            {
                self.take(index, index + 1, Role::Time);
            }
            if self.free(index) {
                named = named.or(Some(found));
            } else {
                said = said.or(Some(found));
            }
        }
        (said.or(named), today)
    }

    /// Whether a number at a place is beside a day or what the event is,
    /// or a part of the day read already, and so is a time.
    fn beside(&self, at: usize, size: usize) -> bool {
        let before = self.before(at, 1);
        let after = self.plain(at + size);
        let before_role = at.checked_sub(1).and_then(|index| self.role(index));
        // After a date, as in "fri 6 nov 9-5" or "nov 6 9-5".
        let two = self.before(at, 2).unwrap_or_default();
        let dated = before.is_some_and(|before| {
            (is_month(before) && is_date(two)) || (is_date(before) && is_month(two))
        });
        if before_role == Some(Role::Title) && dated && !after.is_some_and(is_month) {
            return true;
        }
        // Not a date, like "fri 6 nov", or how long or how many, like
        // "friday 2 hour workshop" or "sunday 5 a side".
        if after.is_some_and(|after| is_month(after) || is_unit(after) || after == "a") {
            return false;
        }
        if before_role == Some(Role::Title)
            && before.is_some_and(|before| {
                kind(before).is_some() || DAY_WORDS.contains(&before) || is_written_day(before)
            })
        {
            return true;
        }
        // After days read already, as in "6-8 nov 9-5".
        if before_role.is_some_and(Role::is_when) && !before.is_some_and(leads) {
            return true;
        }
        if self.role(at + size).is_some_and(Role::is_when)
            && after.is_some_and(|after| matches!(after, "tonight" | "in" | "at" | "this"))
        {
            return true;
        }
        self.free(at + size)
            && after.is_some_and(|after| UNTIL.contains(&after))
            && clock_at(&self.plains(), at + size + 1, false).is_some()
    }

    /// The time it starts, and ends if that's said too: "at 1", "1pm",
    /// "2-4pm", "from 1 to 3", "1pm until 2:30", or only when it ends,
    /// "until 4".
    fn times(&mut self, within: Option<Hours>) -> Option<Times> {
        // "till late" says nothing.
        for index in 0..self.words.len() {
            let until = self.plain(index).is_some_and(|word| UNTIL.contains(&word));
            if self.free(index) && until && self.plain(index + 1) == Some("late") {
                self.take(index, index + 2, Role::Time);
            }
        }
        #[derive(Clone, Copy, PartialEq)]
        enum Pass {
            Led,
            Beside,
            Until,
        }
        for pass in [Pass::Led, Pass::Beside, Pass::Until] {
            for index in 0..self.words.len() {
                if !self.free(index) {
                    continue;
                }
                let plains = self.plains();
                let word = plains[index];
                // "until 4": only when it ends.
                if pass == Pass::Until {
                    if !UNTIL.contains(&word) || !self.free(index + 1) {
                        continue;
                    }
                    let Some((end, size)) = clock_at(&plains, index + 1, true) else { continue };
                    self.take(index, index + 1 + size, Role::Time);
                    return Some((None, Some(EndTime::After(end)), false));
                }
                let lead = pass == Pass::Led && leads(word);
                let mut at = if lead { index + 1 } else { index };
                // "at about 9".
                while lead
                    && self.free(at)
                    && plains.get(at).is_some_and(|word| ABOUT.contains(word))
                {
                    at += 1;
                }
                if !self.free(at) {
                    continue;
                }

                // "2-4pm", "13:00–14:00" in a word.
                if let Some((from, to)) = dashed(plains[at])
                    && let (Some((from, _)), Some((to, _))) =
                        (clock_at(&[from], 0, false), clock_at(&[to], 0, false))
                {
                    let counts = if pass == Pass::Led {
                        from.alone || to.alone || lead
                    } else {
                        self.beside(at, 1)
                    };
                    if counts {
                        self.take(index, at + 1, Role::Time);
                        let start = range_start(from, to, within);
                        let end = clock_time(to, Some(start), None);
                        return Some((Some(start), Some(EndTime::At(end)), false));
                    }
                }

                let Some((first, size)) = clock_at(&plains, at, lead) else { continue };
                // In the first pass, "2 until 3:30" and "1 to 2pm" too,
                // before what ends them is read alone.
                let joiner = plains.get(at + size).copied().unwrap_or_default();
                let then = clock_at(&plains, at + size + 1, true);
                let ranged = (UNTIL.contains(&joiner) && then.is_some())
                    || (matches!(joiner, "to" | "-" | "–")
                        && then.is_some_and(|(clock, _)| clock.alone));
                // Not the day of a month said first, as in "nov 5 until 4".
                let dated = !lead
                    && !first.alone
                    && self.before(at, 1).is_some_and(is_month)
                    && !self.before(at, 2).is_some_and(is_date);
                let counts = if pass == Pass::Led {
                    first.alone || lead || ranged
                } else {
                    self.beside(at, size)
                };
                if dated || !counts {
                    continue;
                }
                let mut end = at + size;
                let mut start = clock_time(first, None, within);
                let mut finish = None;
                if self.free(end)
                    && matches!(joiner, "to" | "till" | "til" | "until" | "-" | "–" | "and")
                    && let Some((second, size)) = clock_at(&plains, end + 1, true)
                    && (joiner != "and" || word == "between")
                {
                    start = range_start(first, second, within);
                    finish = Some(EndTime::At(clock_time(second, Some(start), None)));
                    end += 1 + size;
                }
                self.take(if lead { index } else { at }, end, Role::Time);
                return Some((Some(start), finish, first.midnight && finish.is_none()));
            }
        }
        None
    }

    /// How long it lasts in hours and minutes: "for 2 hours", "for an hour
    /// and a half", "for 45 mins", "for 1h30", or before what it is, like
    /// "30 min call". With `days`, in days: "for 3 days", "for a week",
    /// "for 3 nights", to the morning after the last.
    fn lasts(&mut self, days: bool) -> Option<Length> {
        let amount = |word: Option<&str>| match word? {
            "an" | "a" | "one" => Some(1.0),
            "half" => Some(0.5),
            word => number(word).or_else(|| hour_word(word).map(f64::from)),
        };
        for index in 0..self.words.len() {
            if !self.free(index) || self.plain(index) != Some("for") {
                continue;
            }
            let mut at = index + 1;
            // "1h30", "90m", "2h" in a word.
            let short = self.plain(at).and_then(short_length);
            if let Some(length) =
                short.filter(|length| days || matches!(length, Length::Minutes(_)))
            {
                self.take(index, at + 1, Role::Length);
                return Some(length);
            }
            // "half an hour".
            let mut n = amount(self.plain(at));
            if self.plain(at) == Some("half") && matches!(self.plain(at + 1), Some("an" | "a")) {
                at += 1;
            }
            // "a couple of hours".
            if self.plain(at) == Some("a") && matches!(self.plain(at + 1), Some("couple" | "pair"))
            {
                n = Some(2.0);
                at += if self.plain(at + 2) == Some("of") { 2 } else { 1 };
            }
            let Some(n) = n else { continue };
            let unit = self.plain(at + 1).unwrap_or_default();
            let mut minutes = if is_hours(unit) {
                n * 60.0
            } else if is_minutes(unit) || unit == "m" {
                n
            } else if days {
                let each = match unit {
                    "day" | "days" | "night" | "nights" => 1.0,
                    "week" | "weeks" => 7.0,
                    "fortnight" => 14.0,
                    _ => continue,
                };
                let nights = i64::from(unit.starts_with("night"));
                self.take(index, at + 2, Role::Length);
                return Some(Length::Days(((n * each).round() as i64).max(1) + nights));
            } else {
                continue;
            };
            let mut end = at + 2;
            // "and a half".
            if self.plain(end) == Some("and")
                && self.plain(end + 1) == Some("a")
                && self.plain(end + 2) == Some("half")
            {
                if is_hours(unit) {
                    minutes += 30.0;
                }
                end += 3;
            }
            if minutes <= 0.0 {
                continue;
            }
            self.take(index, end, Role::Length);
            return Some(Length::Minutes(minutes.round() as i64));
        }
        if days {
            return None;
        }
        // "30 min call", "2-hour workshop": up to half a day, as a "24
        // hour race" is what it is, not how long it's in the calendar for.
        for index in 0..self.words.len() {
            if !self.free(index) {
                continue;
            }
            let word = self.plain(index).unwrap_or_default();
            let digits = word.len()
                - word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').len();
            let joined = &word[digits..];
            let joined = joined.strip_prefix('-').unwrap_or(joined);
            let one = digits > 0 && (is_minutes(joined) || is_hours(joined));
            let (n, unit, size) = if one {
                (number(&word[..digits]), joined, 1)
            } else {
                (number(word), self.plain(index + 1).unwrap_or_default(), 2)
            };
            let Some(n) = n.filter(|_| is_minutes(unit) || is_hours(unit)) else { continue };
            if (!one && !self.free(index + 1)) || !self.free(index + size) {
                continue;
            }
            let minutes = if is_minutes(unit) { n } else { n * 60.0 };
            if minutes <= 0.0 || minutes > 12.0 * 60.0 {
                continue;
            }
            self.take(index, index + size, Role::Length);
            return Some(Length::Minutes(minutes.round() as i64));
        }
        None
    }

    /// How it repeats, as the longest run of words around one like "every"
    /// or "weekly" that the repeat reader reads, given the day it starts
    /// from. The time read already can sit in the middle of it, as in
    /// "every sunday at 5pm until christmas", and is left out of what's
    /// read.
    fn repeats(&mut self) -> Option<String> {
        const ORDINALS: [&str; 11] = [
            "first", "second", "third", "fourth", "fifth", "last", "1st", "2nd", "3rd", "4th",
            "5th",
        ];
        // How many words before one like "every" there can be, besides the time.
        const LEADING: usize = 8;
        let timed =
            |sentence: &Sentence, index: usize| sentence.role(index).is_some_and(Role::is_when);
        let ordinal = |sentence: &Sentence, index: usize| {
            sentence.plain(index).is_some_and(|word| ORDINALS.contains(&word))
        };
        for index in 0..self.words.len() {
            if !self.free(index) || !says_repeat(&self.plains()[index..]) {
                continue;
            }
            // Past the time, as in "last friday at 9 every month".
            let mut starts = Vec::new();
            let mut seen = 0;
            for at in (0..index).rev() {
                if seen >= LEADING {
                    break;
                }
                if timed(self, at) {
                    continue;
                }
                if !self.free(at) {
                    break;
                }
                seen += 1;
                let article = matches!(self.plain(at), Some("the" | "on")) && ordinal(self, at + 1);
                if ordinal(self, at) || article {
                    starts.insert(0, at);
                }
            }
            starts.push(index);
            for start in starts {
                // Free words, or the time between them, up to the first
                // that's neither.
                let mut end = start;
                while end < self.words.len() && (self.free(end) || timed(self, end)) {
                    end += 1;
                }
                for size in (index - start + 1..=end - start).rev() {
                    // Not one that ends on a word like "for" or "the",
                    // which leads to something else, like how long each is,
                    // or on punctuation.
                    let last = start + size - 1;
                    let leads_on = self.plain(last).is_some_and(|word| {
                        JOINERS.contains(&word) || FILLERS.contains(&word) || word.is_empty()
                    });
                    if !self.free(last) || leads_on {
                        continue;
                    }
                    let said: Vec<&str> = (start..start + size)
                        .filter(|at| self.free(*at))
                        .filter_map(|at| self.plain(at))
                        .collect();
                    let said = said.join(" ");
                    if self.repeat(&said, self.context.today).is_some() {
                        for at in start..start + size {
                            if self.free(at) {
                                self.take(at, at + 1, Role::Repeat);
                            }
                        }
                        return Some(said);
                    }
                }
            }
        }
        None
    }

    /// How some words say it repeats, from a day: on a schedule, and not
    /// by working days, which calendar apps can't read.
    fn repeat(&self, said: &str, from: Day) -> Option<(Repeat, String)> {
        self.reader
            .repeats(said, from, self.context.today)
            .into_iter()
            .find(|reading| {
                reading.repeat.kind == RepeatKind::Schedule && !reading.repeat.rule.skips_days_off
            })
            .map(|reading| (reading.repeat, reading.how))
    }

    /// Where a day could end, for each word one could start at: the word
    /// after each run of words the day grammar parses from there. The
    /// words that are still the title's are read once, a stretch at a
    /// time, and every run comes out of that one parse, rather than each
    /// run of words being read by itself to see whether it's a day.
    fn day_ends(&self) -> Vec<Vec<usize>> {
        let mut ends = vec![Vec::new(); self.words.len()];
        let mut start = 0;
        while start < self.words.len() {
            if !self.free(start) {
                start += 1;
                continue;
            }
            let stop =
                (start..self.words.len()).find(|at| !self.free(*at)).unwrap_or(self.words.len());
            let plains: Vec<&str> =
                self.words[start..stop].iter().map(|word| word.plain.as_str()).collect();
            let (tokens, pieces): (Vec<Token>, Vec<usize>) = tokens_of(&plains).into_iter().unzip();
            let parser = self.reader.parser(&tokens);
            for (word, ends) in ends.iter_mut().enumerate().take(stop).skip(start) {
                // The first word from here that isn't one always left out.
                let from = pieces.partition_point(|piece| start + piece < word);
                let mut found: Vec<usize> = parser
                    .day(from, 0)
                    .into_iter()
                    // Not part of a word, as "3" is of "3d".
                    .filter(|(_, end)| *end == tokens.len() || pieces[*end] != pieces[*end - 1])
                    .map(|(_, end)| pieces.get(end).map_or(stop, |piece| start + piece))
                    .filter(|end| *end > word)
                    .collect();
                found.sort_unstable();
                found.dedup();
                *ends = found;
            }
            start = stop;
        }
        ends
    }

    /// Which words couldn't be part of either side of a range of days:
    /// those that are in no run the day grammar parses, and aren't words a
    /// time, a part of the day or a time zone is said with. A range is
    /// looked for only among the words between them.
    fn never_when(&self) -> Vec<bool> {
        let ends = self.day_ends();
        let mut never: Vec<bool> = self
            .words
            .iter()
            .map(|word| {
                !could_say_when(&word.plain) && !self.reader.zones.words.contains(&word.plain)
            })
            .collect();
        for (start, ends) in ends.iter().enumerate() {
            if let Some(last) = ends.last() {
                never[start..*last].fill(false);
            }
        }
        never
    }

    /// The day, as the longest run of words the day reader reads as one,
    /// the earliest if two are as long, leaving out a run that's only a
    /// word that isn't a day on its own, unless it's the last word typed,
    /// and anything straight after "at", which is a place, like "at the
    /// Sun". A week or a weekend, like "next week" or "this weekend", is
    /// all of it, though a weekend not said to be which is the event's
    /// name, like "Weekend in Dorset". A day of the week that starts the
    /// title and names the event, like "Friday drinks", stays in it.
    fn days(&mut self) -> Option<(Day, Option<Day>, Vec<Day>)> {
        let today = self.context.today;
        let ends = self.day_ends();
        let plains: Vec<String> = self.words.iter().map(|word| word.plain.clone()).collect();
        let plains: Vec<&str> = plains.iter().map(String::as_str).collect();
        let mut best: Option<(usize, usize, Vec<DayReading>, String)> = None;
        for index in 0..self.words.len() {
            if !self.free(index) {
                continue;
            }
            let at_place = self.before(index, 1) == Some("at")
                || (self.before(index, 1) == Some("the") && self.before(index, 2) == Some("at"));
            if at_place {
                continue;
            }
            // Only as far as a day could end from here, by the one parse of
            // the words, and to the end of the sentence, whose last word
            // may be half typed.
            let to_end = (index..self.words.len()).all(|at| self.free(at));
            let stops = ends[index].iter().copied().chain(to_end.then_some(self.words.len()));
            for size in stops.map(|end| end - index) {
                if best.as_ref().is_some_and(|(_, most, ..)| size <= *most) {
                    continue;
                }
                let run = &plains[index..index + size];
                let saying: Vec<&str> = run
                    .iter()
                    .copied()
                    .filter(|word| !matches!(*word, "on" | "the" | "by"))
                    .collect();
                match saying[..] {
                    [] => continue,
                    [word] => {
                        let last = index + size == self.words.len();
                        // With no title to be part of, as when it's only
                        // when, any word that's a day is one.
                        let alone = DAY_WORDS.contains(&word)
                            || is_written_day(word)
                            || self.reader.names_a_day(word)
                            || !self.event;
                        let typing = last
                            && word.len() > 1
                            && DAY_WORDS.iter().any(|day| day.starts_with(word));
                        if !alone && !typing {
                            continue;
                        }
                        // "weekend" with nothing to say which names the
                        // event when it's beside other words of the title,
                        // as in "Weekend in Dorset" or "Camping weekend".
                        let named = |at: Option<usize>| {
                            at.is_some_and(|at| {
                                self.free(at)
                                    && !self.plain(at).is_some_and(|word| {
                                        JOINERS.contains(&word) || FILLERS.contains(&word)
                                    })
                            })
                        };
                        let beside = named(index.checked_sub(1)) || named(Some(index + 1));
                        if word == "weekend" && size == 1 && beside {
                            continue;
                        }
                        // "tonight" is read as part of the day.
                        if word == "tonight" {
                            continue;
                        }
                    }
                    _ => {}
                }
                let said = run.join(" ");
                let readings = self.reader.days(&said, today);
                if !readings.is_empty() {
                    best = Some((index, size, readings, said));
                }
            }
        }
        let (from, size, readings, said) = best?;
        let named = from == 0
            && size == 1
            && WEEKDAY_WORDS.iter().any(|(name, _)| Some(*name) == self.plain(0))
            && self.free(1);
        if !named {
            self.take(from, from + size, Role::Day);
        }
        let day = readings[0].day;
        // Only when, a month, a quarter, a year or a named day that
        // lasts is all of it too, as "next month" is somewhere to look.
        let whole = self.reader.span(&said, today).filter(|span| {
            let kinds = matches!(span.kind, SpanKind::Week | SpanKind::Weekend)
                || (!self.event
                    && matches!(
                        span.kind,
                        SpanKind::Month | SpanKind::Quarter | SpanKind::Year | SpanKind::Named
                    ));
            kinds && span.first == day
        });
        Some((
            day,
            whole.map(|span| span.last),
            readings[1..].iter().map(|reading| reading.day).collect(),
        ))
    }

    /// A span said whole: "all week", "all of november", "the whole of
    /// next month", "throughout november", from today through the rest of
    /// one, "the rest of the week", or so many days or weeks from today,
    /// "the next 3 days".
    fn spans(&mut self) -> Option<(Day, Day)> {
        let today = self.context.today;
        // The span the words from a place say, as many as do, and where
        // they end.
        let span_at = |sentence: &Sentence, at: usize| {
            (1..=5usize.min(sentence.words.len().saturating_sub(at))).rev().find_map(|size| {
                if !(at..at + size).all(|index| sentence.free(index)) {
                    return None;
                }
                let mut said = sentence.said(at, at + size);
                // "all week" is this one.
                let bare = said.strip_prefix("the ").unwrap_or(&said);
                if matches!(bare, "week" | "month" | "year") {
                    said = format!("this {bare}");
                }
                let found = sentence.reader.span(&said, today)?;
                Some((found.first, found.last, at + size))
            })
        };
        for index in 0..self.words.len() {
            if !self.free(index) {
                continue;
            }
            let word = self.plain(index).unwrap_or_default();
            let next = self.plain(index + 1);
            let the = index > 0 && self.before(index, 1) == Some("the") && self.free(index - 1);
            let from = index - usize::from(the);
            if matches!(word, "all" | "whole" | "throughout") {
                let Some((first, last, end)) =
                    span_at(self, index + 1 + usize::from(next == Some("of")))
                else {
                    continue;
                };
                self.take(if word == "whole" { from } else { index }, end, Role::Day);
                return Some((first, last));
            }
            if word == "rest" && next == Some("of") {
                let Some((_, last, end)) =
                    span_at(self, index + 2).filter(|(_, last, _)| *last >= today)
                else {
                    continue;
                };
                self.take(from, end, Role::Day);
                return Some((today, last));
            }
            if word == "next" {
                let count = next.and_then(|next| match next {
                    "couple" => Some(2),
                    "few" => Some(3),
                    _ => hour_word(next).map(i64::from).or_else(|| figures(next).map(i64::from)),
                });
                let Some(n) =
                    count.filter(|n| *n >= 2 && self.free(index + 1) && self.free(index + 2))
                else {
                    continue;
                };
                let last = match self.plain(index + 2).unwrap_or_default() {
                    "day" | "days" => today.add_days(n - 1),
                    "week" | "weeks" => today.add_days(7 * n - 1),
                    "month" | "months" => today.add_months(n).and_then(|day| day.add_days(-1)),
                    _ => None,
                };
                let Some(last) = last else { continue };
                self.take(from, index + 3, Role::Day);
                return Some((today, last));
            }
        }
        None
    }

    /// From one day to another, read from the words either side of one
    /// like "to", "-" or "until", or a dash in a word: "6-8 nov", "nov
    /// 6-8", "fri to sun", "28 dec to 3 jan", "fri 6pm to sun 5pm". Each
    /// side is any day the day reader reads. One side can borrow the month
    /// or the week the other says, as in "6-8 nov" or "mon-fri next week".
    /// "Until" with nothing before it is from the day given, and until a
    /// span, like "until next year", is up to the day before it starts.
    /// The longest reading wins.
    fn ranges(&mut self) -> Option<Range> {
        let mut best: Option<(usize, usize, Range)> = None;
        let plains: Vec<String> = self.words.iter().map(|word| word.plain.clone()).collect();
        let has_joiner = plains.iter().any(|word| joins(word) || dashed(word).is_some());
        let never = if has_joiner { self.never_when() } else { Vec::new() };
        for at in 0..plains.len() {
            if !self.free(at) {
                continue;
            }
            let word = plains[at].as_str();
            let dash = dashed(word);
            if !joins(word) && dash.is_none() {
                continue;
            }
            // "through to friday".
            let through = matches!(word, "through" | "thru")
                && plains.get(at + 1).is_some_and(|next| next == "to");
            let after = at + 1 + usize::from(through);
            let until = dash.is_none() && UNTIL.contains(&word);
            for from in (at.saturating_sub(8)..=at).rev() {
                if from < at && (!self.free(from) || never[from]) {
                    break;
                }
                // Not from the month of a date said whole, as in "5 nov 1-3".
                if from < at && is_month(&plains[from]) && self.before(from, 1).is_some_and(is_date)
                {
                    continue;
                }
                let first = if dash.is_some() { at + 1 } else { after + 1 };
                for to in first..=plains.len().min(after + 12) {
                    if to > after && (!self.free(to - 1) || never[to - 1]) {
                        break;
                    }
                    if best.as_ref().is_some_and(|(start, end, _)| to - from <= end - start) {
                        continue;
                    }
                    let mut left: Vec<&str> = plains[from..at].iter().map(String::as_str).collect();
                    let mut right: Vec<&str> = Vec::new();
                    if let Some((before, behind)) = dash {
                        left.push(before);
                        right.push(behind);
                    }
                    if to > after {
                        right.extend(plains[after..to].iter().map(String::as_str));
                    }
                    if right.is_empty() || (left.is_empty() && !until) {
                        continue;
                    }
                    if let Some(read) = self.between(&left, &right, until) {
                        best = Some((from, to, read));
                    }
                }
            }
        }
        let (from, to, range) = best?;
        self.take(from, to, Role::Day);
        // Its times, "fri 6pm to sun 5pm", "at 6" or "2-4pm", are times,
        // and so is "9-5" in "fri 6 nov 9-5", from one time to another.
        for at in from..to {
            let word = self.words[at].plain.as_str();
            let times = dashed(word).is_some_and(|(from, to)| {
                let side = |word, sure| side_time(&[word], sure).is_some();
                side(from, true)
                    || side(to, true)
                    || (range.times.is_some() && side(from, false) && side(to, false))
            });
            // A bare number, when the range has times, unless it's the
            // day of a month, as "6" is in "fri 6 nov 9 to 5".
            let before = |back| at.checked_sub(back).and_then(|at| self.plain(at));
            let dated = self.plain(at + 1).is_some_and(is_month)
                || (before(1).is_some_and(is_month) && !before(2).is_some_and(is_date));
            // And only before what ends a time, or another, or nothing, so
            // not "3" in "in 3 weeks".
            let next = self.plain(at + 1).filter(|_| at + 1 < to);
            let ends = next.is_none_or(|next| {
                joins(next) || matches!(next, "and" | "am" | "pm" | "o'clock" | "oclock")
            });
            let bare = range.times.is_some() && figures(word).is_some() && !dated && ends;
            if times || bare || side_time(&[word], true).is_some() {
                self.words[at].role = Role::Time;
            } else if matches!(word, "at" | "@")
                && at + 1 < to
                && side_time(&[self.words[at + 1].plain.as_str()], false).is_some()
            {
                self.words[at].role = Role::Time;
                self.words[at + 1].role = Role::Time;
            }
        }
        // What's between two times, as "to" in "9am to 5pm", is a time too.
        for at in from + 1..to.saturating_sub(1) {
            let between = self.role(at - 1) == Some(Role::Time)
                && self.role(at + 1) == Some(Role::Time)
                && (joins(&self.words[at].plain) || self.words[at].plain == "and");
            if between {
                self.words[at].role = Role::Time;
            }
        }
        Some(range)
    }

    /// When each side of a range says, the second from the first.
    fn between(&self, left: &[&str], right: &[&str], until: bool) -> Option<Range> {
        let today = self.context.today;
        let lone = |words: &[&str]| matches!(words, [word] if is_date(word));
        let read = |words: &[&str], from: Day| self.end(&words.join(" "), from);
        // What one side borrows from the other: its month or week, but not
        // its time, so "6-8 nov at 9" is at 9 from the first to the last.
        // Anything from "at", or a number that isn't a year, is when in
        // the day, and anything from "on" is the day itself.
        let borrow = |words: &[&str]| -> Vec<String> {
            let timed = words.iter().position(|word| {
                matches!(*word, "at" | "@" | "on" | "noon" | "midday" | "midnight")
                    || (word.starts_with(|c: char| c.is_ascii_digit()) && !all_figures(word, 4, 4))
            });
            words[..timed.unwrap_or(words.len())].iter().map(|word| word.to_string()).collect()
        };
        let joined = |one: &[&str], other: &[String]| -> Vec<String> {
            one.iter().map(|word| word.to_string()).chain(other.iter().cloned()).collect()
        };
        let read_owned = |words: &[String], from: Day| self.end(&words.join(" "), from);

        // A lone date borrows the other side's month first: "6-8 nov".
        let start = if left.is_empty() {
            End { day: self.context.fallback_day, time: None, span: None }
        } else if left == ["now"] {
            End { day: today, time: None, span: None }
        } else {
            let tail = borrow(&right[1..]);
            let borrowed =
                if tail.is_empty() { None } else { read_owned(&joined(left, &tail), today) };
            if lone(left) {
                borrowed.or_else(|| read(left, today))
            } else {
                read(left, today).or(borrowed)
            }?
        };
        // The second from today, or from the first if that's before it, so
        // "fri to sun" is the Sunday after.
        let second = |words: &[String]| {
            let found = read_owned(words, today)?;
            if found.day < start.day { read_owned(words, start.day) } else { Some(found) }
        };
        let own = |words: &[&str]| words.iter().map(|word| word.to_string()).collect::<Vec<_>>();
        // The month said before the day, as in "nov 6 to 8", or after it,
        // as in "6 nov to 8".
        let after_left = || second(&joined(&left[..left.len() - 1], &own(right)));
        let borrowed = || {
            if left.len() < 2 {
                None
            } else if lone(right) && left[0].starts_with(|c: char| c.is_ascii_digit()) {
                second(&joined(right, &own(&left[1..]))).or_else(after_left)
            } else {
                after_left()
            }
        };
        let end = if lone(right) {
            borrowed().or_else(|| second(&own(right)))
        } else {
            second(&own(right)).or_else(borrowed)
        }?;
        // Days of one month run forwards: "8-6 nov" isn't most of a year.
        if (lone(left) || lone(right)) && start.day.days_until(end.day) > 31 {
            return None;
        }
        if start.time.is_some() != end.time.is_some() {
            return None;
        }
        let start_day = start.span.map_or(start.day, |(first, _)| first);
        let end_day = match end.span {
            Some((first, _)) if until => first.add_days(-1)?,
            Some((_, last)) => last,
            None => end.day,
        };
        match (start.time, end.time) {
            (Some(from), Some(to)) => ((end_day, to) > (start_day, from)).then_some(Range {
                start_day,
                end_day,
                times: Some((from, to)),
            }),
            _ => (end_day >= start_day).then_some(Range { start_day, end_day, times: None }),
        }
    }

    /// One side of a range: a day, and maybe a time, and nothing else. Not
    /// a bare number, which is a time or a date only beside something else.
    fn end(&self, said: &str, from: Day) -> Option<End> {
        if said.is_empty() || said.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        if let Some(reading) = self.reader.days(said, from).first() {
            // A span, like "next year", that the day is the start of, and
            // not a word half typed, like "mon" for "month".
            let span = self.reader.span(said, from).filter(|span| span.first == reading.day);
            return Some(End {
                day: reading.day,
                time: None,
                span: span.map(|span| (span.first, span.last)),
            });
        }
        // A day and then a time, "fri 6pm", "fri at 6", "9 dec 11", or a
        // time and then a day, "6pm fri". The time's
        // looked for first, as it's quick to say it isn't one.
        let words: Vec<&str> = said.split(' ').collect();
        let read = |day: &[&str], time: &[&str], first: bool| {
            // A bare number is a time only after a day that's plainly one,
            // as "friday 3" and "9 dec 11" are, and "next week 2" isn't.
            let plainly = match day {
                [.., before, last] if is_month(last) && is_date(before) => true,
                [.., before, last] if is_date(last) && is_month(before) => true,
                [.., last] => DAY_WORDS.contains(last),
                [] => false,
            };
            let time = side_time(time, first || !plainly)?;
            let day = self.reader.days(&day.join(" "), from).first()?.day;
            Some(End { day, time: Some(time), span: None })
        };
        let splits = || 1..words.len();
        splits()
            .find_map(|split| read(&words[..split], &words[split..], false))
            .or_else(|| splits().find_map(|split| read(&words[split..], &words[..split], true)))
    }

    /// Where it is: what follows "at" or "@", up to the next of what's
    /// been read, or "with", as in "at Dishoom with #Priya". It stops at
    /// "and" only before what's been read, since "and" is in names like
    /// "Smith and Wollensky".
    fn place(&mut self) -> Option<String> {
        for index in 0..self.words.len() {
            if !self.free(index) || !matches!(self.plain(index), Some("at" | "@")) {
                continue;
            }
            let mut end = index + 1;
            while self.free(end)
                && self.plain(end) != Some("with")
                && !(self.plain(end) == Some("and") && !self.free(end + 1))
            {
                end += 1;
            }
            if end == index + 1 {
                continue;
            }
            let words: Vec<&str> =
                self.words[index + 1..end].iter().map(|word| word.text.as_str()).collect();
            let location = words.join(" ").trim_end_matches([',', ';', ':']).to_string();
            self.take(index, end, Role::Place);
            return Some(location);
        }
        None
    }
}

/// The time one side of a range says, when its words are a time and
/// nothing else: "6pm", "at 6", "half three". With `sure`, only one that
/// can't be anything but a time, or that "at" says is one.
fn side_time(words: &[&str], sure: bool) -> Option<Time> {
    let (led, words) = match words {
        ["at" | "@", rest @ ..] => (true, rest),
        all => (false, all),
    };
    let (clock, size) = clock_at(words, 0, true)?;
    (size == words.len() && (!sure || led || clock.alone)).then(|| clock_time(clock, None, None))
}

/// How long something lasts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Length {
    Minutes(i64),
    Days(i64),
}

/// How long, in one word: "1h30", "90m", "2h", "3d".
fn short_length(word: &str) -> Option<Length> {
    let digits =
        word.len() - word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').len();
    let n = number(&word[..digits])?;
    let rest = &word[digits..];
    let unit_length = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_lowercase()).len();
    let (unit, more) = rest.split_at(unit_length);
    let more = more.strip_suffix('m').unwrap_or(more);
    if !more.is_empty() && !all_figures(more, 1, 4) {
        return None;
    }
    let extra = number(more).unwrap_or(0.0);
    match unit {
        "d" if more.is_empty() => Some(Length::Days((n.round() as i64).max(1))),
        "h" | "hr" | "hrs" => Some(Length::Minutes((n * 60.0 + extra).round() as i64)),
        "m" | "min" | "mins" if more.is_empty() => Some(Length::Minutes(n.round() as i64)),
        _ => None,
    }
}

/// What a sentence says, read as an event, or only for when it is.
struct Heard {
    title: String,
    mentioned: Vec<usize>,
    start_day: Day,
    end_day: Day,
    start_time: Option<Time>,
    end_time: Option<Time>,
    location: Option<String>,
    repeat: Option<EventRepeat>,
    time_zone: Option<String>,
    read: EventRead,
    others: Vec<Day>,
    words: Vec<ReadWord>,
    /// Whether every word was read as something, leaving nothing for a
    /// title.
    all_read: bool,
}

/// Reads an event's sentence.
pub(crate) fn read(reader: &Reader, text: &str, context: &EventContext) -> EventReading {
    let heard = hear(reader, text, context, true).expect("an event has a time to fall back on");
    EventReading {
        title: heard.title,
        mentioned: heard.mentioned,
        start_day: heard.start_day,
        end_day: heard.end_day,
        start_time: heard.start_time,
        end_time: heard.end_time,
        location: heard.location,
        repeat: heard.repeat,
        time_zone: heard.time_zone,
        read: heard.read,
        others: heard.others,
        words: heard.words,
    }
}

/// Reads when something is, and nothing else, as an event's sentence
/// would say it.
pub(crate) fn when(reader: &Reader, text: &str, today: Day) -> Option<WhenReading> {
    let context = EventContext::new(today, Time::from_minutes(0));
    let heard = hear(reader, text, &context, false)?;
    heard.all_read.then_some(WhenReading {
        start_day: heard.start_day,
        end_day: heard.end_day,
        start_time: heard.start_time,
        end_time: heard.end_time,
        time_zone: heard.time_zone,
        others: heard.others,
        words: heard.words,
    })
}

/// Reads a sentence, as an event's, with everything else it says, or only
/// for when it is: then nothing falls back on what the context says when
/// nothing does, and with a time but nothing to say when it starts it's
/// `None`.
fn hear(reader: &Reader, text: &str, context: &EventContext, event: bool) -> Option<Heard> {
    let today = context.today;
    let words = text
        .split_whitespace()
        .map(|word| {
            // Where the word is, from where it is in memory, as it's a
            // part of the text.
            let start = word.as_ptr() as usize - text.as_ptr() as usize;
            Word {
                text: word.to_string(),
                plain: plain_word(word),
                role: Role::Title,
                start,
                end: start + word.len(),
            }
        })
        .collect();
    let mut sentence = Sentence { reader, context, words, event };
    let mentioned = sentence.mentions();
    // The zone first, so a city in it isn't where it is, or its title.
    let time_zone = sentence.zone(&reader.zones.names);
    let whole_day = sentence.all_day();
    let (part, tonight) = sentence.part_of_day();
    // What the event is, which says the hours a bare hour's in, unless the
    // part of the day says.
    let usual_hours = sentence
        .words
        .iter()
        .filter(|word| word.role == Role::Title)
        .find_map(|word| kind(&word.plain));
    let within = part.map(|part| part.hours).or(usual_hours);
    // A repeat reads its own days, as in "every sunday until christmas".
    // Only an event repeats.
    let plains = sentence.plains();
    let repeating = event && (0..plains.len()).any(|at| says_repeat(&plains[at..]));
    let ranged = if repeating { None } else { sentence.ranges() };
    let time = if ranged.is_some_and(|range| range.times.is_some()) {
        None
    } else {
        sentence.times(within)
    };
    // How it repeats comes before how long it lasts in days, as "weekly
    // for 6 weeks" is how long it goes on repeating, but after how long
    // each is in hours or minutes, as "every day for 2 hours" isn't.
    let hours = sentence.lasts(false);
    let repeat_said = if event { sentence.repeats() } else { None };
    let length = hours.or_else(|| sentence.lasts(true));
    let span = if ranged.is_some() { None } else { sentence.spans() };
    let day = if ranged.is_some() || span.is_some() { None } else { sentence.days() };
    let location = if event { sentence.place() } else { None };

    // What joined the title to what was read, like "on" in "on friday",
    // goes with it.
    for index in 0..sentence.words.len() {
        let joiner = sentence.free(index)
            && sentence.plain(index).is_some_and(|word| JOINERS.contains(&word));
        // Or the "at" a place starts with, as in "Dinner on at Dishoom".
        let place = sentence.role(index + 1) == Some(Role::Place)
            && matches!(sentence.plain(index + 1), Some("at" | "@"));
        let next = sentence.role(index + 1).filter(|role| role.is_when() || place);
        if joiner && let Some(next) = next {
            sentence.words[index].role = next;
        }
    }
    // Punctuation on its own, like "," or "!", after what was read, and
    // before more of it or nothing, goes with it.
    for index in 1..sentence.words.len() {
        let after = sentence.role(index - 1).filter(|role| role.is_when());
        let ends = sentence.role(index + 1).is_none_or(|role| role.is_when());
        if sentence.free(index)
            && sentence.words[index].plain.is_empty()
            && ends
            && let Some(after) = after
        {
            sentence.words[index].role = after;
        }
    }
    let mut title = sentence.title();
    // Starting where something read was, like "30 min call", it starts as
    // a sentence does, unless what was typed didn't.
    let lower = text.trim_start().starts_with(|c: char| c.is_ascii_lowercase());
    if sentence.words.first().is_some_and(|word| word.role != Role::Title) && !lower {
        let mut letters = title.chars();
        if let Some(first) = letters.next() {
            title = first.to_uppercase().chain(letters).collect();
        }
    }

    // The repeat picks the first day it comes round on, from the day
    // typed, or else the one given.
    let given = ranged
        .map(|range| range.start_day)
        .or(span.map(|(first, _)| first))
        .or(day.as_ref().map(|(day, ..)| *day))
        .or(tonight.then_some(today));
    let repeated =
        repeat_said.and_then(|said| sentence.repeat(&said, given.unwrap_or(context.fallback_day)));
    let midnight = time.is_some_and(|(_, _, midnight)| midnight) && repeated.is_none();
    let mut start_day = repeated
        .as_ref()
        .map(|(repeat, _)| repeat.starts)
        .or(given)
        .unwrap_or(context.fallback_day);
    let mut others = day.as_ref().map(|(_, _, others)| others.clone()).unwrap_or_default();
    // "at midnight" is as the day ends.
    if midnight && let Some(next) = start_day.add_days(1) {
        others.push(start_day);
        start_day = next;
    }
    // The last day, when a span or a run of days said it, as it ends too,
    // at midnight.
    let said_last = ranged
        .map(|range| range.end_day)
        .or(span.map(|(_, last)| last))
        .or(day.and_then(|(_, last, _)| last));
    // A repeat that starts on a later day than was said carries the last
    // day on with it, so it lasts as long, and never ends before it starts.
    let moved = match (repeated.as_ref(), given) {
        (Some((repeat, _)), Some(given)) => given.days_until(repeat.starts),
        _ => 0,
    };
    let last_day = said_last
        .and_then(|last| last.add_days(moved + i64::from(midnight)))
        .filter(|last| *last >= start_day);

    // A meal in its title, when it's not said when it is, or how long.
    let meal_named = sentence
        .words
        .iter()
        .filter(|word| word.role == Role::Title)
        .find_map(|word| Meal::named(&word.plain));
    let meal_usual = meal_named.map(Meal::usual);
    let days_long = match length {
        Some(Length::Days(days)) => Some(days),
        _ => None,
    };
    let minutes_long = match length {
        Some(Length::Minutes(minutes)) => Some(minutes),
        _ => None,
    };
    let usual = if time.is_some() || days_long.is_some() || last_day.is_some() || whole_day {
        None
    } else {
        meal_usual.map(|(time, _)| time).or(part.and_then(|part| part.time))
    };
    let range_times = ranged.and_then(|range| range.times);
    // Only when, it has to say when it starts, by a day or a time, and not
    // only how long, like "for 2 days".
    let starts_said = given.is_some()
        || time.is_some_and(|(start, ..)| start.is_some())
        || range_times.is_some()
        || usual.is_some();
    if !event && !starts_said {
        return None;
    }
    let read = EventRead {
        day: given.is_some() || repeated.is_some(),
        time: time.is_some() || range_times.is_some(),
        length: length.is_some() || last_day.is_some(),
        meal: meal_named.filter(|_| usual.is_some()),
    };
    let repeat = repeated
        .clone()
        .map(|(repeat, how)| EventRepeat { repeat: Repeat { starts: start_day, ..repeat }, how });
    let words: Vec<ReadWord> = sentence
        .words
        .iter()
        .map(|word| ReadWord { start: word.start, end: word.end, role: word.role })
        .collect();
    let all_read =
        !sentence.words.is_empty() && sentence.words.iter().all(|word| word.role != Role::Title);
    let shared = |end_day, start_time, end_time| Heard {
        title: title.clone(),
        mentioned: mentioned.clone(),
        start_day,
        end_day,
        start_time,
        end_time,
        location: location.clone(),
        repeat: repeat.clone(),
        time_zone: time_zone.clone(),
        read: read.clone(),
        others: others.clone(),
        words: words.clone(),
        all_read,
    };

    // A day, or a repeat, with no time is over whole days, unless it's
    // said how many hours or minutes it is, or it's a meal or a part of
    // the day.
    let timed =
        time.is_some() || range_times.is_some() || usual.is_some() || minutes_long.is_some();
    if whole_day || (!timed && (given.is_some() || repeated.is_some() || days_long.is_some())) {
        let end = last_day
            .or_else(|| start_day.add_days(days_long.unwrap_or(1) - 1))
            .unwrap_or(start_day);
        return Some(shared(end, None, None));
    }

    // From one day and time to another.
    if let (Some(range), Some((from, to))) = (ranged, range_times) {
        return Some(shared(range.end_day, Some(from), Some(to)));
    }

    // Only an event falls back on a time, and how long it lasts.
    let fallback_time = event.then_some(context.fallback_time);
    let default_length = event.then_some(i64::from(context.default_length));
    let start_time = time.and_then(|(start, ..)| start).or(usual).or(fallback_time)?;
    let end = time.and_then(|(_, end, _)| end).map(|end| end.after(start_time));
    let meal_minutes = meal_usual.map(|(_, minutes)| minutes);
    // Over days, at the times said: "6-8 nov 9-5" is from 9 on the first
    // to 5 on the last. With only a start, it ends on the last day as long
    // after it as one would on its own: "mon-fri at 9" is from 9 on Monday
    // to 10 on Friday.
    if let Some(last_day) = last_day {
        let length = minutes_long.or(meal_minutes).or(default_length);
        let end = end
            .or(length.map(|length| Time::from_minutes(i64::from(start_time.minutes()) + length)));
        return Some(shared(last_day, Some(start_time), end));
    }
    let minutes = match (days_long, minutes_long, end) {
        (Some(days), ..) => Some(days * 24 * 60),
        (_, Some(minutes), _) => Some(minutes),
        // To the next time it's that time, the next day if it's earlier.
        (_, _, Some(end)) => Some(
            (i64::from(end.minutes()) - i64::from(start_time.minutes()) - 1).rem_euclid(24 * 60)
                + 1,
        ),
        _ => meal_minutes.or(default_length),
    };
    // At a time, and not until one.
    let Some(minutes) = minutes else { return Some(shared(start_day, Some(start_time), None)) };
    let ends = i64::from(start_time.minutes()) + minutes;
    let end_day = start_day.add_days(ends.div_euclid(24 * 60)).unwrap_or(start_day);
    Some(shared(end_day, Some(start_time), Some(Time::from_minutes(ends))))
}
