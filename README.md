# say-when

Reads days, spans of days, times and how things repeat, written out in
plain English, the way people type them into a date picker or a calendar:

- **a day**: "tomorrow", "next fri", "9 dec", "the last working day of
  november", "3 working days before christmas", "a fortnight after next
  friday", "mum's birthday"
- **a span of days**: "next week", "the weekend", "november", "q3 2027",
  "the weekend before mother's day", "ramadan"
- **a search, as it's typed**: "t" is Today, Tomorrow, Tuesday and Thursday
- **a time of day**: "1pm", "13:30", "half three", "quarter to 5", "noon"
- **how something repeats**, as an iCalendar RRULE: "every other thursday",
  "the last working day of the month", "weekly until christmas", "the
  weekend before each mother's day"
- **where something is from a day**, like a reminder from its event: "3
  days before", "the friday before", "the day before at 9am", "an hour
  before"
- **when something is, and nothing else**: "tomorrow 2-4pm", "fri 6pm to
  sun 5pm", "3pm for an hour", "next month"
- **an event typed as a sentence**: "Lunch with Priya tomorrow at 1 at
  Monmouth", "Standup every weekday at 9:30 for 15 mins", with which words
  were read as what, for showing them as they're typed

The last word can be half typed, numbers can be words, and a phrase that
could mean more than one day gives each, the likeliest first.

```rust
use say_when::{Day, Options, Reader};

let reader = Reader::new(Options::default());
let today = Day::new(2026, 10, 20).unwrap();

let read = reader.days("the day before the last working day of november", today);
assert_eq!(read[0].day.to_string(), "2026-11-29");
assert_eq!(read[0].how.as_deref(), Some("the day before Mon 30 Nov, the last working day of November 2026"));

let repeats = reader.repeats("every other thursday until christmas", today, today);
assert_eq!(repeats[0].repeat.rule.to_string(), "FREQ=WEEKLY;INTERVAL=2;BYDAY=TH;UNTIL=20261225");
assert_eq!(repeats[0].how, "Every 2 weeks on Thursday, until 25 December");
```

A reminder keeps what was said, not the day it lands on, so it follows
its event when the event moves:

```rust
use say_when::{Day, Options, Reader, Time};

let reader = Reader::new(Options::default());
let offset = reader.offset("the day before at 9am").unwrap();
assert_eq!(offset.to_string(), "-P1D@09:00");

let event = Day::new(2026, 12, 3).unwrap();
let (day, time) = reader.offset_from(&offset, event, Time::new(14, 0)).unwrap();
assert_eq!((day.to_string(), time), ("2026-12-02".to_string(), Time::new(9, 0)));
```

An event's sentence says which word was read as what, and when it is
reads the same way on its own, as for a search or finding a free slot:

```rust
use say_when::{Day, EventContext, Options, Reader, Time, WordRole};

let reader = Reader::new(Options::default());
let today = Day::new(2026, 11, 16).unwrap();

let text = "Lunch tomorrow 1-2pm at Dishoom";
let event = reader.event(text, &EventContext::new(today, Time::new(9, 0).unwrap()));
assert_eq!(event.title, "Lunch");
let said: Vec<(&str, WordRole)> =
    event.words.iter().map(|word| (&text[word.start..word.end], word.role)).collect();
assert_eq!(said, [
    ("Lunch", WordRole::Title),
    ("tomorrow", WordRole::Day),
    ("1-2pm", WordRole::Time),
    ("at", WordRole::Place),
    ("Dishoom", WordRole::Place),
]);

let when = reader.when("fri 6pm to sun 5pm", today).unwrap();
assert_eq!((when.start_day.to_string(), when.start_time), ("2026-11-20".to_string(), Time::new(18, 0)));
assert_eq!((when.end_day.to_string(), when.end_time), ("2026-11-22".to_string(), Time::new(17, 0)));
assert!(reader.when("lunch tomorrow", today).is_none());
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
  "Thu, Nov 30, 2028");
- **its calendar** (`Reader::with_calendar`): the days that are off, like
  public holidays, the days that have names, and the days of the week
  that are worked. The days off decide what a working day is, and their
  names can be said: "the august bank holiday", "the next public
  holiday". With none, a working day is Monday to Friday. Public
  holidays can't be worked out by rule, as governments add and move
  them, so the library has none of its own: give it a list kept from a
  source you trust. The days with names can be said too: "easter
  monday", "a week before mother's day", "thanksgiving 2027", by any of
  their names or the others they go by everywhere ("mother's day" for
  Mothering Sunday). One can last more than a day, like Ramadan or half
  term, and what's after it counts from its last: "a week after
  ramadan". Only the few on the same day every year and said everywhere
  English is are built in (Christmas, New Year, Valentine's Day, St
  Patrick's Day, Halloween and Guy Fawkes Night), as which others are
  said depends on where, and many move from year to year. `easter` works
  out Easter Sunday, for the days that hang on it;
- **named dates**, for days only it knows by name, like people's birthdays
  (`Reader::with_named_dates`);
- **the names time zones go by**, for an event's sentence to say which its
  times are in (`Reader::with_zones`). What each name stands for is given
  back as it was given.

```rust
use say_when::{Calendar, Day, DayOff, Locale, NamedDay, Options, Reader, easter};

let calendar = Calendar {
    days_off: vec![DayOff::new(Day::new(2026, 11, 26).unwrap(), Some("Thanksgiving"))],
    named_days: vec![NamedDay::new(easter(2027).unwrap(), ["Easter Sunday", "Easter"])],
    ..Calendar::default()
};
let reader = Reader::new(Options::new(Locale::Us)).with_calendar(calendar);
let today = Day::new(2026, 10, 20).unwrap();

let read = reader.days("2 working days after 11/25", today);
assert_eq!(read[0].day.to_string(), "2026-11-30");
assert_eq!(read[0].how.as_deref(), Some("2 working days after Wed, Nov 25"));

let read = reader.days("a week before easter", today);
assert_eq!(read[0].day.to_string(), "2027-03-21");
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
after another takes the words it reads as something else. When something
is, read on its own, goes through the same passes, leaving out the ones
for how it repeats and where it is, and is nothing if any word is left.

With the `serde` feature, what's read can be written out and options read
in.

## Its cases

`tests/cases` has about 27,000 phrases and what each reads as, which is
what the library is held to. See its README. To change what something
reads as, change the library, run
`BLESS=1 cargo test --features serde --test cases`, and read the diff of
the cases before committing it: that diff is the change in behaviour.

`tests/properties.rs` holds what's true whatever the rule, calendar or
phrase is, with thousands of each made at random by proptest: a rule reads
back as it's written, a series comes round on the day it starts, its days
are in order and the same however a window is split, it's on each of them
and no others, and so many working days on is a working day so many away.

## From JavaScript and TypeScript

The npm package [`say-when`](https://www.npmjs.com/package/say-when) ([source](https://github.com/joeldrapper/say-when/tree/main/js)) is this library compiled to
WebAssembly, with a typed API in TypeScript, and it reads the same: the
cases are run through it too.

```ts
import { Reader } from 'say-when';

new Reader({ locale: 'uk' }).days('next friday', '2026-11-16')[0].day; // '2026-11-27'
```

## Working on it

`mise install` sets up the Rust, Node and aube it's built with, and `mise
run` lists the tasks: `mise run test` tests the crate and the npm
package, and `mise run lint` checks both.

## Licence

MIT. See `LICENSE`.
