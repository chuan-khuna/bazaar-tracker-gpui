//! The state and keys behind what floats above the window (the menu, the command palette, the
//! theme picker and the settings overlay), testable without a window. `ui/` draws them.
//!
//! Each overlay is a plain struct: it takes the name of a key and returns an outcome for the app
//! to carry out.

pub mod commands;
pub mod search;
pub mod settings;
pub mod theme_picker;

use commands::{CommandPalette, Menu};
use settings::SettingsOverlay;
use theme_picker::ThemePicker;

/// The overlay that is open. The app holds at most one, so opening one closes the others.
pub enum Overlay {
    Menu(Menu),
    Palette(CommandPalette),
    ThemePicker(ThemePicker),
    Settings(SettingsOverlay),
}
