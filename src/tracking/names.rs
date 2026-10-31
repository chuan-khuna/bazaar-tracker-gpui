//! Card names and card art for the views: names from the game's card catalog once it has loaded,
//! art from the images extracted into the data folder (`bazaar_tracker::art`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bazaar_tracker::art;
use bazaar_tracker::catalog::CardCatalog;

/// Which art images are in the art folder, by their path under it (`items/<key>.webp`).
#[derive(Debug, Default)]
pub struct ArtIndex {
    dir: PathBuf,
    present: HashSet<String>,
}

impl ArtIndex {
    /// List the images under `dir`. A folder that is missing gives an empty index.
    pub fn load(dir: &Path) -> Self {
        let mut present = HashSet::new();
        for category in std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|entry| entry.path().is_dir()) {
            let category_name = category.file_name().to_string_lossy().into_owned();
            for image in std::fs::read_dir(category.path()).into_iter().flatten().flatten() {
                let name = image.file_name().to_string_lossy().into_owned();
                if name.ends_with(".webp") {
                    present.insert(format!("{category_name}/{name}"));
                }
            }
        }
        Self { dir: dir.to_path_buf(), present }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.present.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.present.is_empty()
    }

    fn path(&self, relative: &str) -> Option<PathBuf> {
        self.present.contains(relative).then(|| self.dir.join(relative))
    }
}

/// Looks a card up by its template id. Until the catalog has loaded, or for a card it does not
/// know, a card is named by its template id, and failing that by its instance id, and has no art.
#[derive(Clone, Default)]
pub struct Names {
    catalog: Option<Arc<CardCatalog>>,
    art: Arc<ArtIndex>,
}

impl Names {
    #[cfg(test)]
    pub fn new(catalog: Arc<CardCatalog>) -> Self {
        Self { catalog: Some(catalog), art: Arc::default() }
    }

    pub fn with_art(mut self, art: Arc<ArtIndex>) -> Self {
        self.art = art;
        self
    }

    pub fn set_catalog(&mut self, catalog: Arc<CardCatalog>) {
        self.catalog = Some(catalog);
    }

    pub fn set_art(&mut self, art: Arc<ArtIndex>) {
        self.art = art;
    }

    pub fn card(&self, template_id: Option<&str>, instance_id: &str) -> String {
        let Some(template_id) = template_id else {
            return instance_id.to_owned();
        };
        match self.catalog.as_ref().and_then(|catalog| catalog.get(template_id)) {
            Some(card) => card.name.clone(),
            None => template_id.to_owned(),
        }
    }

    /// The card's extracted art, if the catalog knows the card and its image is in the art folder.
    pub fn art(&self, template_id: Option<&str>) -> Option<PathBuf> {
        let card = self.catalog.as_ref()?.get(template_id?)?;
        self.art.path(&art::relative_path(card)?)
    }
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
