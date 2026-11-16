//! What a reader is told about where and for whom it's reading: where
//! (see [`crate::locale`]), and the days and time zones that have names
//! only the caller knows.

use crate::day::Weekday;
use crate::locale::Locale;
use crate::words::name_words;

/// Where a reader reads.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Options {
    /// Where English is being read and written: the order of a date in
    /// figures, the day a week starts on, how days are labelled, and the
    /// days that have names there.
    pub locale: Locale,
}

impl Options {
    /// For somewhere.
    pub fn new(locale: Locale) -> Options {
        Options { locale }
    }
}

/// A day that comes round every year and has a name only the caller knows,
/// like someone's birthday, for phrases to say: "mum's birthday", "a week
/// before Ann's birthday", "our anniversary next year".
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedDate {
    /// Each way of saying whose it is: "ann", "ann smith", "mum", "our". A
    /// name with anything but words in it, like a number, is never said.
    /// With none, the day is said by what it is alone, like "payday".
    pub names: Vec<String>,
    /// Each word for what it is: "birthday", "bday".
    pub what: Vec<String>,
    /// Its month, from 1 for January.
    pub month: u32,
    /// Its day of the month. The 29th of February is the 28th in a year
    /// without one.
    pub day: u32,
    /// How to say whose day it is, like "Eli Jones’s birthday", for what's
    /// read from it to say so: "2 weeks before Eli Jones’s birthday".
    #[cfg_attr(feature = "serde", serde(default))]
    pub label: Option<String>,
}

/// A named date as the grammar reads it: each name as its words.
#[derive(Clone, Debug)]
pub(crate) struct Named {
    /// Whether it's said by what it is alone, having been given no names,
    /// rather than names none of which a phrase could say.
    pub bare: bool,
    pub names: Vec<Vec<String>>,
    pub what: Vec<String>,
    pub month: u32,
    pub day: u32,
    pub label: Option<String>,
}

impl From<&NamedDate> for Named {
    fn from(date: &NamedDate) -> Named {
        Named {
            bare: date.names.is_empty(),
            names: date.names.iter().filter_map(|name| name_words(name)).collect(),
            what: date.what.iter().map(|word| word.to_lowercase()).collect(),
            month: date.month,
            day: date.day,
            label: date.label.clone(),
        }
    }
}

/// The names time zones go by, for an event's sentence to say which its
/// times are in. The reader knows no time zones itself: it's given the
/// names, and gives back whatever each stands for.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ZoneNames {
    /// Names read before the word "time", as in "9am New York time": each
    /// in lower case with single spaces, and the zone it names.
    pub before_word_time: Vec<(String, String)>,
    /// Names read straight after a time, as in "10:00 PT" or "at 3pm
    /// Tokyo". Fewer than the others, since many cities are people's
    /// names too.
    pub after_a_time: Vec<(String, String)>,
}

impl ZoneNames {
    pub(crate) fn find<'a>(names: &'a [(String, String)], said: &str) -> Option<&'a str> {
        let said = plain_name(said);
        names.iter().find(|(name, _)| *name == said).map(|(_, zone)| zone.as_str())
    }
}

/// A name as it's matched: in lower case, without accents on the letters
/// that commonly have them, and with what joins words as spaces.
fn plain_name(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        plain.push(match c {
            'à'..='å' => 'a',
            'ç' => 'c',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ñ' => 'n',
            'ò'..='ö' | 'ø' => 'o',
            'ù'..='ü' => 'u',
            '_' | '-' | '.' => ' ',
            other => other,
        });
    }
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The days of the week by their names, from Sunday, as they're matched.
pub(crate) const WEEKDAY_WORDS: [(&str, Weekday); 7] = [
    ("sunday", Weekday::Sunday),
    ("monday", Weekday::Monday),
    ("tuesday", Weekday::Tuesday),
    ("wednesday", Weekday::Wednesday),
    ("thursday", Weekday::Thursday),
    ("friday", Weekday::Friday),
    ("saturday", Weekday::Saturday),
];

/// The other ways days of the week are cut short.
pub(crate) fn weekday_alias(text: &str) -> Option<Weekday> {
    match text {
        "tues" => Some(Weekday::Tuesday),
        "weds" => Some(Weekday::Wednesday),
        "thur" | "thurs" => Some(Weekday::Thursday),
        _ => None,
    }
}

/// The months by their names in lower case, from January.
pub(crate) const MONTH_WORDS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_zone_by_any_spelling_of_its_name() {
        let names = vec![("sao paulo".to_string(), "America/Sao_Paulo".to_string())];
        assert_eq!(ZoneNames::find(&names, "São  Paulo"), Some("America/Sao_Paulo"));
        assert_eq!(ZoneNames::find(&names, "Paulo"), None);
    }
}
