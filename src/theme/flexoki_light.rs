//! Flexoki, light: https://stephango.com/flexoki

use super::Theme;

pub static FLEXOKI_LIGHT: Theme = Theme {
    name: "Flexoki Light",
    dark: false,
    background: 0xf2f0e5,    // 50
    surface: 0xfffcf0,       // paper
    overlay: 0xe6e4d9,       // 100
    highlight_low: 0xe6e4d9, // 100
    highlight_med: 0xcecdc3, // 200
    highlight_high: 0xb7b5ac, // 300
    text: 0x100f0f,   // black
    subtle: 0x6f6e69, // 600
    muted: 0x878580,  // 500
    heading: 0xbc5215, // orange 600
    error: 0xaf3029,   // red 600
    success: 0x66800b, // green 600
    accent: 0x24837b,  // cyan 600
    win: 0x66800b, // green
    loss: 0xaf3029, // red
    live: 0x24837b, // cyan
    bronze: 0xbc5215, // orange
    silver: 0x6f6e69, // 600
    gold: 0xad8301, // yellow
    diamond: 0x205ea6, // blue
    legendary: 0x5e409d, // purple
    damage: 0xaf3029, // red
    shield: 0x205ea6, // blue
    heal: 0x66800b, // green
    burn: 0xbc5215, // orange
    poison: 0x24837b, // cyan
    regen: 0x24837b, // cyan
    ammo: 0x6f6e69, // 600
    value: 0xad8301, // yellow
};
