# say-when

Reads days, spans of days, times and how things repeat, written out in
plain English, the way people type them into a date picker or a calendar:

- **a day**: "tomorrow", "next fri", "9 dec", "the last working day of
  november", "3 working days before christmas", "a fortnight after next
  friday", "mum's birthday"
- **a span of days**: "next week", "the weekend", "november", "q3 2027"
- **a search, as it's typed**: "t" is Today, Tomorrow, Tuesday and Thursday
- **a time of day**: "1pm", "13:30", "half three", "quarter to 5", "noon"
- **how something repeats**, as an iCalendar RRULE: "every other thursday",
  "the last working day of the month", "weekly until christmas"
- **an event typed as a sentence**: "Lunch with Priya tomorrow at 1 at
  Monmouth", "Standup every weekday at 9:30 for 15 mins"

The last word can be half typed, numbers can be words, and a phrase that
could mean more than one day gives each, the likeliest first.

```rust
use say_when::{Day, Options, Reader};

let reader = Reader::new(Options::default());
let today = Day::new(2026, 10, 20).unwrap();

let read = reader.days("the day before the last working day of november", today);
assert_eq!(read[0].day.to_string(), "2026-11-29");
assert_eq!(read[0].how, "the day before Mon 30 Nov, the last working day of November 2026");

let repeats = reader.repeats("every other thursday until christmas", today, today);
assert_eq!(repeats[0].repeat.rule.to_string(), "FREQ=WEEKLY;INTERVAL=2;BYDAY=TH;UNTIL=20261225");
assert_eq!(repeats[0].how, "Every 2 weeks on Thursday, until 25 December");
```

`cargo run --example read -- "next friday"` tries a phrase.

## What it's told

It has no clock, no time zones, no address book and no country's public
holidays. The caller says:

- **the day it is**, with every reading;
- **where it's reading** (`Locale`): the UK, the US, Ireland, Australia,
  New Zealand or Canada. That decides the order of a date in figures
  ("3/4" is 3 April in the UK and 4 March in the US, and both in Canada),
  the day a week starts on, how a day is labelled ("Thu 30 Nov 2028" or
  "Thu, Nov 30, 2028"), and the days that have names there (Thanksgiving,
  Boxing Day, Anzac Day);
- **the days that aren't worked** (`Reader::with_calendar`): public
  holidays, each with what it's called, and the days of the week that are
  worked. They decide what a working day is, and their names can be said:
  "the august bank holiday", "the next public holiday". With none, a
  working day is Monday to Friday. The `say-when-holidays` crate works them
  out by rule for some places, for callers with no better source;
- **named dates**, for days only it knows by name, like people's birthdays
  (`Reader::with_named_dates`);
- **the names time zones go by**, for an event's sentence to say which its
  times are in (`Reader::with_zones`). What each name stands for is given
  back as it was given.

```rust
use say_when::{Calendar, Day, DayOff, Locale, Options, Reader};

let thanksgiving = DayOff { day: Day::new(2026, 11, 26).unwrap(), name: Some("Thanksgiving".into()) };
let reader = Reader::new(Options::new(Locale::Us))
    .with_calendar(Calendar { days_off: vec![thanksgiving], ..Calendar::default() });
let today = Day::new(2026, 10, 20).unwrap();

let read = reader.days("2 working days after 11/25", today);
assert_eq!(read[0].day.to_string(), "2026-11-30");
assert_eq!(read[0].how, "2 working days after Wed, Nov 25");
```

## What it does with what it can't read

Nothing: a phrase that isn't a day reads as no days. So does one whose day
couldn't be written, as every day is in a year from 0001 to 9999. It
doesn't panic on anything typed, which `tests/typed.rs` holds it to.

## How it's built

A phrase is split into words once (`words`), parsed into a small tree that
says what was meant (`days`: a day, a span, a day picked out of a span, a
day moved by so much), and the tree is worked out for the day it is. A
repeat is a body and its tails (`repeat`), over rules that are worked out
by `rule`. An event's sentence (`event`) starts as all title, and one pass
after another takes the words it reads as something else.

With the `serde` feature, what's read can be written out and options read
in.

## Its cases

`tests/cases` has about 27,000 phrases and what each reads as, which is
what the library is held to. See its README.
