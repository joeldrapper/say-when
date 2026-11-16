//! Checks that each repeat in the cases comes round on the days its
//! description says. The cases are what the library read when they were
//! written, so a case can hold a rule that's wrong as well as one that's
//! right; this reads each one's description against its next days, which
//! no case does by itself. "Every month on the 3rd weekend" coming round on
//! a lone Saturday is what it's for.

use say_when::{Day, Weekday};
use serde_json::Value;

const WEEKDAYS: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

/// Every repeat in the cases: what was typed, how it's described, and the
/// next days it comes round on.
fn repeats() -> Vec<(String, String, Vec<Day>)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cases/repeats.jsonl");
    let text = std::fs::read_to_string(path).unwrap();
    let mut found = Vec::new();
    for line in text.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        let typed = case["text"].as_str().unwrap().to_string();
        for reading in case["out"].as_array().into_iter().flatten() {
            if reading["repeat"]["kind"] != "schedule" {
                continue;
            }
            let how = reading["how"].as_str().unwrap().to_string();
            let next = reading["next"].as_array().unwrap();
            let next = next.iter().map(|day| Day::parse(day.as_str().unwrap()).unwrap());
            found.push((typed.clone(), how, next.collect()));
        }
    }
    found
}

/// What a description says of when it comes round, before anything it
/// says of when it starts or stops, or of a day it's moved from.
fn rule_part(how: &str) -> Option<&str> {
    // Up to where it says when it stops or starts: ", until 25 December",
    // ", 10 times", ", once", ", from 4 November". Not ", " alone, as in
    // "Every Monday, Wednesday and Friday".
    let tail = how.match_indices(", ").map(|(at, _)| at).find(|at| {
        let rest = &how[at + 2..];
        rest.starts_with("until ")
            || rest.starts_with("from ")
            || rest.starts_with("once")
            || rest.split(' ').nth(1) == Some("times")
    });
    let said = &how[..tail.unwrap_or(how.len())];
    // Moved from a day, as in "the Friday before Christmas Day", is said of
    // that day, not the one it's on.
    (!said.contains(" before ") && !said.contains(" after ")).then_some(said)
}

/// The days of the week a description names, if it names any.
fn named_weekdays(said: &str) -> Vec<Weekday> {
    let mut days: Vec<Weekday> =
        WEEKDAYS.iter().copied().filter(|weekday| said.contains(weekday.name())).collect();
    if said.contains("weekend") {
        days.extend([Weekday::Saturday, Weekday::Sunday]);
    }
    if said.contains("weekday") || said.contains("working day") {
        days.extend(&WEEKDAYS[..5]);
    }
    days
}

#[test]
fn every_day_it_comes_round_on_is_one_it_names() {
    let mut wrong = Vec::new();
    for (typed, how, next) in repeats() {
        let Some(said) = rule_part(&how) else { continue };
        let named = named_weekdays(said);
        if named.is_empty() {
            continue;
        }
        if let Some(day) = next.iter().find(|day| !named.contains(&day.weekday())) {
            wrong.push(format!(
                "{typed:?}: {how:?} comes round on {day}, a {}",
                day.weekday().name()
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_weekend_is_a_saturday_and_the_sunday_after() {
    let mut wrong = Vec::new();
    for (typed, how, next) in repeats() {
        let Some(said) = rule_part(&how) else { continue };
        if !said.contains("weekend") || said.contains("weekend day") {
            continue;
        }
        for (at, day) in next.iter().enumerate() {
            // A Saturday last in the list, or a Sunday first, may have the
            // other half of its weekend out of it.
            let partner = match day.weekday() {
                Weekday::Saturday if at + 1 < next.len() => Some((next[at + 1], 1)),
                Weekday::Sunday if at > 0 => Some((next[at - 1], -1)),
                _ => None,
            };
            if let Some((other, apart)) = partner
                && day.add_days(apart) != Some(other)
            {
                wrong.push(format!("{typed:?}: {how:?} has {day} without the rest of its weekend"));
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_so_many_days_is_that_many_apart() {
    let mut wrong = Vec::new();
    for (typed, how, next) in repeats() {
        let Some(said) = rule_part(&how) else { continue };
        let apart = match said.strip_prefix("Every ") {
            Some("day") => 1,
            Some(rest) => match rest.strip_suffix(" days").map(str::parse::<i64>) {
                Some(Ok(n)) => n,
                _ => continue,
            },
            None => continue,
        };
        for pair in next.windows(2) {
            if pair[0].days_until(pair[1]) != apart {
                wrong.push(format!("{typed:?}: {how:?} goes from {} to {}", pair[0], pair[1]));
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_nth_weekday_of_a_month_is_in_its_nth_week() {
    let mut wrong = Vec::new();
    for (typed, how, next) in repeats() {
        let Some(said) = rule_part(&how) else { continue };
        for (nth, word) in
            [(1, "1st"), (2, "2nd"), (3, "3rd"), (4, "4th"), (5, "5th"), (-1, "last")]
        {
            let one = WEEKDAYS
                .iter()
                .find(|weekday| said.contains(&format!("the {word} {}", weekday.name())));
            let Some(weekday) = one else { continue };
            for day in &next {
                let week = (day.day() as i32 - 1) / 7 + 1;
                let last = day.day() + 7 > day.month_end().day();
                let right = if nth == -1 { last } else { week == nth };
                if day.weekday() != *weekday || !right {
                    wrong.push(format!("{typed:?}: {how:?} comes round on {day}"));
                    break;
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
