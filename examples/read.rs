//! Reads what's given as a day, a repeat and an event, on today's date or
//! the day given first:
//!
//!     cargo run --example read -- "the last working day of november"
//!     cargo run --example read -- 2026-10-20 "every other thursday until christmas"

use say_when::{Day, EventContext, Options, Reader, Time};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let today = match args.first().and_then(|first| Day::parse(first)) {
        Some(day) => {
            args.remove(0);
            day
        }
        None => Day::new(2026, 10, 20).expect("a real day"),
    };
    let text = args.join(" ");
    let reader = Reader::new(Options::default());

    println!("On {today}, {text:?} reads as");
    for reading in reader.days(&text, today) {
        println!("  the day {} {}", reading.day, reading.how.unwrap_or_default());
    }
    for reading in reader.repeats(&text, today, today) {
        println!("  repeating: {} ({}), next {:?}", reading.how, reading.repeat.rule, reading.next);
    }
    let context = EventContext::new(today, Time::new(9, 0).expect("a real time"));
    let event = reader.event(&text, &context);
    println!(
        "  the event {:?} from {} {} to {} {}{}",
        event.title,
        event.start_day,
        event.start_time.map(|time| time.to_string()).unwrap_or_default(),
        event.end_day,
        event.end_time.map(|time| time.to_string()).unwrap_or_default(),
        event.location.map(|place| format!(" at {place}")).unwrap_or_default(),
    );
    if let Some(repeat) = event.repeat {
        println!("  repeating: {} ({})", repeat.how, repeat.repeat.rule);
    }
}
