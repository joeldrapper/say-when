//! What holds whatever a rule, a calendar or a phrase is: thousands of each
//! made at random, and any that breaks it made as small as it can be.

use proptest::prelude::*;
use say_when::{
    Calendar, Day, DayOff, EventContext, Offset, OffsetUnit, Options, Reader, Repeat, Rule, Time,
    Weekday, WordRole, read_clock,
};

/// A day from 1990 to 2060.
fn a_day() -> impl Strategy<Value = Day> {
    (1990i32..2060, 1u32..=12, 1u32..=28)
        .prop_map(|(year, month, day)| Day::new(year, month, day).expect("a day of every month"))
}

/// A calendar: any working week of a day or more, and days off around a
/// day, alone or in runs as long as a closure.
fn a_calendar(around: Day) -> impl Strategy<Value = Calendar> {
    let week = proptest::sample::subsequence(Weekday::ALL.to_vec(), 1..=7);
    let runs = proptest::collection::vec((-60i64..60, 1i64..=20), 0..4);
    let singles = proptest::collection::vec(-120i64..120, 0..8);
    (week, runs, singles).prop_map(move |(working_week, runs, singles)| {
        let mut off: Vec<i64> = singles;
        for (from, long) in runs {
            off.extend(from..from + long);
        }
        let days_off = off
            .into_iter()
            .filter_map(|away| around.add_days(away))
            .map(|day| DayOff::new(day, None))
            .collect();
        Calendar { days_off, working_week, ..Calendar::default() }
    })
}

/// The days of a month a monthly or yearly rule is on: none said, days of
/// the month, the nth of a day of the week, or one of several days of the
/// week picked by where it is.
fn month_days() -> impl Strategy<Value = String> {
    let plain = || {
        proptest::sample::subsequence(vec!["MO", "TU", "WE", "TH", "FR", "SA", "SU"], 1..=5)
            .prop_map(|days| days.join(","))
    };
    prop_oneof![
        Just(String::new()),
        proptest::sample::subsequence(vec!["1", "15", "28", "31", "-1", "-7"], 1..=2)
            .prop_map(|days| format!(";BYMONTHDAY={}", days.join(","))),
        proptest::sample::subsequence(vec!["1MO", "-1FR", "2TU", "3SU", "-2WE"], 1..=2)
            .prop_map(|days| format!(";BYDAY={}", days.join(","))),
        (plain(), proptest::sample::select(vec![1, 2, -1]))
            .prop_map(|(days, at)| format!(";BYDAY={days};BYSETPOS={at}")),
    ]
}

/// An RRULE of any shape this works out the days of.
fn rule_text() -> impl Strategy<Value = String> {
    let weekly =
        proptest::sample::subsequence(vec!["MO", "TU", "WE", "TH", "FR", "SA", "SU"], 0..=3)
            .prop_map(|days| {
                if days.is_empty() {
                    "FREQ=WEEKLY".to_string()
                } else {
                    format!("FREQ=WEEKLY;BYDAY={}", days.join(","))
                }
            });
    let shape = prop_oneof![
        Just("FREQ=DAILY".to_string()),
        weekly,
        month_days().prop_map(|days| format!("FREQ=MONTHLY{days}")),
        (1u32..=12, month_days()).prop_map(|(month, days)| {
            if days.is_empty() {
                "FREQ=YEARLY".to_string()
            } else {
                format!("FREQ=YEARLY;BYMONTH={month}{days}")
            }
        }),
    ];
    let interval = proptest::option::of(2u32..=4);
    let count = proptest::option::of(1u32..=40);
    let skips = any::<bool>();
    let offset = proptest::option::of(proptest::sample::select(vec![
        "-P1D",
        "P2D",
        "-P1W",
        "-2WORKDAY",
        "+1WORKDAY",
        "-1WEEKDAY",
        "+MO",
        "-FR",
        "-WEEKEND",
        "+WEEKEND",
        "P0D",
    ]));
    (shape, interval, count, skips, offset).prop_map(
        |(mut text, interval, count, skips, offset)| {
            if let Some(interval) = interval {
                text += &format!(";INTERVAL={interval}");
            }
            if let Some(count) = count {
                text += &format!(";COUNT={count}");
            }
            if skips {
                text += ";X-SKIP-DAYS-OFF=TRUE";
            }
            if let Some(offset) = offset {
                text += &format!(";X-OFFSET={offset}");
            }
            text
        },
    )
}

/// A rule this can work out the days of.
fn a_rule() -> impl Strategy<Value = Rule> {
    rule_text().prop_filter_map("a rule it works out", |text| Rule::parse(&text))
}

/// A series that starts on the first day it comes round on from a day,
/// with a reader for a calendar around it.
fn a_series() -> impl Strategy<Value = (Reader, Repeat)> {
    (a_rule(), a_day().prop_flat_map(|day| (Just(day), a_calendar(day)))).prop_filter_map(
        "a series that comes round",
        |(rule, (from, calendar))| {
            let reader = Reader::new(Options::default()).with_calendar(calendar);
            let looking = Repeat::schedule(rule.clone(), from);
            let starts = reader.repeat_days(&looking, from, from.add_days(3 * 366)?).next()?;
            Some((reader, Repeat::schedule(rule, starts)))
        },
    )
}

/// The days a series comes round on from one day to another.
fn days(reader: &Reader, repeat: &Repeat, from: Day, to: Day) -> Vec<Day> {
    reader.repeat_days(repeat, from, to).collect()
}

proptest! {
    // Nothing's kept between runs: a failure says what failed, as small as
    // it can be made, to be made a case of its own.
    #![proptest_config(ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn nothing_typed_panics(text in "\\PC{0,24}") {
        Rule::parse(&text);
        text.parse::<Offset>().ok();
        Day::parse(&text);
        Time::parse(&text);
        read_clock(&text, None);
        let reader = Reader::new(Options::default());
        let today = Day::new(2026, 11, 16).unwrap();
        reader.offset(&text);
        let event = reader.event(&text, &EventContext::new(today, Time::new(9, 0).unwrap()));
        let mut last = 0;
        for word in &event.words {
            prop_assert!(last <= word.start && word.start < word.end);
            prop_assert!(!text[word.start..word.end].trim().is_empty());
            last = word.end;
        }
        if let Some(when) = reader.when(&text, today) {
            prop_assert!(when.words.iter().all(|word| word.role != WordRole::Title));
            prop_assert_eq!(&when.words, &event.words);
            prop_assert_eq!((when.start_day, when.start_time), (event.start_day, event.start_time));
        }
    }

    #[test]
    fn a_rule_reads_back_as_it_was_written(rule in a_rule()) {
        let written = rule.to_string();
        prop_assert_eq!(Rule::parse(&written), Some(rule), "{}", written);
    }

    #[test]
    fn a_series_comes_round_on_the_day_it_starts((reader, repeat) in a_series()) {
        let to = repeat.starts.add_days(400).unwrap();
        let found = days(&reader, &repeat, repeat.starts, to);
        prop_assert_eq!(found.first(), Some(&repeat.starts), "{}", repeat.rule);
    }

    #[test]
    fn a_series_days_are_in_order_and_in_the_window(
        (reader, repeat) in a_series(),
        from in -40i64..400,
        long in 0i64..400,
    ) {
        let from = repeat.starts.add_days(from).unwrap();
        let to = from.add_days(long).unwrap();
        let found = days(&reader, &repeat, from, to);
        prop_assert!(found.windows(2).all(|pair| pair[0] < pair[1]), "{}: {:?}", repeat.rule, found);
        prop_assert!(found.iter().all(|day| (from..=to).contains(day) && *day >= repeat.starts));
        if let Some(count) = repeat.rule.count {
            prop_assert!(found.len() <= count as usize);
        }
    }

    #[test]
    fn a_window_split_in_two_has_the_same_days(
        (reader, repeat) in a_series(),
        from in -40i64..300,
        first in 0i64..200,
        second in 0i64..200,
    ) {
        let from = repeat.starts.add_days(from).unwrap();
        let middle = from.add_days(first).unwrap();
        let to = middle.add_days(1 + second).unwrap();
        let whole = days(&reader, &repeat, from, to);
        let mut halves = days(&reader, &repeat, from, middle);
        halves.extend(days(&reader, &repeat, middle.add_days(1).unwrap(), to));
        prop_assert_eq!(whole, halves, "{}", repeat.rule);
    }

    #[test]
    fn a_series_is_on_each_of_its_days_and_no_others(
        (reader, repeat) in a_series(),
        from in -10i64..200,
    ) {
        let from = repeat.starts.add_days(from).unwrap();
        let to = from.add_days(60).unwrap();
        let found = days(&reader, &repeat, from, to);
        let mut day = from;
        while day <= to {
            prop_assert_eq!(reader.repeats_on(&repeat, day), found.contains(&day), "{} on {}", repeat.rule, day);
            day = day.add_days(1).unwrap();
        }
    }

    #[test]
    fn a_skipped_day_is_left_out_and_still_counted(
        (reader, mut repeat) in a_series(),
        skip in proptest::collection::vec(any::<prop::sample::Index>(), 0..4),
    ) {
        let to = repeat.starts.add_days(500).unwrap();
        let all = days(&reader, &repeat, repeat.starts, to);
        prop_assume!(!all.is_empty());
        repeat.skipped = skip.iter().map(|index| *index.get(&all)).collect();
        let left = days(&reader, &repeat, repeat.starts, to);
        let expected: Vec<Day> =
            all.iter().copied().filter(|day| !repeat.skipped.contains(day)).collect();
        prop_assert_eq!(left, expected, "{}", repeat.rule);
    }

    #[test]
    fn working_days_land_on_a_working_day_so_many_away(
        (from, calendar) in a_day().prop_flat_map(|day| (Just(day), a_calendar(day))),
        count in -40i64..=40,
    ) {
        let reader = Reader::new(Options::default()).with_calendar(calendar);
        let offset = Offset::by(count, OffsetUnit::WorkingDay);
        let (to, _) = reader.offset_from(&offset, from, None).unwrap();
        if count == 0 {
            prop_assert_eq!(to, from);
        } else {
            prop_assert!(reader.is_working_day(to));
            let (first, last) = if count > 0 { (from.add_days(1).unwrap(), to) } else { (to, from.add_days(-1).unwrap()) };
            let mut worked = 0;
            let mut day = first;
            while day <= last {
                worked += i64::from(reader.is_working_day(day));
                day = day.add_days(1).unwrap();
            }
            prop_assert_eq!(worked, count.abs());
        }
    }

    #[test]
    fn an_offset_reads_back_as_it_was_written(
        count in -400i64..400,
        unit in prop_oneof![
            Just(OffsetUnit::Minute), Just(OffsetUnit::Hour), Just(OffsetUnit::Day),
            Just(OffsetUnit::Week), Just(OffsetUnit::Month), Just(OffsetUnit::Year),
            Just(OffsetUnit::WorkingDay), Just(OffsetUnit::Weekday),
        ],
        hour in proptest::option::of(0u32..24),
    ) {
        let offset = Offset::by(count, unit);
        let offset = match hour {
            Some(hour) => offset.at(Time::new(hour, 30).unwrap()).unwrap_or(offset),
            None => offset,
        };
        prop_assert_eq!(offset.to_string().parse::<Offset>(), Ok(offset));
    }
}
