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

#[test]
fn counts_from_the_day_an_offset_moves_to_the_start() {
    let reader = Reader::new(Options::default());
    let today = day("2026-01-07");
    let every = &reader.repeats("the monday after every sunday", today, today)[0];
    assert_eq!(every.repeat.starts, day("2026-01-12"));
    assert_eq!(every.next[..2], [day("2026-01-12"), day("2026-01-19")]);
    // Every other one keeps to the weeks of the Sunday it starts from.
    let other = &reader.repeats("the monday after every other sunday", today, today)[0];
    assert_eq!(other.next[..2], [day("2026-01-12"), day("2026-01-26")]);
    let days: Vec<Day> = reader.repeat_days(&other.repeat, today, day("2026-02-28")).collect();
    assert_eq!(days, [day("2026-01-12"), day("2026-01-26"), day("2026-02-09"), day("2026-02-23")]);
}

#[test]
fn works_the_days_the_calendar_does() {
    use say_when::{Calendar, Weekday};
    let week = vec![
        Weekday::Sunday,
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
    ];
    let calendar = Calendar { working_week: week, ..Calendar::default() };
    let reader = Reader::new(Options::default()).with_calendar(calendar);
    let today = day("2026-01-07");
    let working = &reader.repeats("every working day", today, today)[0];
    assert_eq!(
        working.next,
        [day("2026-01-07"), day("2026-01-08"), day("2026-01-11"), day("2026-01-12")]
    );
    assert_eq!(working.how, "Every working day");
    let first = &reader.repeats("the first working day of every month", today, today)[0];
    assert_eq!(first.next[0], day("2026-02-01"));
    assert_eq!(first.how, "Every month on the 1st working day");
}

#[test]
fn a_weekend_working_week_has_a_first_working_day() {
    use say_when::{Calendar, Weekday};
    let calendar =
        Calendar { working_week: vec![Weekday::Saturday, Weekday::Sunday], ..Calendar::default() };
    let reader = Reader::new(Options::default()).with_calendar(calendar);
    let today = day("2026-10-28");
    let first = &reader.repeats("the first working day of every month", today, today)[0];
    assert_eq!(first.next[..2], [day("2026-11-01"), day("2026-12-05")]);
    assert_eq!(first.how, "Every month on the 1st working day");
    // Though a weekend picked out of a month is no rule.
    assert!(reader.repeats("the first weekend of every month", today, today).is_empty());
}

#[test]
fn looks_past_days_off_however_many_there_are() {
    use say_when::{Calendar, DayOff};
    let days_off = (6..=20).map(|n| DayOff::new(Day::new(2026, 1, n).unwrap(), None)).collect();
    let reader =
        Reader::new(Options::default()).with_calendar(Calendar { days_off, ..Calendar::default() });
    let every = repeat("FREQ=DAILY;X-SKIP-DAYS-OFF=TRUE;X-OFFSET=-1WORKDAY", "2026-01-02");
    assert!(reader.repeats_on(&every, day("2026-01-05")));
    let days: Vec<Day> = reader.repeat_days(&every, day("2026-01-05"), day("2026-01-21")).collect();
    assert_eq!(days, [day("2026-01-05"), day("2026-01-21")]);
}

#[test]
fn reads_no_rule_from_what_isnt_one() {
    for text in ["12345é", "RRULEé", "é", "RRULE:é", "FREQ=DAILYé"] {
        assert_eq!(Rule::parse(text), None, "{text}");
    }
}
