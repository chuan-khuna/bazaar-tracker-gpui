//! The card catalog: a card id turned into its name, starting tier, size, type and description,
//! from the game's own card data.
//!
//! The data is `GameData.db` (SQLite) or, failing that, `cards.json`, in the game's
//! `LocalLow\Tempo Storm\The Bazaar` folder. The game may be running and writing there, so
//! `GameData.db` is copied to a temporary file and read from the copy. Finding nothing is not an
//! error: the catalog is empty and a line is logged.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

/// What the catalog knows about one card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CardInfo {
    /// The card's id, as the game writes it.
    pub id: String,
    /// Its localised title, else its internal name, else its id.
    pub name: String,
    pub starting_tier: Option<String>,
    pub size: Option<String>,
    pub card_type: Option<String>,
    pub description: Option<String>,
    /// The Addressables key of its art (`ArtKey`): see `crate::art`.
    pub art_key: Option<String>,
    /// The heroes whose shops sell it (`Heroes`); a neutral card has [`NEUTRAL_HERO`].
    pub heroes: Vec<String>,
}

/// What the game writes in a card's `Heroes` when every hero can get it.
pub const NEUTRAL_HERO: &str = "Common";

/// Cards by id. Looking one up ignores case.
#[derive(Clone, Debug, Default)]
pub struct CardCatalog {
    cards: HashMap<String, CardInfo>,
    source: Option<PathBuf>,
}

impl CardCatalog {
    /// A catalog with no cards.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The card with this id, ignoring case.
    pub fn get(&self, id: &str) -> Option<&CardInfo> {
        self.cards.get(&id.to_lowercase())
    }

    /// Every card, in no particular order.
    pub fn cards(&self) -> impl Iterator<Item = &CardInfo> {
        self.cards.values()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// The file the cards were read from, if any were.
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    fn insert(&mut self, card: CardInfo) {
        self.cards.insert(card.id.to_lowercase(), card);
    }

    /// Load the catalog from the game's data under `root` (the `The Bazaar` folder in `LocalLow`):
    /// `GameData.db` if it can be read, otherwise `cards.json`, otherwise nothing.
    pub fn load(root: &Path) -> Self {
        if let Some(path) = newest_existing(&candidates(root, "GameData.db")) {
            match load_game_data_db(&path) {
                Ok(catalog) => return catalog,
                Err(error) => tracing::info!("card catalog: {} could not be read ({error}), trying cards.json", path.display()),
            }
        }
        if let Some(path) = newest_existing(&candidates(root, "cards.json")) {
            match load_cards_json(&path) {
                Ok(catalog) => return catalog,
                Err(error) => tracing::info!("card catalog: {} could not be read ({error})", path.display()),
            }
        }
        tracing::info!("card catalog: no game data found, so card names will not resolve");
        Self::empty()
    }

    /// Parse the text of a `cards.json`: an object whose values are arrays of cards.
    pub fn from_cards_json(text: &str) -> Result<Self, serde_json::Error> {
        let root: Value = serde_json::from_str(text)?;
        let mut catalog = Self::empty();
        if let Some(groups) = root.as_object() {
            for card in groups.values().filter_map(Value::as_array).flatten() {
                if let Some(id) = card.get("Id").and_then(text_of) {
                    catalog.insert(parse_card(&id, card));
                }
            }
        }
        Ok(catalog)
    }
}

/// Where the game may keep `file`: in its `cache` folder (the usual place), and in the folder
/// itself, each with and without the `prod` folder.
pub fn candidates(root: &Path, file: &str) -> Vec<PathBuf> {
    let prod = root.join("prod");
    vec![prod.join("cache").join(file), root.join("cache").join(file), prod.join(file), root.join(file)]
}

/// The candidate that exists and was written most recently.
pub fn newest_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .filter_map(|path| fs::metadata(path).ok().filter(|meta| meta.is_file()).map(|meta| (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), path)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path.clone())
}

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Build a card from its JSON. The name is the localised title, else the internal name, else the id.
fn parse_card(id: &str, json: &Value) -> CardInfo {
    let localised = |part: &str| json.get("Localization").and_then(|loc| loc.get(part)).and_then(|part| part.get("Text")).and_then(text_of);
    CardInfo {
        id: id.to_owned(),
        name: localised("Title").or_else(|| json.get("InternalName").and_then(text_of)).unwrap_or_else(|| id.to_owned()),
        starting_tier: json.get("StartingTier").and_then(text_of),
        size: json.get("Size").and_then(text_of),
        card_type: json.get("Type").and_then(text_of),
        description: localised("Description"),
        art_key: json.get("ArtKey").and_then(text_of).filter(|key| !key.is_empty()),
        heroes: json.get("Heroes").and_then(Value::as_array).map(|heroes| heroes.iter().filter_map(text_of).collect()).unwrap_or_default(),
    }
}

fn load_cards_json(path: &Path) -> Result<CardCatalog, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    let mut catalog = CardCatalog::from_cards_json(&text)?;
    catalog.source = Some(path.to_path_buf());
    Ok(catalog)
}

/// A temporary file that is deleted when dropped.
struct TempCopy(PathBuf);

impl Drop for TempCopy {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn temp_copy_name() -> PathBuf {
    let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_nanos());
    std::env::temp_dir().join(format!("bazaar_tracker_gamedata_{}_{nanos}.db", std::process::id()))
}

/// Read `GameData.db`: shared-read open (the std default on Windows lets the game keep writing),
/// copy to a temporary file, read the copy, delete the copy.
fn load_game_data_db(path: &Path) -> Result<CardCatalog, Box<dyn std::error::Error>> {
    let mut source = File::open(path)?;
    let copy = TempCopy(temp_copy_name());
    {
        let mut target = File::create(&copy.0)?;
        io::copy(&mut source, &mut target)?;
    }
    drop(source);

    let connection = Connection::open_with_flags(&copy.0, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare("SELECT Id, Data FROM cards")?;
    let mut rows = statement.query([])?;
    let mut catalog = CardCatalog::empty();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        // The card is JSON, stored as a blob (or as text).
        let data: Vec<u8> = match row.get_ref(1)? {
            rusqlite::types::ValueRef::Blob(bytes) | rusqlite::types::ValueRef::Text(bytes) => bytes.to_vec(),
            _ => continue,
        };
        if let Ok(json) = serde_json::from_slice::<Value>(&data) {
            catalog.insert(parse_card(&id, &json));
        }
    }
    drop(rows);
    drop(statement);
    drop(connection);
    catalog.source = Some(path.to_path_buf());
    Ok(catalog)
}

/// A catalog being loaded on its own thread. Loading happens once, when this is started; asking for
/// the catalog never blocks unless [`Self::wait`] is used.
#[derive(Clone)]
pub struct CatalogHandle {
    shared: Arc<(Mutex<Option<Arc<CardCatalog>>>, Condvar)>,
}

impl CatalogHandle {
    /// Start loading the catalog from `root` on a new thread.
    pub fn start(root: PathBuf) -> Self {
        let shared: Arc<(Mutex<Option<Arc<CardCatalog>>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
        let loader = shared.clone();
        std::thread::Builder::new()
            .name("card-catalog".into())
            .spawn(move || {
                let catalog = Arc::new(CardCatalog::load(&root));
                *loader.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(catalog);
                loader.1.notify_all();
            })
            .expect("the operating system can start a thread");
        Self { shared }
    }

    /// The catalog, if it has finished loading.
    pub fn get(&self) -> Option<Arc<CardCatalog>> {
        self.shared.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    /// Wait up to `timeout` for the catalog.
    pub fn wait(&self, timeout: Duration) -> Option<Arc<CardCatalog>> {
        let (lock, condvar) = &*self.shared;
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (guard, _) = condvar.wait_timeout_while(guard, timeout, |catalog| catalog.is_none()).unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.clone()
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
