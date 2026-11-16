//! Whatever's typed, nothing here panics, loops or gives a day that can't
//! be written: words from what the readers know, numbers of every size and
//! what joins them, put together at random, thousands of times over.

use say_when::{
    Calendar, Day, DayOff, EventContext, Locale, Mention, NamedDay, Offset, Options, Reader, Time,
    read_clock,
};

const WORDS: [&str; 68] = [
    "the",
    "of",
    "in",
    "on",
    "at",
    "to",
    "from",
    "until",
    "for",
    "every",
    "other",
    "next",
    "last",
    "this",
    "before",
    "after",
    "ago",
    "day",
    "days",
    "week",
    "weeks",
    "month",
    "year",
    "years",
    "working",
    "weekday",
    "weekend",
    "monday",
    "fri",
    "sun",
    "november",
    "feb",
    "may",
    "christmas",
    "easter",
    "first",
    "2nd",
    "3rd",
    "31st",
    "last",
    "end",
    "start",
    "mid",
    "q1",
    "today",
    "tomorrow",
    "noon",
    "half",
    "past",
    "quarter",
    "pm",
    "am",
    "times",
    "twice",
    "+",
    "-",
    "a",
    "two",
    "twenty",
    "birthday",
    "mum",
    "all",
    "rest",
    "#Priya",
    "hour",
    "minutes",
    "an",
    "end",
];
const NUMBERS: [&str; 16] = [
    "0",
    "1",
    "2",
    "3",
    "7",
    "12",
    "13",
    "29",
    "31",
    "99",
    "1999",
    "2027",
    "9999",
    "100000",
    "99999999",
    "999999999999999999999",
];
const PIECES: [&str; 12] =
    ["2027-02-29", "9/12", "31/2/99", "2-4pm", "13:60", "9.30", "1h30", "é", "🎂", "", "'s", "–"];

/// A number generator that's the same every run, so a failure can be run again.
struct Random(u64);

impl Random {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % below
    }
}

#[test]
fn nothing_typed_panics_or_leaves_the_calendar() {
    // A reader for each place, with a calendar of a few days off and a
    // few with names, some at the ends of time.
    let off = |day: &str, name: Option<&str>| DayOff::new(Day::parse(day).unwrap(), name);
    let named = |day: &str, names: &[&str]| NamedDay::new(Day::parse(day).unwrap(), names.to_vec());
    let calendar = Calendar {
        days_off: vec![
            off("2026-12-25", Some("Christmas Day")),
            off("2027-05-03", Some("Early May bank holiday")),
            off("9999-12-31", None),
            off("0001-01-01", Some("the of")),
        ],
        named_days: vec![
            named("2026-04-05", &["Easter Sunday", "easter"]),
            named("2027-03-28", &["Easter Sunday", "easter"]),
            named("9999-12-30", &["The End", "end"]),
            named("0001-01-02", &["Mother’s Day", "2nd", "next"]),
            named("2026-11-26", &[]),
        ],
        ..Calendar::default()
    };
    let readers: Vec<Reader> = Locale::ALL
        .iter()
        .map(|locale| Reader::new(Options::new(*locale)).with_calendar(calendar.clone()))
        .collect();
    let mut random = Random(20261115);
    let days = ["0001-01-01", "2026-10-20", "2028-02-29", "9999-12-31"];
    for round in 0..20_000 {
        let length = 1 + random.next(9);
        let mut text = String::new();
        for _ in 0..length {
            let word = match random.next(10) {
                0..=5 => WORDS[random.next(WORDS.len())],
                6..=8 => NUMBERS[random.next(NUMBERS.len())],
                _ => PIECES[random.next(PIECES.len())],
            };
            text.push_str(word);
            text.push_str(if random.next(8) == 0 { "" } else { " " });
        }
        let today = Day::parse(days[round % days.len()]).unwrap();
        let reader = &readers[random.next(readers.len())];
        let within = |day: Day| assert!((Day::MIN..=Day::MAX).contains(&day), "{text:?}");

        reader.days(&text, today).iter().for_each(|reading| within(reading.day));
        reader.suggest_days(&text, today).iter().for_each(|found| within(found.day));
        if let Some(span) = reader.span(&text, today) {
            assert!(span.first <= span.last, "{text:?}");
        }
        for reading in reader.repeats(&text, today, today) {
            reading.next.iter().for_each(|day| within(*day));
        }
        let context = EventContext {
            mentions: vec![Mention::new("#Priya", "Priya")],
            ..EventContext::new(today, Time::MIDNIGHT)
        };
        let event = reader.event(&text, &context);
        assert!(event.start_day <= event.end_day, "{text:?}: {event:?}");
        if let Some(offset) = reader.offset(&text) {
            assert_eq!(offset.to_string().parse::<Offset>(), Ok(offset), "{text:?}");
            offset.describe();
            reader.offset_from(&offset, today, Some(Time::MIDNIGHT));
        }
        "-P1D@09:00x".parse::<Offset>().ok();
        text.parse::<Offset>().ok();
        read_clock(&text, None);
        reader.corrected(&text, &[]);
    }
}
