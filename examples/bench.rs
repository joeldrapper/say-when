//! Times each reader over the library's own cases, natively:
//!
//!     cargo run --release --example bench --features serde [reader…]
//!
//! Every case is read as Britain reads it, with England's holidays, whatever
//! locale it's for. Each case is read many times over, and the time for one reading is
//! said, as the mean over every case of the file and for the slowest case.

use say_when::{
    Calendar, Day, DayOff, EventContext, NamedDate, Options, Reader, Time, ZoneNames, read_clock,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Deserialize)]
struct Fixture {
    named: Vec<NamedDate>,
    zones: ZoneNames,
    calendars: HashMap<String, Vec<DayOff>>,
    fallback_time: Time,
}

fn main() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let fixture: Fixture =
        serde_json::from_str(&std::fs::read_to_string(directory.join("fixture.json")).unwrap())
            .unwrap();
    let reader = Reader::new(Options::default())
        .with_calendar(Calendar {
            days_off: fixture.calendars["england-and-wales"].clone(),
            ..Calendar::default()
        })
        .with_named_dates(fixture.named.clone())
        .with_zones(fixture.zones);
    let cases = |name: &str| -> Vec<Value> {
        let text = std::fs::read_to_string(directory.join(format!("{name}.jsonl"))).unwrap();
        text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    };
    let day = |case: &Value, key: &str| Day::parse(case[key].as_str().unwrap()).unwrap();

    // Times a reading of every case, a few times round, and says the mean
    // of the quickest round and the slowest case in it.
    // Only the readers named, when any are: `bench event`.
    let only: Vec<String> = std::env::args().skip(1).collect();
    let time = |name: &str, what: &str, read: &dyn Fn(&Value, &str)| {
        if !only.is_empty() && !only.iter().any(|each| what.contains(each.as_str())) {
            return;
        }
        let all = cases(name);
        let mut best = f64::MAX;
        let mut slowest = (0.0, String::new());
        for _ in 0..5 {
            let began = Instant::now();
            let mut worst = (0.0, String::new());
            for case in &all {
                let said = case["text"].as_str().unwrap();
                let each = Instant::now();
                read(case, said);
                let took = each.elapsed().as_secs_f64() * 1e6;
                if took > worst.0 {
                    worst = (took, said.chars().take(50).collect());
                }
            }
            let mean = began.elapsed().as_secs_f64() * 1e6 / all.len() as f64;
            if mean < best {
                best = mean;
                slowest = worst;
            }
        }
        println!(
            "{what:<28} {best:>8.2} µs a reading over {:>5} cases; slowest {:>8.1} µs: {:?}",
            all.len(),
            slowest.0,
            slowest.1
        );
    };

    time("days", "a day", &|case, said| {
        black_box(reader.days(said, day(case, "today")));
    });
    time("days", "a span", &|case, said| {
        black_box(reader.span(said, day(case, "today")));
    });
    time("search", "the picker's search", &|case, said| {
        black_box(reader.suggest_days(said, day(case, "today")));
    });
    time("repeats", "a repeat", &|case, said| {
        black_box(reader.repeats(said, day(case, "from"), day(case, "today")));
    });
    time("events", "an event", &|case, said| {
        let today = day(case, "today");
        let context = EventContext::new(today, fixture.fallback_time);
        black_box(reader.event(said, &context));
    });
    time("clocks", "a time of day", &|_, said| {
        black_box(read_clock(said, None));
    });
}
