# say-when

Reads days, spans of days, times and how things repeat, written in plain
English, the way people type them into a date picker or a calendar:
"tomorrow", "next fri", "the last working day of november", "3 working
days before christmas", "every other thursday until christmas", "the day
before at 9am", "Lunch with Priya tomorrow at 1 at Monmouth".

This is the [say-when](https://crates.io/crates/say-when) Rust library,
compiled to WebAssembly, with a typed TypeScript API. It reads exactly as
the crate does: the crate's 27,000 cases are run through this package too.

```sh
npm install say-when
```

```ts
import { Reader } from 'say-when';

const reader = new Reader({ locale: 'uk' });

reader.days('the day before the last working day of november', '2026-10-20');
// [{ day: '2026-11-29', how: 'the day before Mon 30 Nov, the last working day of November 2026', about: null }]

const [repeat] = reader.repeats('every other thursday until christmas', '2026-10-20', '2026-10-20');
repeat.repeat.rule; // 'FREQ=WEEKLY;INTERVAL=2;BYDAY=TH;UNTIL=20261225'
repeat.how; // 'Every 2 weeks on Thursday, until 25 December'
repeat.next; // ['2026-10-22', '2026-11-05', '2026-11-19', '2026-12-03']

reader.when('fri 6pm to sun 5pm', '2026-11-16');
// { startDay: '2026-11-20', startTime: '18:00', endDay: '2026-11-22', endTime: '17:00', … }
```

It works anywhere JavaScript and WebAssembly do: Node 20.16 and later,
Deno, Bun, browsers and workers. It has no dependencies.

## What it reads

- **Days**: `reader.days(text, today)` gives every day a phrase might mean,
  the likeliest first, each with how it was read. `reader.span(text,
  today)` gives the span a phrase names, like "next week", "the weekend",
  "q3 2027" or "november".
- **A date picker's search**, as it's typed: `reader.suggestDays(typed,
  today)`, where "t" is Today, Tomorrow, Tuesday and Thursday.
  `reader.corrected(typed)` puts right a word that's a letter off, like
  "tmorrow", for searching again when nothing was found.
- **How something repeats**, as an iCalendar RRULE: `reader.repeats(text,
  from, today)`. Then `nextDays`, `repeatDays`, `repeatsOn`,
  `lastRepeatDay` and `describeRepeat` work out and say the days it comes
  round on.
- **Where something is from a day**, like a reminder from its event:
  `reader.offset('the day before at 9am')` is `'-P1D@09:00'`, which keeps
  what was said rather than the day, so it follows its event when the
  event moves. `reader.offsetFrom(offset, day, time)` lands it.
- **An event typed as a sentence**: `reader.event(text, { today,
  fallbackTime })` reads its title, when it is, how it repeats, where it
  is, who's mentioned and its time zone, with what each word was read as.
- **When something is, and nothing else**: `reader.when(text, today)`, or
  `null` if anything typed isn't.
- **A time of day**: `readClock('half three')` is `'15:30'`.

```ts
const text = 'Lunch tomorrow 1-2pm at Dishoom';
const event = reader.event(text, { today: '2026-11-16', fallbackTime: '09:00' });
event.title; // 'Lunch'
event.words.map((word) => [text.slice(word.start, word.end), word.role]);
// [['Lunch', 'title'], ['tomorrow', 'day'], ['1-2pm', 'time'], ['at', 'place'], ['Dishoom', 'place']]
```

Where each word is, is where it is in the string, so `text.slice(start,
end)` is the word, whatever's in it.

## What it's told

It has no clock, no time zones, no address book and no country's public
holidays. You say:

- **the day it is**, with every reading;
- **where it's reading**, as `locale`: `'uk'`, `'us'`, `'ireland'`,
  `'australia'`, `'new-zealand'` or `'canada'`. That decides the order of
  a date in figures ("3/4" is 3 April in the UK and 4 March in the US), the
  day a week starts on, and how a day is labelled;
- **its calendar**: the days that are off, like public holidays, which
  decide what a working day is and can be said ("the august bank
  holiday"); the days that have names beyond the few built in, like
  Easter, Mother's Day or Ramadan; and the days of the week that are
  worked. Public holidays can't be worked out by rule, as governments add
  and move them, so give it a list kept from a source you trust;
- **named dates**, for days only you know by name, like people's
  birthdays;
- **the names time zones go by**, for an event's sentence to say which its
  times are in.

```ts
import { easter, Reader } from 'say-when';

const reader = new Reader({
	locale: 'us',
	calendar: {
		daysOff: [{ day: '2026-11-26', name: 'Thanksgiving' }],
		namedDays: [{ day: easter(2027)!, names: ['Easter Sunday', 'Easter'] }]
	},
	namedDates: [{ names: ['mum'], what: ['birthday'], month: 3, day: 14, label: 'Mum’s birthday' }],
	zones: { afterATime: { pt: 'America/Los_Angeles' } }
});

reader.days('2 working days after 11/25', '2026-10-20')[0].day; // '2026-11-30'
reader.days("a week before mum's birthday", '2026-10-20')[0].day; // '2027-03-07'
reader.event('Call 10:00 PT', { today: '2026-10-20', fallbackTime: '09:00' }).timeZone; // 'America/Los_Angeles'
```

Make a reader once and keep it, rather than one a phrase: making one works
out what it's told.

## Days and times

Days are given back written `YYYY-MM-DD`, and times `HH:MM` on the
24-hour clock, as they're in no time zone. They can be given as text like
that, or as anything with a `year`, `month` and `day`, like a
`Temporal.PlainDate`, or with an `hour` and `minute`, like a
`Temporal.PlainTime`, or as a `Date`, which is the day it is where the
code runs.

## Loading the WebAssembly

From `say-when`, the module is in the package's code, and compiled the
first time it's needed, so there's nothing to set up and nothing to
await. To compile it ahead, off the main thread where a browser can, call
`await init()` first.

From `say-when/slim`, it isn't, which saves about a quarter of what's
downloaded, and lets a browser compile it as it arrives and keep it
cached as its own file. Call `await init()` before reading anything: it
loads `say-when.wasm` from beside the package's code, which bundlers that
follow `new URL(…, import.meta.url)`, like Vite, webpack and Parcel, copy
over. Or tell it where the module is, as a URL, a `Response`, its bytes or
a compiled `WebAssembly.Module`:

```ts
import wasm from 'say-when/say-when.wasm?url';
import { init, Reader } from 'say-when/slim';

await init(wasm);
```

Either way, a page with a Content Security Policy needs `'wasm-unsafe-eval'`
in its `script-src`.

## Licence

MIT.
