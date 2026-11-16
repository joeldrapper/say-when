//! A series' days, as a calendar asks for them.

use say_when::{Day, Offset, OffsetUnit, Options, Reader, Repeat, Rule, Time};

fn day(text: &str) -> Day {
    Day::parse(text).unwrap()
}

fn repeat(rule: &str, starts: &str) -> Repeat {
    Repeat::schedule(Rule::parse(rule).unwrap(), day(starts))
}

#[test]
fn gives_a_series_days_between_two() {
    let reader = Reader::new(Options::default());
    let tuesdays = repeat("FREQ=WEEKLY;BYDAY=TU", "2026-11-03");
    let days: Vec<String> = reader
        .repeat_days(&tuesdays, day("2026-11-16"), day("2026-12-06"))
        .map(|day| day.to_string())
        .collect();
    assert_eq!(days, ["2026-11-17", "2026-11-24", "2026-12-01"]);
    // However far on, and as many as are asked for.
    let far = reader.repeat_days(&tuesdays, day("4000-01-01"), Day::MAX).next();
    assert_eq!(far, Some(day("4000-01-04")));
    assert_eq!(reader.repeat_days(&tuesdays, day("2026-01-01"), Day::MAX).take(500).count(), 500);
    // None before it starts, nor from a day after the one to.
    assert_eq!(reader.repeat_days(&tuesdays, day("2026-10-01"), day("2026-11-03")).count(), 1);
    assert_eq!(reader.repeat_days(&tuesdays, day("2026-12-01"), day("2026-11-01")).count(), 0);
}

#[test]
fn says_whether_a_series_is_on_a_day_and_its_last() {
    let reader = Reader::new(Options::default());
    let tuesdays = repeat("FREQ=WEEKLY;BYDAY=TU", "2026-11-03");
    assert!(reader.repeats_on(&tuesdays, day("2026-11-17")));
    assert!(!reader.repeats_on(&tuesdays, day("2026-11-18")));
    assert!(!reader.repeats_on(&tuesdays, day("2026-10-27")));
    assert_eq!(reader.last_repeat_day(&tuesdays), None);
    let ten = repeat("FREQ=WEEKLY;BYDAY=TU;COUNT=10", "2026-11-03");
    assert_eq!(reader.last_repeat_day(&ten), Some(day("2027-01-05")));
    let until = repeat("FREQ=MONTHLY;BYMONTHDAY=31;UNTIL=20270615", "2026-10-31");
    assert_eq!(reader.last_repeat_day(&until), Some(day("2027-05-31")));
    // One that's done and so comes round after has no days of its own.
    let after = Repeat::after_done(Rule::parse("FREQ=WEEKLY").unwrap(), day("2026-11-16"));
    assert_eq!(reader.repeat_days(&after, day("2026-11-01"), day("2026-12-31")).count(), 0);
    assert!(!reader.repeats_on(&after, day("2026-11-23")));
}

#[test]
fn leaves_out_the_days_it_skips() {
    let reader = Reader::new(Options::default());
    let mut ten = repeat("FREQ=WEEKLY;BYDAY=TU;COUNT=3", "2026-11-03");
    ten.skipped = vec![day("2026-11-10")];
    let days: Vec<String> =
        reader.repeat_days(&ten, day("2026-11-01"), Day::MAX).map(|day| day.to_string()).collect();
    // A day skipped still counts towards how many times.
    assert_eq!(days, ["2026-11-03", "2026-11-17"]);
    assert!(!reader.repeats_on(&ten, day("2026-11-10")));
    assert_eq!(reader.next_days(&ten, day("2026-11-04"), 5), [day("2026-11-17")]);
    assert_eq!(reader.last_repeat_day(&ten), Some(day("2026-11-17")));
}

#[test]
fn finds_a_day_however_far_off() {
    let reader = Reader::new(Options::default());
    let rule = Rule::parse("FREQ=YEARLY;INTERVAL=100").unwrap();
    let repeat = Repeat::schedule(rule, Day::new(2000, 1, 1).unwrap());
    let next = reader.next_days(&repeat, Day::new(2001, 1, 1).unwrap(), 2);
    assert_eq!(next, [Day::new(2100, 1, 1).unwrap(), Day::new(2200, 1, 1).unwrap()]);
}

#[test]
fn takes_no_interval_as_one() {
    let reader = Reader::new(Options::default());
    let mut rule = Rule::parse("FREQ=DAILY").unwrap();
    rule.interval = 0;
    assert_eq!(rule.to_string(), "FREQ=DAILY");
    let start = Day::new(2026, 11, 16).unwrap();
    let repeat = Repeat::schedule(rule, start);
    assert!(reader.repeats_on(&repeat, start.add_days(3).unwrap()));
    assert_eq!(reader.next_days(&repeat, start, 2), [start, start.add_days(1).unwrap()]);
}

#[test]
fn moves_no_further_than_can_be_said() {
    let reader = Reader::new(Options::default());
    let day = Day::new(2026, 11, 16).unwrap();
    let units = [
        OffsetUnit::Minute,
        OffsetUnit::Hour,
        OffsetUnit::Day,
        OffsetUnit::Week,
        OffsetUnit::Month,
        OffsetUnit::Year,
        OffsetUnit::WorkingDay,
        OffsetUnit::Weekday,
    ];
    for unit in units {
        for count in [i64::MIN, i64::MAX] {
            for time in [None, Time::new(9, 0)] {
                assert_eq!(reader.offset_from(&Offset::by(count, unit), day, time), None);
            }
        }
    }
}
