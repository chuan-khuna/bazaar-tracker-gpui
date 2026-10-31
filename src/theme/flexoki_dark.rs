//! Flexoki, dark: https://stephango.com/flexoki

use super::Theme;

pub static FLEXOKI_DARK: Theme = Theme {
    name: "Flexoki Dark",
    dark: true,
    background: 0x100f0f,    // black
    surface: 0x1c1b1a,       // 950
    overlay: 0x282726,       // 900
    highlight_low: 0x282726, // 900
    highlight_med: 0x403e3c, // 800
    highlight_high: 0x575653, // 700
    text: 0xcecdc3,   // 200
    subtle: 0x878580, // 500
    muted: 0x6f6e69,  // 600
    heading: 0xda702c, // orange 400
    error: 0xd14d41,   // red 400
    success: 0x879a39, // green 400
    accent: 0x3aa99f,  // cyan 400
    win: 0x879a39, // green
    loss: 0xd14d41, // red
    live: 0x3aa99f, // cyan
    bronze: 0xda702c, // orange
    silver: 0x878580, // 500
    gold: 0xd0a215, // yellow
    diamond: 0x4385be, // blue
    legendary: 0x8b7ec8, // purple
    damage: 0xd14d41, // red
    shield: 0x4385be, // blue
    heal: 0x879a39, // green
    burn: 0xda702c, // orange
    poison: 0x3aa99f, // cyan
    regen: 0x3aa99f, // cyan
    ammo: 0x878580, // 500
    value: 0xd0a215, // yellow
};
