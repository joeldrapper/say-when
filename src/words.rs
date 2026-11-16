//! Splits what's typed into the words every reader reads: numbers, however
//! they're written ("3", "three", "twenty one", "a"), ordinals ("3rd",
//! "third"), and everything else as it is, in lower case.
//!
//! Words that can always be left out ("the", "of", "on", "time") are
//! dropped here, so no rule has to allow for them.

/// One word of a phrase.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Token {
    /// A number, like "3", "three" or "a", with the text it was read from.
    Number(u64, String),
    /// An ordinal, like "3rd" or "third".
    Ordinal(u64),
    /// Any other word, in lower case, or a symbol, like "+".
    Word(String),
}

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 8] =
    ["twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
const ORDINAL_ONES: [&str; 19] = [
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];

/// Words that can always be left out.
const FILLER: [&str; 5] = ["the", "of", "time", "times", "on"];

fn position(list: &[&str], word: &str) -> Option<u64> {
    list.iter().position(|each| *each == word).map(|index| index as u64)
}

/// A number written as a word, like "seven", "twenty" or "a".
pub(crate) fn number_word(word: &str) -> Option<u64> {
    position(&ONES, word)
        .or_else(|| position(&TENS, word).map(|index| (index + 2) * 10))
        .or_else(|| matches!(word, "a" | "an").then_some(1))
}

fn ordinal_word(word: &str) -> Option<u64> {
    position(&ORDINAL_ONES, word).map(|index| index + 1).or(match word {
        "twentieth" => Some(20),
        "thirtieth" => Some(30),
        _ => None,
    })
}

/// The number an ordinal word counts, written the same way: "four" for
/// "fourth", "4" for "4th", and "twenty" for "twentieth", so "twenty
/// fourth" can be "twenty four".
pub(crate) fn cardinal(word: &str) -> Option<String> {
    if let Some(n) = ordinal_figures(word) {
        return Some(n.to_string());
    }
    let n = ordinal_word(word)?;
    let at = usize::try_from(n).ok()?;
    ONES.get(at).or_else(|| TENS.get((at / 10).checked_sub(2)?)).map(|word| word.to_string())
}

/// A number written in figures, like "15", small enough to count with.
fn figures(word: &str) -> Option<u64> {
    (!word.is_empty() && word.len() <= 15 && word.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

/// An ordinal written in figures, like "3rd" or "22nd".
fn ordinal_figures(word: &str) -> Option<u64> {
    let digits = word.len().checked_sub(2)?;
    let (number, suffix) = word.split_at_checked(digits)?;
    matches!(suffix, "st" | "nd" | "rd" | "th").then(|| figures(number)).flatten()
}

/// What's typed, in lower case, with what joins words taken apart:
/// possessives lose their "'s", commas and apostrophes are spaces, a "+"
/// stands alone, "3d" is "3 d" (though "3rd" is left), and a hyphen splits
/// words unless it's between figures, as in "2027-03-01".
fn spaced(text: &str) -> String {
    let lower: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let letter = |at: usize| lower.get(at).is_some_and(|c| c.is_ascii_lowercase());
    let digit = |at: usize| lower.get(at).is_some_and(|c| c.is_ascii_digit());
    let word = |at: usize| lower.get(at).is_some_and(|c| c.is_alphanumeric() || *c == '_');
    let mut out = String::with_capacity(lower.len() + 8);
    let mut at = 0;
    while at < lower.len() {
        let c = lower[at];
        match c {
            '\'' | '’' if lower.get(at + 1) == Some(&'s') && !word(at + 2) => at += 1,
            '\'' | '’' | ',' => out.push(' '),
            '+' => out.push_str(" + "),
            '-' if !(at > 0 && digit(at - 1) && digit(at + 1)) => out.push(' '),
            _ => {
                out.push(c);
                // "3d" and "10days", but not "3rd" or "2nd".
                if c.is_ascii_digit() && letter(at + 1) {
                    let suffix: String = lower[at + 1..].iter().take(2).collect();
                    let ordinal =
                        matches!(suffix.as_str(), "st" | "nd" | "rd" | "th") && !word(at + 3);
                    if !ordinal {
                        out.push(' ');
                    }
                }
            }
        }
        at += 1;
    }
    out
}

/// The words of a phrase.
pub(crate) fn tokens(text: &str) -> Vec<Token> {
    tokens_of(&[text]).into_iter().map(|(token, _)| token).collect()
}

/// The words of some pieces of text read one after another, each with
/// which piece it started in, for whoever reads a sentence a word at a
/// time and needs to know which of its words a reading took.
pub(crate) fn tokens_of(pieces: &[&str]) -> Vec<(Token, usize)> {
    let spaced: Vec<String> = pieces.iter().map(|piece| spaced(piece)).collect();
    let words: Vec<(&str, usize)> = spaced
        .iter()
        .enumerate()
        .flat_map(|(piece, text)| text.split_whitespace().map(move |word| (word, piece)))
        .collect();
    let mut found: Vec<(Token, usize)> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        let (word, piece) = words[at];
        let next = words.get(at + 1).map(|(word, _)| *word);
        at += 1;
        if FILLER.contains(&word) {
            continue;
        }
        let token = if matches!(word, "couple" | "pair") {
            // "a couple of days": the "a" was taken for a number already.
            if matches!(found.last(), Some((Token::Number(_, text), _)) if text == "a" || text == "an")
            {
                found.pop();
            }
            Token::Number(2, word.to_string())
        } else if let Some(n) = figures(word) {
            Token::Number(n, word.to_string())
        } else if let Some(n) = ordinal_figures(word) {
            Token::Ordinal(n)
        } else if let Some(tens) = position(&TENS, word).map(|index| (index + 2) * 10) {
            // "twenty one" and "twenty first".
            let ones = next.and_then(|next| position(&ONES, next)).filter(|n| (1..10).contains(n));
            let ordinal = next
                .and_then(|next| position(&ORDINAL_ONES, next))
                .map(|index| index + 1)
                .filter(|n| (1..10).contains(n));
            if let Some(ones) = ones {
                at += 1;
                Token::Number(tens + ones, format!("{word} {}", next.unwrap_or("")))
            } else if let Some(ones) = ordinal {
                at += 1;
                Token::Ordinal(tens + ones)
            } else {
                Token::Number(tens, word.to_string())
            }
        } else if let Some(n) = number_word(word) {
            Token::Number(n, word.to_string())
        } else if let Some(n) = ordinal_word(word) {
            Token::Ordinal(n)
        } else {
            Token::Word(word.to_string())
        };
        found.push((token, piece));
    }
    found
}

/// A name as the words a phrase would have, or `None` if it has anything a
/// phrase's name can't, like a number.
pub(crate) fn name_words(name: &str) -> Option<Vec<String>> {
    let found = tokens(name);
    if found.is_empty() {
        return None;
    }
    found
        .into_iter()
        .map(|token| match token {
            Token::Word(word) => Some(word),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str) -> String {
        tokens(text)
            .iter()
            .map(|token| match token {
                Token::Number(n, _) => format!("#{n}"),
                Token::Ordinal(n) => format!("{n}th"),
                Token::Word(word) => word.clone(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn reads_numbers_however_they_are_written() {
        assert_eq!(said("3 three twenty-one twenty one a an"), "#3 #3 #21 #21 #1 #1");
        assert_eq!(said("3rd third twenty-first 22nd thirtieth"), "3th 3th 21th 22th 30th");
        assert_eq!(said("a couple of days"), "#2 days");
        assert_eq!(said("3d 10days 2nd"), "#3 d #10 days 2th");
    }

    #[test]
    fn drops_what_can_be_left_out_and_takes_words_apart() {
        assert_eq!(
            said("The day before Mum's birthday, on the 3rd"),
            "day before mum birthday 3th"
        );
        assert_eq!(said("in two years' time"), "in #2 years");
        assert_eq!(said("christmas+3d"), "christmas + #3 d");
        assert_eq!(said("2027-03-01 mon-fri"), "2027-03-01 mon fri");
    }

    #[test]
    fn counts_what_an_ordinal_counts() {
        assert_eq!(cardinal("fourth").as_deref(), Some("four"));
        assert_eq!(cardinal("4th").as_deref(), Some("4"));
        assert_eq!(cardinal("twelfth").as_deref(), Some("twelve"));
        assert_eq!(cardinal("twentieth").as_deref(), Some("twenty"));
        assert_eq!(cardinal("thirtieth").as_deref(), Some("thirty"));
        assert_eq!(cardinal("four"), None);
    }

    #[test]
    fn leaves_numbers_too_long_to_count_with_as_words() {
        assert_eq!(said("999999999999999999999"), "999999999999999999999");
    }

    #[test]
    fn a_name_is_words_alone() {
        assert_eq!(name_words("Ann Smith"), Some(vec!["ann".to_string(), "smith".to_string()]));
        assert_eq!(name_words("Agent 47"), None);
        assert_eq!(name_words(""), None);
    }
}
