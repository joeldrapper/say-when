//! Which words an event's sentence, or when something is, reads as what.

use say_when::{Day, EventContext, Mention, Options, Reader, Time, WordRole};

fn roles(text: &str, mentions: Vec<Mention>) -> (String, Vec<(&str, WordRole)>) {
    let reader = Reader::new(Options::default());
    let today = Day::new(2026, 11, 16).unwrap();
    let context = EventContext { mentions, ..EventContext::new(today, Time::new(9, 0).unwrap()) };
    let event = reader.event(text, &context);
    let words = event.words.iter().map(|word| (&text[word.start..word.end], word.role)).collect();
    (event.title, words)
}

#[test]
fn a_mention_is_one_word_however_many_it_is_written_in() {
    let mentions = vec![Mention::new("#Priya Raman", "Priya")];
    let (title, words) = roles("Café with  #Priya Raman, tomorrow", mentions);
    assert_eq!(title, "Café with Priya");
    assert_eq!(
        words,
        [
            ("Café", WordRole::Title),
            ("with", WordRole::Title),
            ("#Priya Raman,", WordRole::Mention(0)),
            ("tomorrow", WordRole::Day),
        ]
    );
}

#[test]
fn what_leads_to_a_place_goes_with_it() {
    let (title, words) = roles("Dinner on at Dishoom", Vec::new());
    assert_eq!(title, "Dinner");
    assert_eq!(
        words,
        [
            ("Dinner", WordRole::Title),
            ("on", WordRole::Place),
            ("at", WordRole::Place),
            ("Dishoom", WordRole::Place),
        ]
    );
}

#[test]
fn says_each_part_of_when() {
    let (title, words) = roles("Standup every weekday at 9:30 for 15 mins !", Vec::new());
    assert_eq!(title, "Standup");
    let role = |said: &str| words.iter().find(|(word, _)| *word == said).unwrap().1;
    assert_eq!(role("every"), WordRole::Repeat);
    assert_eq!(role("weekday"), WordRole::Repeat);
    assert_eq!(role("9:30"), WordRole::Time);
    assert_eq!(role("15"), WordRole::Length);
    assert_eq!(role("mins"), WordRole::Length);
    assert_eq!(role("!"), WordRole::Length);
}

#[test]
fn the_times_of_a_range_are_times() {
    let (_, words) = roles("fri 6 nov 9 to 5", Vec::new());
    assert_eq!(
        words,
        [
            ("fri", WordRole::Day),
            ("6", WordRole::Day),
            ("nov", WordRole::Day),
            ("9", WordRole::Time),
            ("to", WordRole::Time),
            ("5", WordRole::Time),
        ]
    );
}
