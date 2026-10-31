//! Kanagawa Wave, dark: https://github.com/rebelot/kanagawa.nvim

use super::Theme;

pub static KANAGAWA_WAVE: Theme = Theme {
    name: "Kanagawa Wave",
    dark: true,
    background: 0x16161d,     // sumiInk0
    surface: 0x1f1f28,        // sumiInk3
    overlay: 0x2a2a37,        // sumiInk4
    highlight_low: 0x1a1a22,  // sumiInk2
    highlight_med: 0x363646,  // sumiInk5
    highlight_high: 0x54546d, // sumiInk6
    text: 0xdcd7ba,   // fujiWhite
    subtle: 0xc8c093, // oldWhite
    muted: 0x727169,  // fujiGray
    heading: 0xffa066, // surimiOrange
    error: 0xe46876,   // waveRed
    success: 0x7aa89f, // waveAqua2
    accent: 0x957fb8,  // oniViolet
    win: 0x98bb6c, // green
    loss: 0xe46876, // red
    live: 0x7aa89f, // waveAqua2
    bronze: 0xffa066, // orange
    silver: 0xc8c093, // oldWhite
    gold: 0xe6c384, // yellow
    diamond: 0x7e9cd8, // blue
    legendary: 0x957fb8, // purple
    damage: 0xe46876, // red
    shield: 0x7e9cd8, // blue
    heal: 0x98bb6c, // green
    burn: 0xffa066, // orange
    poison: 0x76946a, // autumnGreen
    regen: 0x7aa89f, // waveAqua2
    ammo: 0xc8c093, // oldWhite
    value: 0xe6c384, // yellow
};
