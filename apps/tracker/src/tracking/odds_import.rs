//! Reading item odds saved by the odds scraper (`apps/scraper/`, see
//! `docs/documents/odds-scraper.md`), so Import item odds… can save them to the history.
//!
//! The scraper writes one `<template_id>.json` per item, each entry of its `odds` with exactly the
//! fields of [`SavedItemOdds`]:
//!
//! ```json
//! { "format": 1, "template_id": "…", "item_name": "Pasta Maker", "url_id": "qnwx…",
//!   "odds": [ { "template_id": "…", "hero": "Jules", "tier": "None", "url_id": "qnwx…",
//!               "item_name": "Pasta Maker", "body": "{\"itemName\":…}", "saved_ms": 1793534002000 } ] }
//! ```
//!
//! `index.json` and `skipped.json` sit in the same folder and are not odds; they are left out.

use std::path::{Path, PathBuf};

use bazaar_tracker::history::SavedItemOdds;
use serde::Deserialize;

/// The scraper's format; a file of another is not read.
pub const FORMAT: u32 = 1;
/// Files in the scraper's folder that hold no odds.
pub const NOT_ODDS: [&str; 2] = ["index.json", "skipped.json"];

/// What was read from a folder or file.
#[derive(Debug, Default, PartialEq)]
pub struct Read {
    /// Every entry whose answer can be read, ready to save.
    pub odds: Vec<SavedItemOdds>,
    /// How many odds files were read.
    pub files: usize,
    /// What could not be read, one line each, naming the file.
    pub problems: Vec<String>,
}

#[derive(Deserialize)]
struct File {
    format: Option<u32>,
    #[serde(default)]
    odds: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    template_id: String,
    hero: String,
    tier: String,
    #[serde(default)]
    url_id: String,
    #[serde(default)]
    item_name: String,
    body: String,
    saved_ms: i64,
}

/// The odds in `path`: every odds file in it if it is a folder (the scraper's `odds/`), or the one
/// file it names.
pub fn read(path: &Path) -> Result<Read, String> {
    let files = if path.is_dir() { odds_files(path)? } else { vec![path.to_path_buf()] };
    let mut read = Read::default();
    for file in files {
        let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| file.display().to_string());
        match std::fs::read_to_string(&file) {
            Ok(text) => read_file(&name, &text, &mut read),
            Err(error) => read.problems.push(format!("{name}: {error}")),
        }
    }
    Ok(read)
}

fn odds_files(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let listing = std::fs::read_dir(folder).map_err(|error| format!("Could not open {}: {error}", folder.display()))?;
    let mut files: Vec<PathBuf> = listing
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "json"))
        .filter(|path| path.file_name().is_none_or(|name| !NOT_ODDS.iter().any(|skip| name == *skip)))
        .collect();
    files.sort();
    Ok(files)
}

/// Add the entries of one file's `text` to `read`, or say why they can not be.
pub fn read_file(name: &str, text: &str, read: &mut Read) {
    let file: File = match serde_json::from_str(text) {
        Ok(file) => file,
        Err(error) => {
            read.problems.push(format!("{name}: not an odds file ({error})"));
            return;
        }
    };
    if file.format != Some(FORMAT) {
        read.problems.push(format!("{name}: format {:?}, this app reads format {FORMAT}", file.format));
        return;
    }
    read.files += 1;
    for entry in file.odds {
        if let Err(error) = bazaardb::item_odds::parse_json(&entry.body) {
            read.problems.push(format!("{name}: the odds for {} ({}, {}) can not be read: {error}", entry.item_name, entry.hero, entry.tier));
            continue;
        }
        read.odds.push(SavedItemOdds {
            template_id: entry.template_id.to_lowercase(),
            hero: entry.hero,
            tier: entry.tier,
            url_id: entry.url_id,
            item_name: entry.item_name,
            body: entry.body,
            saved_ms: entry.saved_ms,
        });
    }
}

#[cfg(test)]
#[path = "odds_import_tests.rs"]
mod tests;
