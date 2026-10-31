//! Card names for the views, from the game's card catalog once it has loaded.

use std::sync::Arc;

use bazaar_tracker::catalog::CardCatalog;

/// Looks a card up by its template id. Until the catalog has loaded, or for a card it does not
/// know, a card is named by its template id, and failing that by its instance id.
#[derive(Clone, Default)]
pub struct Names {
    catalog: Option<Arc<CardCatalog>>,
}

impl Names {
    pub fn new(catalog: Arc<CardCatalog>) -> Self {
        Self { catalog: Some(catalog) }
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
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
