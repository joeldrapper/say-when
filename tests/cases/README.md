# say-when's cases

Each `.jsonl` file is a case to a line: what's typed, the day it's read on,
and what it should read as (`out`). `fixture.json` has what every case is
read with: some named dates, the names of some time zones, each region's
public holidays, and the time an event is at when nothing says.

| File            | What's read                                          |
| --------------- | ---------------------------------------------------- |
| `days.jsonl`    | `Reader::days` and `Reader::span`, on `today`        |
| `search.jsonl`  | `Reader::suggest_days` and `Reader::corrected`       |
| `repeats.jsonl` | `Reader::repeats`, for something on the day `from`   |
| `events.jsonl`  | `Reader::event`, with `mentions` when it has any     |
| `clocks.jsonl`  | `read_clock`, after the time `after` when it has one |

A case is read for its `locale`, or the UK, with the public holidays of
its `region` as its days off, from the fixture's `calendars`: the bank
holidays of England and Wales, Scotland and Northern Ireland, and the
federal holidays of the United States, for 2015 to 2045. A UK case with
no region has England and Wales's, and a case for anywhere else with no
region has none. Its named days are its locale's, from the fixture's
`named_days`: the days said there that aren't built in, like Easter,
Mother's Day and Thanksgiving, each with every day it falls on in the
same years. The cases with a locale didn't come from the TypeScript, which
read British English only: each was read through by hand when it was
added.

A search case with an `about` is read by `Reader::about` of it, so "his
birthday" is whoever it names.

They're plain JSON, so a program that reaches the library some other way,
through WebAssembly or another language's bindings, can run the same files
and agree with it on every case.

## Where they came from

They began as what an earlier reader, written in TypeScript, made of every
phrase its own tests read, of phrases built from the grammar piece by
piece, of each as it's half typed, and of inputs that used to go wrong.
That reader has since been retired, and the cases are this library's own:
change one when the library should read something differently, and say
why in the commit.

Where the library reads a phrase differently from the TypeScript on
purpose, the case has a `note`:

| Note              | What changed, and why                                                                                                                                                                                                                                                         |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `range`           | A day outside the years 0001 to 9999, or a count too large to work out, reads as nothing. The TypeScript gave days that can't be written, like `+011026-10-20`, threw, or took seconds.                                                                                       |
| `ordinal-wording` | Counting from the end is worded one way, as repeats always were: "the 2nd to last Friday", "the 3rd to last Saturday". The TypeScript said "second to last" for two and "the -3th Saturday" for three.                                                                        |
| `this-last-month` | "this november" is this year's November and "last november" the latest one before this month. The TypeScript read neither.                                                                                                                                                    |
| `last-weekend`    | "last weekend" is the weekend before this one. The TypeScript gave this weekend.                                                                                                                                                                                              |
| `leap-day`        | "29 feb" is the next 29 February there is, and "every 29 feb" repeats on it. The TypeScript read neither outside a leap year.                                                                                                                                                 |
| `valentines`      | "valentines" is read without its apostrophe.                                                                                                                                                                                                                                  |
| `every-second`    | "every second week" is every other week.                                                                                                                                                                                                                                      |
| `may-day`         | "may day" is the 1st of May, the day itself. The TypeScript gave the early May bank holiday, which is the calendar's to name now: "the may bank holiday".                                                                                                                     |
| `easter-monday`   | "easter monday" is the day after Easter everywhere, whether or not it's a day off there. The TypeScript read nothing in Scotland, where it isn't.                                                                                                                             |
| `calendar-ends`   | Working days are counted by the calendar given, which here has the years 2015 to 2045: past them, every Monday to Friday counts. The TypeScript worked bank holidays out for any year.                                                                                        |
| `last-day`        | "last day", with nothing after it, is the last day of every month, as "the last day of the month" is.                                                                                                                                                                         |
| `next-week`       | In an event, "next week" and "this week" are days wherever they're said, and the event is over the week: "Review next week at 2". Before, only at the end of the sentence, as "next" alone was taken for a word that says nothing.                                            |
| `range-search`    | Each side of a range of days is read as a day and maybe a time, and nothing else. Before, each was read as a whole event, which swallowed words after it, like "from 1" and "for 45 mins", and lost the range: "fri to sun from 1 to 3" is now Friday to Sunday, from 1 to 3. |
| `weekday-near`    | A day of the week so long from now is the one nearest then: "wednesday in 2 weeks time", "tuesday, 2 weeks from now", "monday in 3 months". Before, no day was read.                                                                                                          |
