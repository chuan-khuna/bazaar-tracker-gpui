//! Extract the card art from your install of The Bazaar.
//!
//!     cargo run --release -p bazaar-tracker --example extract_art -- [--game <folder>] [--out <folder>] [--force]
//!
//! Without `--game` the game is found in the Steam libraries. Without `--out` the art goes to
//! `<repo>/.bazaar-tracker/art` (or `$BAZAAR_TRACKER_DATA_DIR/art`), where the app looks.

use std::path::PathBuf;

use bazaar_tracker::art::{self, Options, Progress};
use bazaar_tracker::catalog::CardCatalog;
use bazaar_tracker::log::path as log_path;

fn main() {
    let mut game = String::new();
    let mut out = None;
    let mut force = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game" => game = args.next().unwrap_or_default(),
            "--out" => out = args.next().map(PathBuf::from),
            "--force" => force = true,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(game_dir) = art::find_game_dir(&game) else {
        eprintln!("The Bazaar was not found: pass --game <install folder>");
        std::process::exit(1);
    };
    let out = out.unwrap_or_else(|| {
        let data = std::env::var_os("BAZAAR_TRACKER_DATA_DIR").filter(|dir| !dir.is_empty()).map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.bazaar-tracker"), PathBuf::from);
        data.join("art")
    });
    let Some(profile) = log_path::user_profile() else {
        eprintln!("the home folder is not set (USERPROFILE or HOME), so the game's card data cannot be found");
        std::process::exit(1);
    };
    let catalog = CardCatalog::load(&log_path::game_folder_in(&profile));
    println!("game {} · {} cards · into {}", game_dir.display(), catalog.len(), out.display());

    let mut options = Options::new(game_dir, out);
    options.force = force;
    let result = art::extract(&options, &catalog, &|progress| match progress {
        Progress::Planned { images, to_write, bundles } => println!("{images} images; {to_write} to write from {bundles} bundles"),
        Progress::Bundle { done, total, name, written, of } => println!("[{done}/{total}] {name}: {written}/{of}"),
    });
    match result {
        Ok(summary) => {
            println!("wrote {} images ({:.1} MB) in {:.0}s; {} cards have art, {} do not", summary.written, summary.bytes as f64 / 1e6, summary.seconds, summary.cards, summary.failures.len());
            let mut reasons: Vec<(usize, &String)> = Vec::new();
            for why in summary.failures.values() {
                match reasons.iter_mut().find(|(_, seen)| *seen == why) {
                    Some((count, _)) => *count += 1,
                    None => reasons.push((1, why)),
                }
            }
            reasons.sort_by_key(|(count, _)| std::cmp::Reverse(*count));
            for (count, why) in reasons.iter().take(8) {
                println!("  {count:5} {why}");
            }
        }
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(1);
        }
    }
}
