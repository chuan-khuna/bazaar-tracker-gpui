//! Card art: running `bazaar_tracker::art` on a background thread to fill `<data dir>/art`, and
//! reading what a run left there.
//!
//! The art is Tempo Storm's: it is extracted from the player's own install, for showing here only.

use std::fs;
use std::path::{Path, PathBuf};

use bazaar_tracker::art::{self as extract, Options};
use bazaar_tracker::catalog::CardCatalog;
use bazaar_tracker::log::path as log_path;
use serde_json::Value;

pub use bazaar_tracker::art::Progress as Step;

/// The folder in the data folder that holds the art.
pub const FOLDER: &str = "art";

/// What the art folder holds, from its `manifest.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtInfo {
    /// Cards that have an image.
    pub cards: usize,
    /// Cards with an `ArtKey` whose image could not be extracted.
    pub missing: usize,
    /// When the manifest was written (ISO 8601, UTC).
    pub generated_at: String,
}

impl ArtInfo {
    /// Read `<art dir>/manifest.json`. None if there is none, or it cannot be read.
    pub fn load(art_dir: &Path) -> Option<Self> {
        Self::parse(&fs::read_to_string(art_dir.join(extract::MANIFEST)).ok()?)
    }

    pub fn parse(text: &str) -> Option<Self> {
        let manifest: Value = serde_json::from_str(text).ok()?;
        let count = |field: &str| manifest.get(field).and_then(Value::as_object).map_or(0, |map| map.len());
        Some(Self {
            cards: count("cards"),
            missing: count("failures"),
            generated_at: manifest.get("generated_at").and_then(Value::as_str).unwrap_or_default().to_owned(),
        })
    }

    /// The day it was written, as `yyyy-mm-dd`.
    pub fn date(&self) -> &str {
        self.generated_at.get(..10).unwrap_or(&self.generated_at)
    }
}

/// How far a run has got.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    /// Bundles finished, and how many there are.
    pub bundles: Option<(usize, usize)>,
}

impl Progress {
    pub fn apply(&mut self, step: &Step) {
        match step {
            Step::Planned { bundles, .. } => self.bundles = Some((0, *bundles)),
            Step::Bundle { done, total, .. } => self.bundles = Some((*done, *total)),
        }
    }

    /// One line for the settings overlay.
    pub fn describe(&self) -> String {
        match self.bundles {
            Some((_, 0)) => "Finishing…".into(),
            Some((done, total)) => format!("Extracting… {done} of {total} bundles"),
            None => "Reading the game's card data…".into(),
        }
    }
}

/// What a run reports, step by step and then once at the end.
#[derive(Debug)]
pub enum Report {
    Step(Step),
    /// The number of cards with art, or why it failed.
    Finished(Result<usize, String>),
}

/// The art folder in `data_dir`.
pub fn art_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(FOLDER)
}

/// Start extracting into `<data_dir>/art` on a background thread. `game_dir` is the `game_dir`
/// setting: empty finds the game in the Steam libraries.
pub fn start(data_dir: &Path, game_dir: &str) -> async_channel::Receiver<Report> {
    let (sender, receiver) = async_channel::unbounded();
    let out = art_dir(data_dir);
    let game_dir = game_dir.to_owned();
    let spawned = std::thread::Builder::new().name("card-art".into()).spawn({
        let sender = sender.clone();
        move || {
            let outcome = run(out, &game_dir, &|step| {
                let _ = sender.send_blocking(Report::Step(step));
            });
            let _ = sender.send_blocking(Report::Finished(outcome));
        }
    });
    if let Err(error) = spawned {
        let _ = sender.send_blocking(Report::Finished(Err(format!("could not start: {error}"))));
    }
    receiver
}

fn run(out: PathBuf, game_dir: &str, on_step: &(dyn Fn(Step) + Sync)) -> Result<usize, String> {
    let game = extract::find_game_dir(game_dir).ok_or_else(|| {
        if game_dir.trim().is_empty() { "The Bazaar was not found in any Steam library. Start the game and try again, or set game_dir in settings.toml.".to_owned() } else { format!("{} is not The Bazaar's install folder", game_dir.trim()) }
    })?;
    let profile = log_path::user_profile().ok_or("USERPROFILE is not set, so the game's card data cannot be found")?;
    let catalog = CardCatalog::load(&log_path::game_folder_in(&profile));
    tracing::info!("card art: from {} into {}", game.display(), out.display());
    let summary = extract::extract(&Options::new(game, out), &catalog, on_step)?;
    tracing::info!("card art: wrote {} images ({} bytes) in {:.0}s; {} cards have art, {} do not", summary.written, summary.bytes, summary.seconds, summary.cards, summary.failures.len());
    Ok(summary.cards)
}

#[cfg(test)]
#[path = "art_tests.rs"]
mod tests;
