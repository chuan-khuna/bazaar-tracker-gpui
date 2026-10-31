//! Catppuccin Mocha, dark: https://catppuccin.com/palette

use super::Theme;

pub static CATPPUCCIN_MOCHA: Theme = Theme {
    name: "Catppuccin Mocha",
    dark: true,
    background: 0x1e1e2e,     // base
    surface: 0x181825,        // mantle
    overlay: 0x313244,        // surface0
    highlight_low: 0x181825,  // mantle
    highlight_med: 0x45475a,  // surface1
    highlight_high: 0x585b70, // surface2
    text: 0xcdd6f4,
    subtle: 0xa6adc8, // subtext0
    muted: 0x6c7086,  // overlay0
    heading: 0xcba6f7, // mauve
    error: 0xf38ba8,   // red
    success: 0x94e2d5, // teal
    accent: 0xb4befe,  // lavender
    win: 0xa6e3a1, // green
    loss: 0xf38ba8, // red
    live: 0x94e2d5, // teal
    bronze: 0xeba0ac, // bronze
    silver: 0xa6adc8, // subtext0
    gold: 0xf9e2af, // yellow
    diamond: 0x74c7ec, // blue
    legendary: 0xcba6f7, // purple
    damage: 0xf38ba8, // red
    shield: 0x74c7ec, // blue
    heal: 0xa6e3a1, // green
    burn: 0xfab387, // orange
    poison: 0xcba6f7, // mauve
    regen: 0x94e2d5, // teal
    ammo: 0xa6adc8, // subtext0
    value: 0xf9e2af, // yellow
};
