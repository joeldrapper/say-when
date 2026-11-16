//! say-when as a WebAssembly module with nothing to import, for the npm
//! package (see `../src`, which wraps it in typed functions): a request is
//! written into its memory as JSON, and the answer read back out as JSON.
//!
//! What's asked with and answered is written as the library's `serde`
//! feature writes it, in snake case, so the library's own cases can be run
//! through it as they are.
//!
//! A reader is made once, with its calendar, named dates and time zones,
//! and asked by number after that, so what it reads with isn't sent again
//! on every key.

use say_when::{
    Calendar, Day, DayOff, EventContext, Locale, Mention, NamedDate, NamedDay, Offset, Options,
    Reader, Repeat, Rule, Time, ZoneNames, easter, read_clock,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;

/// What a request was asked with: its fields, read as what each should be.
///
/// A request is its name and these, like `{"days": {"reader": 0, "text":
/// "tomorrow", "today": "2026-10-20"}}`. They're read out of the JSON by
/// hand, a field at a time, rather than by the library's own
/// `Deserialize`, which would make the module much larger for what's only
/// ever a few fields.
#[derive(Clone, Copy)]
struct Asked<'a>(&'a Value);

type Result<T> = std::result::Result<T, String>;

impl<'a> Asked<'a> {
    fn field(self, name: &str) -> Result<&'a Value> {
        self.said(name).ok_or_else(|| format!("no {name}"))
    }

    /// A field that's there, and isn't null.
    fn said(self, name: &str) -> Option<&'a Value> {
        self.0.get(name).filter(|value| !value.is_null())
    }

    fn of(self, name: &str) -> Result<Asked<'a>> {
        self.field(name).map(Asked)
    }

    fn text(self, name: &str) -> Result<&'a str> {
        self.field(name)?.as_str().ok_or_else(|| format!("{name} isn't text"))
    }

    fn number(self, name: &str) -> Result<u32> {
        let number = self.field(name)?.as_u64().ok_or_else(|| format!("{name} isn't a number"))?;
        u32::try_from(number).map_err(|_| format!("{name} is too large"))
    }

    fn reader(self) -> Result<u64> {
        self.field("reader")?.as_u64().ok_or_else(|| "reader isn't a number".to_string())
    }

    fn day(self, name: &str) -> Result<Day> {
        Day::parse(self.text(name)?)
            .ok_or_else(|| format!("{name} is not a day written YYYY-MM-DD"))
    }

    fn maybe_day(self, name: &str) -> Result<Option<Day>> {
        self.said(name).map(|_| self.day(name)).transpose()
    }

    fn time(self, name: &str) -> Result<Time> {
        Time::parse(self.text(name)?).ok_or_else(|| format!("{name} is not a time written HH:MM"))
    }

    fn maybe_time(self, name: &str) -> Result<Option<Time>> {
        self.said(name).map(|_| self.time(name)).transpose()
    }

    fn maybe_text(self, name: &str) -> Option<String> {
        self.said(name).and_then(Value::as_str).map(str::to_string)
    }

    /// A list, or none where there's no such field.
    fn list(self, name: &str) -> &'a [Value] {
        self.said(name).and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }

    /// Each of a list, read as what it should be.
    fn each<T>(self, name: &str, read: impl Fn(Asked<'a>) -> Result<T>) -> Result<Vec<T>> {
        self.list(name).iter().map(|each| read(Asked(each))).collect()
    }

    /// A list of text.
    fn texts(self, name: &str) -> Result<Vec<String>> {
        let text = |each: &Value| each.as_str().map(str::to_string);
        self.list(name).iter().map(|each| text(each).ok_or(format!("{name} isn't text"))).collect()
    }

    /// This, as one of the library's small enums, like a locale or a day
    /// of the week, as `serde` reads it.
    fn one_of<T: DeserializeOwned>(self, what: &str) -> Result<T> {
        T::deserialize(self.0).map_err(|_| format!("no such {what}: {}", self.0))
    }
}

/// A calendar: its days off, its days with names, and the days of the week
/// that are worked.
fn calendar(asked: Asked) -> Result<Calendar> {
    let days_off = asked.each("days_off", |off| {
        Ok(DayOff::new(off.day("day")?, off.maybe_text("name").as_deref()))
    })?;
    let named_days = asked.each("named_days", |named| {
        let names = named.texts("names")?;
        Ok(match named.maybe_day("last")? {
            Some(last) => NamedDay::lasting(named.day("day")?, last, names),
            None => NamedDay::new(named.day("day")?, names),
        })
    })?;
    let mut calendar = Calendar { days_off, named_days, ..Calendar::default() };
    if asked.said("working_week").is_some() {
        calendar.working_week = asked.each("working_week", |day| day.one_of("day of the week"))?;
    }
    Ok(calendar)
}

/// A named date, like someone's birthday.
fn named_date(asked: Asked) -> Result<NamedDate> {
    Ok(NamedDate {
        names: asked.texts("names")?,
        what: asked.texts("what")?,
        month: asked.number("month")?,
        day: asked.number("day")?,
        label: asked.maybe_text("label"),
    })
}

/// The names time zones go by, each with the zone it names.
fn zones(asked: Asked) -> Result<ZoneNames> {
    let pairs = |name: &str| -> Result<Vec<(String, String)>> {
        let pair = |pair: &Value| {
            Some((pair.get(0)?.as_str()?.to_string(), pair.get(1)?.as_str()?.to_string()))
        };
        let each = |each: &Value| pair(each).ok_or(format!("{name} isn't pairs of names"));
        asked.list(name).iter().map(each).collect()
    };
    Ok(ZoneNames {
        before_word_time: pairs("before_word_time")?,
        after_a_time: pairs("after_a_time")?,
    })
}

/// What an event's sentence is read with.
fn context(asked: Asked) -> Result<EventContext> {
    let mut context = EventContext::new(asked.day("today")?, asked.time("fallback_time")?);
    if let Some(day) = asked.maybe_day("fallback_day")? {
        context.fallback_day = day;
    }
    if asked.said("default_length").is_some() {
        context.default_length = asked.number("default_length")?;
    }
    context.mentions = asked.each("mentions", |mention| {
        Ok(Mention::new(mention.text("written")?, mention.text("said")?))
    })?;
    Ok(context)
}

/// How something repeats.
fn repeat(asked: Asked) -> Result<Repeat> {
    let rule = Rule::parse(asked.text("rule")?)
        .ok_or("the rule isn't one whose days can be worked out")?;
    let starts = asked.day("starts")?;
    let mut repeat = match asked.text("kind")? {
        "schedule" => Repeat::schedule(rule, starts),
        "after" => Repeat::after_done(rule, starts),
        other => return Err(format!("no such kind of repeat: {other}")),
    };
    repeat.skipped = asked.each("skipped", |day| {
        day.0.as_str().and_then(Day::parse).ok_or("skipped isn't days".to_string())
    })?;
    Ok(repeat)
}

fn offset(asked: Asked) -> Result<Offset> {
    asked.text("offset")?.parse().map_err(|_| "offset isn't an offset".to_string())
}

/// A reader, as it's asked for: where it reads, its calendar of days off
/// and days with names, its named dates, and the names of time zones.
fn reader(asked: Asked) -> Result<(Reader, Locale)> {
    let locale = match asked.said("locale") {
        Some(locale) => Asked(locale).one_of("locale")?,
        None => Locale::default(),
    };
    let calendar = match asked.said("calendar") {
        Some(said) => calendar(Asked(said))?,
        None => Calendar::default(),
    };
    let named = asked.each("named_dates", named_date)?;
    let zones = match asked.said("zones") {
        Some(said) => zones(Asked(said))?,
        None => ZoneNames::default(),
    };
    let reader = Reader::new(Options::new(locale))
        .with_calendar(calendar)
        .with_named_dates(named)
        .with_zones(zones);
    Ok((reader, locale))
}

/// An answer, as JSON, written as the library writes it.
fn answer(value: impl Serialize) -> Result<String> {
    serde_json::to_string(&value).map_err(|error| error.to_string())
}

/// The readers made, by their numbers, each with where it reads. A number
/// is never given twice, so one kept after its reader was let go of can't
/// come to mean another.
#[derive(Default)]
pub struct Readers {
    made: BTreeMap<u64, (Reader, Locale)>,
    next: u64,
}

impl Readers {
    fn get(&self, asked: Asked) -> Result<&(Reader, Locale)> {
        let number = asked.reader()?;
        self.made.get(&number).ok_or_else(|| format!("no reader {number}"))
    }

    fn reader(&self, asked: Asked) -> Result<&Reader> {
        Ok(&self.get(asked)?.0)
    }

    /// Keeps a reader, and says its number.
    fn keep(&mut self, reader: (Reader, Locale)) -> u64 {
        let number = self.next;
        self.next += 1;
        self.made.insert(number, reader);
        number
    }

    /// Answers one request: what it's called, and what it's asked with.
    fn answer(&mut self, name: &str, asked: Asked) -> Result<String> {
        let text = || asked.text("text");
        let today = || asked.day("today");
        match name {
            "reader" => {
                let made = reader(asked)?;
                answer(self.keep(made))
            }
            "about" => {
                let (reader, locale) = self.get(asked)?;
                let about = (reader.about(text()?), *locale);
                answer(self.keep(about))
            }
            "drop" => {
                self.made.remove(&asked.reader()?);
                answer(())
            }
            "days" => answer(self.reader(asked)?.days(text()?, today()?)),
            "span" => answer(self.reader(asked)?.span(text()?, today()?)),
            "suggest_days" => answer(self.reader(asked)?.suggest_days(text()?, today()?)),
            "corrected" => {
                let more = asked.texts("more")?;
                let more: Vec<&str> = more.iter().map(String::as_str).collect();
                answer(self.reader(asked)?.corrected(text()?, &more))
            }
            "repeats" => answer(self.reader(asked)?.repeats(text()?, asked.day("from")?, today()?)),
            "suggest_repeats" => {
                answer(self.reader(asked)?.suggest_repeats(asked.day("from")?, today()?))
            }
            "describe_repeat" => {
                let repeat = repeat(asked.of("repeat")?)?;
                answer(self.reader(asked)?.describe_repeat(&repeat, asked.maybe_day("today")?))
            }
            "next_days" => {
                let repeat = repeat(asked.of("repeat")?)?;
                let many = asked.number("many")? as usize;
                answer(self.reader(asked)?.next_days(&repeat, asked.day("from")?, many))
            }
            "repeat_days" => {
                let repeat = repeat(asked.of("repeat")?)?;
                let reader = self.reader(asked)?;
                let days = reader.repeat_days(&repeat, asked.day("from")?, asked.day("to")?);
                let most = match asked.said("most") {
                    Some(_) => asked.number("most")? as usize,
                    None => usize::MAX,
                };
                answer(days.take(most).collect::<Vec<Day>>())
            }
            "repeats_on" => {
                let repeat = repeat(asked.of("repeat")?)?;
                answer(self.reader(asked)?.repeats_on(&repeat, asked.day("day")?))
            }
            "last_repeat_day" => {
                let repeat = repeat(asked.of("repeat")?)?;
                answer(self.reader(asked)?.last_repeat_day(&repeat))
            }
            "is_working_day" => answer(self.reader(asked)?.is_working_day(asked.day("day")?)),
            "offset" => answer(self.reader(asked)?.offset(text()?)),
            "offset_from" => {
                let offset = offset(asked)?;
                let reader = self.reader(asked)?;
                let time = asked.maybe_time("time")?;
                answer(reader.offset_from(&offset, asked.day("day")?, time))
            }
            "describe_offset" => answer(offset(asked)?.describe()),
            "event" => {
                let context = context(asked.of("context")?)?;
                answer(self.reader(asked)?.event(text()?, &context))
            }
            "when" => answer(self.reader(asked)?.when(text()?, today()?)),
            "label" => {
                let (_, locale) = self.get(asked)?;
                answer(locale.label(asked.day("day")?, today()?))
            }
            "long" => {
                let (_, locale) = self.get(asked)?;
                answer(locale.long(asked.day("day")?, asked.maybe_day("today")?))
            }
            "short" => {
                let (_, locale) = self.get(asked)?;
                answer(locale.short(asked.day("day")?, today()?))
            }
            "clock" => answer(read_clock(text()?, asked.maybe_time("after")?)),
            "easter" => {
                let year = asked.field("year")?.as_i64().ok_or("year isn't a number")?;
                answer(i32::try_from(year).ok().and_then(easter))
            }
            other => Err(format!("nothing's called {other}")),
        }
    }

    /// Answers a request written as JSON, as JSON: `{"ok": …}`, or
    /// `{"error": "…"}` for one that couldn't be answered.
    pub fn call(&mut self, request: &str) -> String {
        let answered = match serde_json::from_str::<Value>(request) {
            Ok(Value::Object(request)) if request.len() == 1 => {
                let (name, asked) = request.iter().next().expect("one field");
                self.answer(name, Asked(asked))
            }
            Ok(_) => Err("a request is one name and what it's asked with".to_string()),
            Err(error) => Err(error.to_string()),
        };
        match answered {
            Ok(answer) => format!(r#"{{"ok":{answer}}}"#),
            Err(error) => format!(r#"{{"error":{}}}"#, Value::String(error)),
        }
    }
}

/// The module as a page holds it: one request at a time, written into a
/// buffer the module keeps, and its answer read from another.
#[cfg(target_arch = "wasm32")]
mod module {
    use super::Readers;
    use core::cell::UnsafeCell;

    struct State {
        readers: Readers,
        request: Vec<u8>,
        answer: String,
    }

    /// WebAssembly here has one thread, and runs one thing at a time.
    struct Alone(UnsafeCell<Option<State>>);
    // SAFETY: WebAssembly built for wasm32-unknown-unknown has one thread.
    unsafe impl Sync for Alone {}
    static STATE: Alone = Alone(UnsafeCell::new(None));

    fn state() -> &'static mut State {
        // SAFETY: one thread, and no call here is made from inside another,
        // so no other reference to the state is alive.
        let state = unsafe { &mut *STATE.0.get() };
        state.get_or_insert_with(|| State {
            readers: Readers::default(),
            request: Vec::new(),
            answer: String::new(),
        })
    }

    /// Where to write a request of so many bytes.
    #[unsafe(no_mangle)]
    pub extern "C" fn request(bytes: usize) -> *mut u8 {
        let state = state();
        state.request.clear();
        state.request.resize(bytes, 0);
        state.request.as_mut_ptr()
    }

    /// Answers the request written, and says how many bytes the answer is.
    #[unsafe(no_mangle)]
    pub extern "C" fn call() -> usize {
        let state = state();
        state.answer = match core::str::from_utf8(&state.request) {
            Ok(request) => state.readers.call(request),
            Err(_) => r#"{"error":"the request wasn't UTF-8"}"#.to_string(),
        };
        state.answer.len()
    }

    /// Where the answer is, until the next request is written.
    #[unsafe(no_mangle)]
    pub extern "C" fn answer() -> *const u8 {
        state().answer.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_a_reader_and_asks_it() {
        let mut readers = Readers::default();
        assert_eq!(readers.call(r#"{"reader":{}}"#), r#"{"ok":0}"#);
        assert_eq!(
            readers.call(r#"{"days":{"reader":0,"text":"tomorrow","today":"2026-10-20"}}"#),
            r#"{"ok":[{"day":"2026-10-21","how":null,"about":null}]}"#
        );
        assert_eq!(readers.call(r#"{"clock":{"text":"half three"}}"#), r#"{"ok":"15:30"}"#);
        assert_eq!(readers.call(r#"{"drop":{"reader":0}}"#), r#"{"ok":null}"#);
        // A number isn't given again once its reader's let go of.
        assert_eq!(readers.call(r#"{"reader":{}}"#), r#"{"ok":1}"#);
        let gone = readers.call(r#"{"days":{"reader":0,"text":"x","today":"2026-10-20"}}"#);
        assert!(gone.contains("no reader 0"), "{gone}");
        assert!(readers.call("nonsense").starts_with(r#"{"error":"#));
        assert!(readers.call(r#"{"clock":{"text":"x","after":"25:00"}}"#).contains("after"));
        let bad = readers.call(r#"{"reader":{"calendar":{"days_off":[{"day":"2026-02-30"}]}}}"#);
        assert!(bad.contains("not a day"), "{bad}");
        let bad = readers.call(r#"{"reader":{"locale":"mars"}}"#);
        assert!(bad.contains("no such locale"), "{bad}");
    }

    #[test]
    fn reads_with_its_calendar_named_dates_and_zones() {
        let mut readers = Readers::default();
        let made = readers.call(
            r#"{"reader":{"locale":"us","calendar":{"days_off":[{"day":"2026-11-26","name":"Thanksgiving"}],
                "named_days":[{"day":"2027-03-28","names":["Easter Sunday","easter"]}],
                "working_week":["monday","tuesday","wednesday","thursday"]},
                "named_dates":[{"names":["mum"],"what":["birthday"],"month":3,"day":14,"label":"Mum’s birthday"}],
                "zones":{"before_word_time":[["new york","America/New_York"]],"after_a_time":[["pt","America/Los_Angeles"]]}}}"#,
        );
        assert_eq!(made, r#"{"ok":0}"#);
        let days =
            readers.call(r#"{"days":{"reader":0,"text":"mum's birthday","today":"2026-10-20"}}"#);
        assert_eq!(days, r#"{"ok":[{"day":"2027-03-14","how":null,"about":"Mum’s birthday"}]}"#);
        let easter = readers.call(r#"{"days":{"reader":0,"text":"easter","today":"2026-10-20"}}"#);
        assert!(easter.contains("2027-03-28"), "{easter}");
        let friday = readers.call(r#"{"is_working_day":{"reader":0,"day":"2026-11-20"}}"#);
        assert_eq!(friday, r#"{"ok":false}"#);
        let event = readers.call(
            r#"{"event":{"reader":0,"text":"Call 10:00 PT","context":{"today":"2026-10-20","fallback_time":"14:00"}}}"#,
        );
        assert!(event.contains(r#""time_zone":"America/Los_Angeles""#), "{event}");
    }

    #[test]
    fn works_out_the_days_a_repeat_comes_round_on() {
        let mut readers = Readers::default();
        readers.call(r#"{"reader":{}}"#);
        let repeat = r#"{"kind":"schedule","rule":"FREQ=WEEKLY;COUNT=3","starts":"2026-11-03","skipped":["2026-11-10"]}"#;
        let next = readers.call(&format!(
            r#"{{"next_days":{{"reader":0,"repeat":{repeat},"from":"2026-11-01","many":5}}}}"#
        ));
        assert_eq!(next, r#"{"ok":["2026-11-03","2026-11-17"]}"#);
        let last =
            readers.call(&format!(r#"{{"last_repeat_day":{{"reader":0,"repeat":{repeat}}}}}"#));
        assert_eq!(last, r#"{"ok":"2026-11-17"}"#);
        let said = readers.call(&format!(
            r#"{{"describe_repeat":{{"reader":0,"repeat":{repeat},"today":"2026-11-01"}}}}"#
        ));
        assert_eq!(said, r#"{"ok":"Every Tuesday, 3 times"}"#);
        let most = readers.call(
            r#"{"repeat_days":{"reader":0,"repeat":{"kind":"schedule","rule":"FREQ=DAILY","starts":"2026-01-01"},"from":"2026-01-01","to":"9999-12-31","most":2}}"#,
        );
        assert_eq!(most, r#"{"ok":["2026-01-01","2026-01-02"]}"#);
    }

    #[test]
    fn lands_an_offset() {
        let mut readers = Readers::default();
        readers.call(r#"{"reader":{}}"#);
        let offset = readers.call(r#"{"offset":{"reader":0,"text":"the day before at 9am"}}"#);
        assert_eq!(offset, r#"{"ok":"-P1D@09:00"}"#);
        let landed = readers.call(
            r#"{"offset_from":{"reader":0,"offset":"-P1D@09:00","day":"2026-12-03","time":"14:00"}}"#,
        );
        assert_eq!(landed, r#"{"ok":["2026-12-02","09:00"]}"#);
        let said = readers.call(r#"{"describe_offset":{"offset":"-P1D@09:00"}}"#);
        assert_eq!(said, r#"{"ok":"the day before at 9am"}"#);
    }
}
