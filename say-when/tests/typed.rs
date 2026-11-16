//! Whatever's typed, nothing here panics, loops or gives a day that can't
//! be written: words from what the readers know, numbers of every size and
//! what joins them, put together at random, thousands of times over.

use say_when::{Calendar, Day, DayOff, EventContext, Locale, Options, Reader, Time, read_clock};

const WORDS: [&str; 64] = [
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
    // A reader for each place, with a calendar of a few days off.
    let off = |day: &str, name: Option<&str>| DayOff {
        day: Day::parse(day).unwrap(),
        name: name.map(str::to_string),
    };
    let calendar = Calendar {
        days_off: vec![
            off("2026-12-25", Some("Christmas Day")),
            off("2027-05-03", Some("Early May bank holiday")),
            off("9999-12-31", None),
            off("0001-01-01", Some("the of")),
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
            today,
            fallback_day: today,
            fallback_time: Time::MIDNIGHT,
            mentions: &["Priya"],
            only_title: false,
        };
        let event = reader.event(&text, &context);
        assert!(event.start_day <= event.end_day, "{text:?}: {event:?}");
        read_clock(&text, None);
        reader.corrected(&text, &[]);
    }
}
