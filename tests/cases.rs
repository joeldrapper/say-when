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

/// A day with a name, and every day it falls on.
#[derive(Deserialize)]
struct NamedDays {
    names: Vec<String>,
    days: Vec<Day>,
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
    let named_days =
        fixture.named_days.get(&locale).into_iter().flatten().flat_map(|named| {
            named.days.iter().map(|day| NamedDay::new(*day, named.names.clone()))
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
fn clocks() {
    run("clocks", |case, said, _, _| {
        let after = case["after"].as_str().and_then(Time::parse);
        json!(read_clock(said, after))
    });
}
