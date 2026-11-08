//! Everforest, dark (medium): https://github.com/sainnhe/everforest

use super::Theme;

pub static EVERFOREST: Theme = Theme {
    name: "Everforest",
    dark: true,
    background: 0x232a2e,     // bg_dim
    surface: 0x2d353b,        // bg0
    overlay: 0x343f44,        // bg1
    highlight_low: 0x343f44,  // bg1
    highlight_med: 0x475258,  // bg3
    highlight_high: 0x56635f, // bg5
    text: 0xd3c6aa,   // fg
    subtle: 0x9da9a0, // grey2
    muted: 0x7a8478,  // grey0
    heading: 0xe69875, // orange
    error: 0xe67e80,   // red
    success: 0x83c092, // aqua
    accent: 0x7fbbb3,  // blue
    win: 0xa7c080, // green
    loss: 0xe67e80, // red
    live: 0x83c092, // aqua
    bronze: 0xe69875, // orange
    silver: 0x9da9a0, // grey2
    gold: 0xdbbc7f, // yellow
    diamond: 0x7fbbb3, // blue
    legendary: 0xd699b6, // purple
    damage: 0xe67e80, // red
    shield: 0x7fbbb3, // blue
    heal: 0xa7c080, // green
    burn: 0xe69875, // orange
    poison: 0x83c092, // aqua
    regen: 0x83c092, // aqua
    ammo: 0x9da9a0, // grey2
    value: 0xdbbc7f, // yellow
};
