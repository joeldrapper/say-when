//! Reads a time of day as people type and say them: "1pm", "13:30", "9.15",
//! "noon", "9ish", "3 o'clock", "seven thirty", "half past 3", "half three"
//! (half past, as in Britain), "quarter to 5".
//!
//! A time with no am or pm, like "7", could be either. What it's taken for
//! depends on what's around it, which is the caller's to say: a time it
//! must come after, the hours something usually happens in, or else the
//! afternoon up to 7.

use crate::day::Time;

/// Hours of the day, in minutes from midnight, from one up to the other,
/// round midnight when the second is earlier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Hours {
    pub from: u32,
    pub to: u32,
}

impl Hours {
    pub const fn new(from: u32, to: u32) -> Hours {
        Hours { from: from * 60, to: to * 60 }
    }

    fn has(self, time: Time) -> bool {
        let at = time.minutes();
        if self.from <= self.to {
            at >= self.from && at < self.to
        } else {
            at >= self.from || at < self.to
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Half {
    Am,
    Pm,
}

/// A time as it was written, before it's known which half of the day a
/// bare hour is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Clock {
    pub hour: u32,
    pub minute: u32,
    pub half: Option<Half>,
    /// Whether it's that time as written, like 13:00 or 09:30.
    pub exact: bool,
    /// Whether it can only be a time, like 9:30, "half three" or "3
    /// o'clock", so it needs no "at" before it.
    pub alone: bool,
    /// Midnight, which is as the day ends when it's when something starts.
    pub midnight: bool,
}

const HOUR_WORDS: [&str; 12] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
    "twelve",
];

/// An hour said as a word, from "one" to "twelve".
pub(crate) fn hour_word(word: &str) -> Option<u32> {
    HOUR_WORDS.iter().position(|each| *each == word).map(|index| index as u32 + 1)
}

/// A number of one or two figures.
pub(crate) fn figures(word: &str) -> Option<u32> {
    ((1..=2).contains(&word.len()) && word.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

/// An hour from 1 to 12, in figures or as a word.
fn hour_of(word: Option<&str>) -> Option<u32> {
    let word = word?;
    figures(word).filter(|hour| (1..=12).contains(hour)).or_else(|| hour_word(word))
}

/// "am", "pm", "a.m", "p.m".
fn meridiem(word: Option<&str>) -> Option<Half> {
    match word? {
        "am" | "a.m" => Some(Half::Am),
        "pm" | "p.m" => Some(Half::Pm),
        _ => None,
    }
}

/// A time in figures: an hour, minutes after a colon or a dot if it has
/// them, and "am", "pm", "a" or "p" if it ends with one.
struct Figures {
    hour: u32,
    hour_figures: usize,
    minute: Option<u32>,
    colon: bool,
    half: Option<Half>,
}

fn figures_of(word: &str) -> Option<Figures> {
    let (time, half) = ["a.m", "p.m", "am", "pm", "a", "p"]
        .iter()
        .find_map(|suffix| Some((word.strip_suffix(suffix)?, suffix.starts_with('a'))))
        .map_or((word, None), |(time, am)| (time, Some(if am { Half::Am } else { Half::Pm })));
    let (hour, minute, colon) = match time.split_once([':', '.']) {
        Some((hour, minute)) => {
            if minute.len() != 2 {
                return None;
            }
            (hour, Some(figures(minute)?), time.contains(':'))
        }
        None => (time, None, false),
    };
    Some(Figures { hour: figures(hour)?, hour_figures: hour.len(), minute, colon, half })
}

/// A time of day as one word, like "1pm", "1:30", "13.00", "noon" or
/// "9ish", or as more, like "1 pm", "3 o'clock", "seven thirty", "half past
/// 3" or "quarter to 5". After a word that leads to a time, like "at", four
/// figures run together, like 1530, are one too. Says how many words it
/// took.
pub(crate) fn clock_at(words: &[&str], index: usize, lead: bool) -> Option<(Clock, usize)> {
    let word = *words.get(index)?;
    if let Some(spoken) = spoken_at(words, index) {
        return Some(spoken);
    }
    let whole = |hour, half, midnight| {
        Some((Clock { hour, minute: 0, half: Some(half), exact: true, alone: true, midnight }, 1))
    };
    match word {
        "noon" | "midday" => return whole(12, Half::Pm, false),
        "midnight" => return whole(0, Half::Am, true),
        _ => {}
    }
    let joined = lead && word.len() == 4 && word.bytes().all(|byte| byte.is_ascii_digit());
    let ish =
        word.strip_suffix("ish").filter(|time| time.starts_with(|c: char| c.is_ascii_digit()));
    let figures = if joined {
        Some(Figures {
            hour: word[..2].parse().ok()?,
            hour_figures: 2,
            minute: word[2..].parse().ok(),
            colon: true,
            half: None,
        })
    } else {
        figures_of(ish.unwrap_or(word))
    };
    let after = |size: usize| words.get(index + size).copied();
    let mut size = 1;
    let (hour, minute) = match &figures {
        Some(figures) => (figures.hour, figures.minute.unwrap_or(0)),
        // "seven", "seven thirty", "seven forty five".
        None => {
            let hour = hour_word(word)?;
            let minute = match (after(1), after(2)) {
                (Some("fifteen"), _) => Some((15, 2)),
                (Some("thirty"), _) => Some((30, 2)),
                (Some("forty-five"), _) => Some((45, 2)),
                (Some("forty"), Some("five")) => Some((45, 3)),
                _ => None,
            };
            if let Some((_, took)) = minute {
                size = took;
            }
            (hour, minute.map_or(0, |(minute, _)| minute))
        }
    };
    let mut half = figures.as_ref().and_then(|figures| figures.half);
    if half.is_none() {
        half = meridiem(after(size));
        if half.is_some() {
            size += 1;
        }
    }
    let minutes_written = figures.as_ref().is_some_and(|figures| figures.minute.is_some());
    let oclock =
        !minutes_written && minute == 0 && matches!(after(size), Some("o'clock" | "oclock"));
    if oclock {
        size += 1;
    }
    if minute > 59 || hour > 23 || (half.is_some() && (hour == 0 || hour > 12)) {
        return None;
    }
    let two_figures =
        minutes_written && figures.as_ref().is_some_and(|figures| figures.hour_figures == 2);
    let colon = figures.as_ref().is_some_and(|figures| figures.colon);
    let clock = Clock {
        hour,
        minute,
        half,
        // Written as on a 24 hour clock, like 13:00 or 09:30, it's that time.
        exact: half.is_some() || hour > 12 || hour == 0 || two_figures,
        // Said with am or pm, "o'clock" or "ish", or with minutes after a
        // colon, it's a time. Minutes after a dot need two figures before
        // it, as "2.15" is a version, and a number with nothing else, as
        // in "Apollo 13", needs an "at".
        alone: half.is_some() || oclock || ish.is_some() || colon || two_figures,
        midnight: false,
    };
    Some((clock, size))
}

/// A time said as people say them: "half past 3", "half three", "quarter
/// past 9", "quarter to 5", "10 past 4", "twenty to six". "5 to 7" and "10
/// to 12" are more likely from one to the other, and after "from" or
/// "between" they are.
fn spoken_at(words: &[&str], index: usize) -> Option<(Clock, usize)> {
    let at = |offset: usize| words.get(index + offset).copied();
    let ranged = index > 0 && matches!(words[index - 1], "from" | "between");
    let (hour, minute, mut size) = if at(0) == Some("half")
        && at(1) != Some("past")
        && hour_of(at(1)).is_some()
    {
        (hour_of(at(1))?, 30, 2)
    } else {
        let (minutes, used, in_figures) = match (at(0)?, at(1)) {
            ("twenty", Some("five")) => (25, 2, false),
            ("five", _) => (5, 1, false),
            ("ten", _) => (10, 1, false),
            ("quarter", _) => (15, 1, false),
            ("twenty", _) => (20, 1, false),
            ("twenty-five", _) => (25, 1, false),
            ("half", _) => (30, 1, false),
            (word, _) => (figures(word).filter(|minutes| (1..=30).contains(minutes))?, 1, true),
        };
        let of = hour_of(at(used + 1))?;
        match at(used)? {
            // "Quarter after 5" and "quarter of 5", as said in America.
            "past" | "after" => (of, minutes, used + 2),
            "to" | "of" | "til" | "till"
                if !ranged && minutes != 30 && (!in_figures || minutes == 20 || minutes == 25) =>
            {
                (if of == 1 { 12 } else { of - 1 }, 60 - minutes, used + 2)
            }
            _ => return None,
        }
    };
    let half = meridiem(at(size));
    if half.is_some() {
        size += 1;
    }
    Some((Clock { hour, minute, half, exact: half.is_some(), alone: true, midnight: false }, size))
}

/// The time a clock says. A bare hour, given a time it comes after, like
/// the end of something, is the first after that, or given the hours
/// something is usually in, the one in them, or else in the afternoon up
/// to 7.
pub(crate) fn clock_time(clock: Clock, after: Option<Time>, within: Option<Hours>) -> Time {
    let Clock { hour, minute, .. } = clock;
    let at = |hour: u32| Time::from_minutes(i64::from(hour * 60 + minute));
    match clock.half {
        Some(Half::Pm) => at(hour % 12 + 12),
        Some(Half::Am) => at(hour % 12),
        None if clock.exact => at(hour),
        None => {
            let (morning, evening) = (at(hour % 12), at(hour % 12 + 12));
            // The first after the start, for the end of a range.
            if let Some(after) = after {
                return if morning > after || evening <= after { morning } else { evening };
            }
            if let Some(within) = within {
                match (within.has(morning), within.has(evening)) {
                    (true, false) => return morning,
                    (false, true) => return evening,
                    _ => {}
                }
            }
            at(if hour == 12 {
                12
            } else if hour <= 7 {
                hour + 12
            } else {
                hour
            })
        }
    }
}

/// Where a range starts: "1 to 3pm" is in the afternoon all along, but "11
/// to 1pm" starts in the morning. A bare start is in the hours the thing
/// is usually in, so "breakfast 7-9" is in the morning.
pub(crate) fn range_start(from: Clock, to: Clock, within: Option<Hours>) -> Time {
    if from.half.is_some() || from.exact {
        return clock_time(from, None, None);
    }
    let Some(half) = to.half else { return clock_time(from, None, within) };
    let same = clock_time(Clock { half: Some(half), ..from }, None, None);
    if same < clock_time(to, None, None) {
        return same;
    }
    let other = if half == Half::Pm { Half::Am } else { Half::Pm };
    clock_time(Clock { half: Some(other), ..from }, None, None)
}

/// A word as it's matched: in lower case, with a straight apostrophe, and
/// without what ends a sentence.
pub(crate) fn plain_word(word: &str) -> String {
    word.to_lowercase()
        .replace('’', "'")
        .trim_end_matches([',', '.', ';', ':', '!', '?'])
        .to_string()
}

/// A time of day typed on its own, like "3pm", "15", "1530", "9:30", "half
/// three" or "noon", or `None` when it isn't one. A bare hour is in the
/// afternoon up to 7, or given a time it comes after, like the end of
/// something, the first after that.
pub fn read_clock(text: &str, after: Option<Time>) -> Option<Time> {
    let lower = text.to_lowercase();
    let mut words: Vec<String> = lower.split_whitespace().map(str::to_string).collect();
    // Three or four figures run together, like 1530, are hours and minutes.
    if let Some(first) = words.first_mut() {
        let digits = first.bytes().take_while(u8::is_ascii_digit).count();
        let rest = &first[digits..];
        if (3..=4).contains(&digits) && matches!(rest, "" | "a" | "p" | "am" | "pm") {
            *first = format!("{}:{}{rest}", &first[..digits - 2], &first[digits - 2..digits]);
        }
    }
    let plain: Vec<String> = words.iter().map(|word| plain_word(word)).collect();
    let plain: Vec<&str> = plain.iter().map(String::as_str).collect();
    let (clock, size) = clock_at(&plain, 0, true)?;
    (size == plain.len()).then(|| clock_time(clock, after, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Option<String> {
        read_clock(text, None).map(|time| time.to_string())
    }

    #[test]
    fn reads_times_in_figures() {
        assert_eq!(read("3pm").as_deref(), Some("15:00"));
        assert_eq!(read("15").as_deref(), Some("15:00"));
        assert_eq!(read("1530").as_deref(), Some("15:30"));
        assert_eq!(read("9:30").as_deref(), Some("09:30"));
        assert_eq!(read("9.30pm").as_deref(), Some("21:30"));
        assert_eq!(read("12am").as_deref(), Some("00:00"));
        assert_eq!(read("3 p.m.").as_deref(), Some("15:00"));
        assert_eq!(read("25:00"), None);
        assert_eq!(read("13pm"), None);
        assert_eq!(read("banana"), None);
    }

    #[test]
    fn takes_a_bare_hour_for_the_afternoon_up_to_seven() {
        assert_eq!(read("1").as_deref(), Some("13:00"));
        assert_eq!(read("7").as_deref(), Some("19:00"));
        assert_eq!(read("8").as_deref(), Some("08:00"));
        assert_eq!(read("12").as_deref(), Some("12:00"));
    }

    #[test]
    fn takes_a_bare_hour_for_the_first_after_a_time() {
        let after = Time::parse("14:00");
        assert_eq!(read_clock("3", after).map(|t| t.to_string()).as_deref(), Some("15:00"));
        assert_eq!(read_clock("1", after).map(|t| t.to_string()).as_deref(), Some("01:00"));
        assert_eq!(read_clock("9", after).map(|t| t.to_string()).as_deref(), Some("21:00"));
    }

    #[test]
    fn reads_times_as_they_are_said() {
        assert_eq!(read("noon").as_deref(), Some("12:00"));
        assert_eq!(read("midnight").as_deref(), Some("00:00"));
        assert_eq!(read("half three").as_deref(), Some("15:30"));
        assert_eq!(read("half past 9").as_deref(), Some("09:30"));
        assert_eq!(read("quarter to 5").as_deref(), Some("16:45"));
        assert_eq!(read("twenty five past 4").as_deref(), Some("16:25"));
        assert_eq!(read("seven thirty").as_deref(), Some("19:30"));
        assert_eq!(read("3 o'clock").as_deref(), Some("15:00"));
        assert_eq!(read("9ish").as_deref(), Some("09:00"));
        assert_eq!(read("ten to six pm").as_deref(), Some("17:50"));
        assert_eq!(read("quarter after 9").as_deref(), Some("09:15"));
        assert_eq!(read("quarter of 5").as_deref(), Some("16:45"));
        assert_eq!(read("ten til six").as_deref(), Some("17:50"));
    }
}
