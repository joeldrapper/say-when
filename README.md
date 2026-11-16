# say-when

You say when, and it works out the day.

Two Rust crates:

- [**say-when**](say-when) reads days, spans of days, times and how things
  repeat, written in plain English the way people type them into a date
  picker or a calendar: "next fri", "the last working day of november",
  "half three", "every other thursday until christmas", "Lunch with Priya
  tomorrow at 1 at Monmouth".
- [**say-when-holidays**](say-when-holidays) works out public holidays by
  rule, as calendars for say-when to count working days by.

Each has a version of its own. See each crate's README for how to use it.

## Developing

```sh
cargo test --workspace --all-features
cargo run -p say-when --example read -- "next friday"
cargo run -p say-when --release --example bench --features serde
```

What say-when is held to is its cases, in `say-when/tests/cases`: about
27,000 phrases, each with what it reads as. To change what something reads
as, change the library, run
`BLESS=1 cargo test -p say-when --features serde --test cases`, and read
the diff of the cases before committing it. That diff is the change in
behaviour.

## Licence

MIT. See [LICENSE](LICENSE).
