//! The colour themes, and the search behind the theme picker.
//!
//! Each theme is in its own file and maps its palette onto the same roles. To add one, add a file
//! and list it in `THEMES`.

mod catppuccin_latte;
mod catppuccin_mocha;
mod everforest;
mod flexoki_dark;
mod flexoki_light;
mod kanagawa_wave;
mod nord;
mod rose_pine;
mod rose_pine_dawn;
mod tokyo_night;

pub use catppuccin_latte::CATPPUCCIN_LATTE;
pub use catppuccin_mocha::CATPPUCCIN_MOCHA;
pub use everforest::EVERFOREST;
pub use flexoki_dark::FLEXOKI_DARK;
pub use flexoki_light::FLEXOKI_LIGHT;
pub use kanagawa_wave::KANAGAWA_WAVE;
pub use nord::NORD;
pub use rose_pine::ROSE_PINE;
pub use rose_pine_dawn::ROSE_PINE_DAWN;
pub use tokyo_night::TOKYO_NIGHT;

/// Every colour the UI uses, by role. Colours are `0xRRGGBB`.
///
/// The roles after `accent` are for the board and event views that come in a later spec; every
/// theme fills them in now so those views need no change to any theme.
#[derive(Debug, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    /// The window behind everything.
    pub background: u32,
    /// The island panels, and the menus and pickers.
    pub surface: u32,
    /// Buttons, separators, hovered menu rows.
    pub overlay: u32,
    pub highlight_low: u32,
    /// Menu borders and other quiet borders.
    pub highlight_med: u32,
    pub highlight_high: u32,
    pub text: u32,
    pub subtle: u32,
    pub muted: u32,
    pub heading: u32,
    pub error: u32,
    pub success: u32,
    /// The caret and the border of a focused field.
    pub accent: u32,

    // Outcome of a run, and the run being played now.
    pub win: u32,
    pub loss: u32,
    pub live: u32,

    // Item tiers.
    pub bronze: u32,
    pub silver: u32,
    pub gold: u32,
    pub diamond: u32,
    pub legendary: u32,

    // Item badges.
    pub damage: u32,
    pub shield: u32,
    pub heal: u32,
    pub burn: u32,
    pub poison: u32,
    pub regen: u32,
    pub ammo: u32,
    pub value: u32,
}

impl Theme {
    pub fn appearance(&self) -> &'static str {
        if self.dark { "Dark" } else { "Light" }
    }

    /// Every colour of the theme, with its role's name, for checking that none is left out.
    #[cfg(test)]
    pub fn roles(&self) -> [(&'static str, u32); 29] {
        [
            ("background", self.background),
            ("surface", self.surface),
            ("overlay", self.overlay),
            ("highlight_low", self.highlight_low),
            ("highlight_med", self.highlight_med),
            ("highlight_high", self.highlight_high),
            ("text", self.text),
            ("subtle", self.subtle),
            ("muted", self.muted),
            ("heading", self.heading),
            ("error", self.error),
            ("success", self.success),
            ("accent", self.accent),
            ("win", self.win),
            ("loss", self.loss),
            ("live", self.live),
            ("bronze", self.bronze),
            ("silver", self.silver),
            ("gold", self.gold),
            ("diamond", self.diamond),
            ("legendary", self.legendary),
            ("damage", self.damage),
            ("shield", self.shield),
            ("heal", self.heal),
            ("burn", self.burn),
            ("poison", self.poison),
            ("regen", self.regen),
            ("ammo", self.ammo),
            ("value", self.value),
        ]
    }
}

/// Every theme, Rosé Pine (the default) first, then the other dark ones and then the light ones:
/// the picker lists them in this order, under a heading for each.
pub static THEMES: [&Theme; 10] = [
    &ROSE_PINE,
    &CATPPUCCIN_MOCHA,
    &FLEXOKI_DARK,
    &TOKYO_NIGHT,
    &EVERFOREST,
    &KANAGAWA_WAVE,
    &NORD,
    &ROSE_PINE_DAWN,
    &CATPPUCCIN_LATTE,
    &FLEXOKI_LIGHT,
];

/// The theme called exactly `name`, or Rosé Pine if there is none (or no name).
pub fn by_name(name: Option<&str>) -> &'static Theme {
    name.and_then(|name| THEMES.iter().copied().find(|theme| theme.name == name)).unwrap_or(&ROSE_PINE)
}

/// The themes whose name (or "dark"/"light") matches `query`, in order.
/// See `search::matches`: "cmo" finds Catppuccin Mocha, and "rose" finds Rosé Pine.
pub fn search(query: &str) -> Vec<&'static Theme> {
    THEMES.iter().copied().filter(|theme| crate::overlays::search::matches(query, &format!("{} {}", theme.name, theme.appearance()))).collect()
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
