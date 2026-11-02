//! Prints Bazaar DB's shop odds.
//!
//!   cargo run -p bazaardb --example shop_odds                     # ask the API (refused without a browser token)
//!   cargo run -p bazaardb --example shop_odds -- shop-odds.json   # read an answer saved from a browser
//!   ... -- --json                                                 # print JSON instead of a table
//!
//! To save the answer: open https://bazaardb.gg/shop-odds, open the developer tools (F12) on the
//! Network tab, reload, click the `shop-odds` request, and save its Response as a .json file.

use bazaardb::{SHOP_ODDS_PAGE, TIERS, fetch_shop_odds, shop_odds::parse_json};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let file = args.iter().find(|a| !a.starts_with("--"));

    let odds = match file {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("reading {path}: {e}")));
            parse_json(&text).unwrap_or_else(|e| fail(&e.to_string()))
        }
        None => fetch_shop_odds().unwrap_or_else(|e| {
            fail(&format!(
                "{e}\nOpen {SHOP_ODDS_PAGE} in a browser, save the `shop-odds` response from the Network tab, and pass the file."
            ))
        }),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&odds).expect("serialisable"));
        return;
    }
    print!("{:>4}{:>9}", "Day", "Samples");
    for tier in TIERS {
        print!("{tier:>9}");
    }
    println!();
    for day in &odds.days {
        print!("{:>4}{:>9}", day.day, day.samples);
        for p in day.tiers {
            print!("{:>8.1}%", p);
        }
        println!();
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}
