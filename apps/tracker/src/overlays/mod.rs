//! The state and keys behind what floats above the window (the menu, the command palette, the
//! theme picker, the settings overlay, the Advisor's model picker and the widgets' pop-ups), testable without a window. `ui/` draws them.
//!
//! Each overlay is a plain struct: it takes the name of a key and returns an outcome for the app
//! to carry out.

pub mod commands;
pub mod item_odds;
pub mod model_picker;
pub mod search;
pub mod settings_overlay;
pub mod shop_odds;
pub mod theme_picker;
pub mod tracked_items;

use commands::{CommandPalette, Menu};
use item_odds::ItemOddsOverlay;
use model_picker::ModelPicker;
use settings_overlay::SettingsOverlay;
use shop_odds::ShopOddsOverlay;
use theme_picker::ThemePicker;
use tracked_items::TrackedItemsOverlay;

/// The overlay that is open. The app holds at most one, so opening one closes the others.
pub enum Overlay {
    Menu(Menu),
    Palette(CommandPalette),
    ThemePicker(ThemePicker),
    Settings(SettingsOverlay),
    ShopOdds(ShopOddsOverlay),
    ItemOdds(ItemOddsOverlay),
    TrackedItems(TrackedItemsOverlay),
    ModelPicker(ModelPicker),
}
