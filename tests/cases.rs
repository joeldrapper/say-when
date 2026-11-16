//! Runs the library's cases: thousands of phrases, each with what it
//! should read as, a case to a line in `tests/cases/*.jsonl`.
//!
//! Where the library reads a phrase differently from how it once did, on
//! purpose, the case has a `note` saying which of the changes listed in
//! `cases/README.md` it is. To take what the library reads now as the
//! answer for every case, having looked at what changed, run `BLESS=1
//! cargo test -p say-when --features serde --test cases` and read the diff.

use say_when::{
    Calendar, Day, DayOff, EventContext, Locale, Mention, NamedDate, NamedDay, Options, Reader,
    Time, ZoneNames, read_clock,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixture {
    named: Vec<NamedDate>,
    zones: ZoneNames,
    /// Each region's public holidays, for the years the cases are read in.
    calendars: HashMap<String, Vec<DayOff>>,
    /// The days with names said in each locale that aren't built in, like
    /// Easter and Thanksgiving, for the same years.
    named_days: HashMap<Locale, Vec<NamedDays>>,
    fallback_time: Time,
}

/// A day with a name, and every day it falls on, or for one that lasts
/// longer, every time, its first and last days written `first/last`.
#[derive(Deserialize)]
struct NamedDays {
    names: Vec<String>,
    days: Vec<String>,
}

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases")
}

fn fixture() -> Fixture {
    let text = std::fs::read_to_string(directory().join("fixture.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The reader a case is read with: for its `locale`, or Britain, with
/// the public holidays of its `region`, or England and Wales's, or for
/// another locale, none unless it says, and the days with names said in
/// the locale.
fn reader(case: &Value, fixture: &Fixture) -> Reader {
    let locale = match case.get("locale") {
        Some(locale) => serde_json::from_value::<Locale>(locale.clone()).unwrap(),
        None => Locale::Uk,
    };
    let region = case["region"].as_str().or((locale == Locale::Uk).then_some("england-and-wales"));
    let reader = Reader::new(Options::new(locale))
        .with_named_dates(fixture.named.clone())
        .with_zones(fixture.zones.clone());
    let days_off = match region {
        Some(region) => fixture
            .calendars
            .get(region)
            .unwrap_or_else(|| panic!("no calendar for {region}"))
            .clone(),
        None => Vec::new(),
    };
    let named_days = fixture.named_days.get(&locale).into_iter().flatten().flat_map(|named| {
        named.days.iter().map(|days| {
            let day = |text: &str| Day::parse(text).unwrap_or_else(|| panic!("no day {text}"));
            match days.split_once('/') {
                Some((first, last)) => {
                    NamedDay::lasting(day(first), day(last), named.names.clone())
                }
                None => NamedDay::new(day(days), named.names.clone()),
            }
        })
    });
    let calendar = Calendar { days_off, named_days: named_days.collect(), ..Calendar::default() };
    let reader = reader.with_calendar(calendar);
    match case["about"].as_str() {
        Some(about) => reader.about(about),
        None => reader,
    }
}

fn day(case: &Value, key: &str) -> Day {
    Day::parse(case[key].as_str().unwrap_or_default())
        .unwrap_or_else(|| panic!("no {key} in {case}"))
}

/// Runs a file of cases through a reader, and fails with the first few
/// that read differently.
fn run(name: &str, read: impl Fn(&Value, &str, &Reader, &Fixture) -> Value) {
    let path = directory().join(format!("{name}.jsonl"));
    let fixture = fixture();
    let text = std::fs::read_to_string(&path).unwrap();
    let bless = std::env::var_os("BLESS").is_some();
    let mut blessed = String::new();
    let mut wrong = Vec::new();
    let mut count = 0;
    // A reader for each locale, region and what it's about, made once.
    let mut readers: HashMap<String, Reader> = HashMap::new();
    for line in text.lines() {
        let mut case: Value = serde_json::from_str(line).unwrap();
        let said = case["text"].as_str().unwrap().to_string();
        let key = format!("{} {} {}", case["locale"], case["region"], case["about"]);
        let reader = readers.entry(key).or_insert_with(|| reader(&case, &fixture));
        let out = read(&case, &said, reader, &fixture);
        count += 1;
        if case["out"] != out {
            wrong.push(format!("{said:?}\n  should be {}\n  but was   {out}", case["out"]));
        }
        if bless {
            case["out"] = out;
            blessed.push_str(&serde_json::to_string(&case).unwrap());
            blessed.push('\n');
        }
    }
    if bless {
        std::fs::write(&path, blessed).unwrap();
        return;
    }
    assert!(count > 0, "no cases in {name}");
    assert!(
        wrong.is_empty(),
        "{} of {count} {name} cases differ:\n{}",
        wrong.len(),
        wrong[..wrong.len().min(10)].join("\n")
    );
}

#[test]
fn days_and_spans() {
    run("days", |case, said, reader, _| {
        let today = day(case, "today");
        json!({ "days": reader.days(said, today), "span": reader.span(said, today) })
    });
}

#[test]
fn suggestions() {
    run(
        "search",
        |case, said, reader, _| json!({ "days": reader.suggest_days(said, day(case, "today")), "corrected": reader.corrected(said, &[]) }),
    );
}

#[test]
fn repeats() {
    run("repeats", |case, said, reader, _| {
        json!(reader.repeats(said, day(case, "from"), day(case, "today")))
    });
}

#[test]
fn events() {
    run("events", |case, said, reader, fixture| {
        let today = day(case, "today");
        // Each written after a "#", and said without it.
        let mentions = case["mentions"]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(Value::as_str)
                    .map(|m| Mention::new(format!("#{m}"), m))
                    .collect()
            })
            .unwrap_or_default();
        let context = EventContext { mentions, ..EventContext::new(today, fixture.fallback_time) };
        json!(reader.event(said, &context))
    });
}

#[test]
fn when() {
    run("when", |case, said, reader, _| json!(reader.when(said, day(case, "today"))));
}

/// When something is, read on its own, is when an event's sentence of it
/// says it is: every event case that's only when, and so has no title.
#[test]
fn when_is_as_an_event_says() {
    let fixture = fixture();
    let text = std::fs::read_to_string(directory().join("events.jsonl")).unwrap();
    let mut checked = 0;
    let mut readers: HashMap<String, Reader> = HashMap::new();
    for line in text.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case.get("mentions").is_some() {
            continue;
        }
        let said = case["text"].as_str().unwrap();
        let key = format!("{} {} {}", case["locale"], case["region"], case["about"]);
        let reader = readers.entry(key).or_insert_with(|| reader(&case, &fixture));
        let today = day(&case, "today");
        let Some(when) = reader.when(said, today) else { continue };
        let event = reader.event(said, &EventContext::new(today, fixture.fallback_time));
        // Only an event whose every word was read, as a word like "may" is
        // only a month when nothing else is said.
        if !event.title.is_empty() {
            continue;
        }
        checked += 1;
        assert_eq!(
            (when.start_day, when.start_time),
            (event.start_day, event.start_time),
            "{said}"
        );
        assert_eq!(when.words, event.words, "{said}");
        assert_eq!(when.time_zone, event.time_zone, "{said}");
        if when.end_time.is_some() {
            assert_eq!((when.end_day, when.end_time), (event.end_day, event.end_time), "{said}");
        }
        // Over whole days to the same day, or to the end of the span said,
        // like "next month", which an event starts on.
        if when.is_all_day() && when.end_day != event.end_day {
            let span = reader.span(said, today).map(|span| (span.first, span.last));
            assert_eq!(span, Some((when.start_day, when.end_day)), "{said}");
        }
    }
    assert!(checked > 100, "only {checked} event cases are only when");
}

#[test]
fn offsets() {
    run("offsets", |_, said, reader, _| {
        // Landed from a Monday at 2pm, and from the same day with no time.
        let from = Day::new(2026, 11, 16).unwrap();
        let two = Time::new(14, 0);
        match reader.offset(said) {
            Some(offset) => json!({
                "offset": offset,
                "said": offset.describe(),
                "from": reader.offset_from(&offset, from, two),
                "all_day": reader.offset_from(&offset, from, None),
            }),
            None => Value::Null,
        }
    });
}

#[test]
fn clocks() {
    run("clocks", |case, said, _, _| {
        let after = case["after"].as_str().and_then(Time::parse);
        json!(read_clock(said, after))
    });
}
