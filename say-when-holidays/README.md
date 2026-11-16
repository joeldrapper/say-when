# say-when-holidays

Public holidays, worked out by rule, as calendars for
[say-when](https://crates.io/crates/say-when), which knows no country's
holidays of its own.

It has the bank holidays of England and Wales, Scotland and Northern
Ireland, as gov.uk lists them, and the federal holidays of the United
States, each on the day it's observed. One-off days, like a coronation,
are listed by hand, so the years ahead are only as right as the rules
are now. A new holiday is a release of this crate and nothing else, and a
caller with a better source needn't use it at all.

```rust
use say_when::{Day, Options, Reader};
use say_when_holidays::{Region, calendar};

let reader = Reader::new(Options::default())
    .with_calendar(calendar(Region::EnglandAndWales, 2025..=2035));
let today = Day::new(2026, 10, 20).unwrap();

let read = reader.days("3 working days before christmas", today);
assert_eq!(read[0].day.to_string(), "2026-12-22");
```

`cargo run -p say-when-holidays --example calendars -- 2015 2045` writes
each region's holidays over those years as JSON.

## Licence

MIT.
