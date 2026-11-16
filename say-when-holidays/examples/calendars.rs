//! Writes each region's holidays over some years as JSON, a day and its
//! name to a pair, as the dates library's cases keep them
//! (`say-when/tests/cases/fixture.json`):
//!
//!     cargo run -p say-when-holidays --example calendars -- 2015 2045

use say_when_holidays::{Region, holidays};

fn main() {
    let mut years = std::env::args().skip(1).filter_map(|year| year.parse::<i32>().ok());
    let (from, to) = (years.next().unwrap_or(2015), years.next().unwrap_or(2045));
    let regions = [
        ("england-and-wales", Region::EnglandAndWales),
        ("scotland", Region::Scotland),
        ("northern-ireland", Region::NorthernIreland),
        ("united-states", Region::UnitedStates),
    ];
    let all: Vec<String> = regions
        .iter()
        .map(|(name, region)| {
            let days: Vec<String> = (from..=to)
                .flat_map(|year| holidays(year, *region))
                .map(|off| format!("[\"{}\",\"{}\"]", off.day, off.name.unwrap_or_default()))
                .collect();
            format!("\"{name}\":[{}]", days.join(","))
        })
        .collect();
    println!("{{{}}}", all.join(","));
}
