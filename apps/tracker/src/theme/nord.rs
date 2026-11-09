//! Nord, dark: https://www.nordtheme.com/docs/colors-and-palettes

use super::Theme;

pub static NORD: Theme = Theme {
    name: "Nord",
    dark: true,
    background: 0x242933,     // darker than nord0, so the panels read as islands
    surface: 0x2e3440,        // nord0
    overlay: 0x3b4252,        // nord1
    highlight_low: 0x3b4252,  // nord1
    highlight_med: 0x434c5e,  // nord2
    highlight_high: 0x4c566a, // nord3
    text: 0xd8dee9,   // nord4
    subtle: 0x9aa5b8, // between nord3 and nord4
    muted: 0x616e88,  // nord3, brightened
    heading: 0xd08770, // nord12, orange
    error: 0xbf616a,   // nord11, red
    success: 0x8fbcbb, // nord7
    accent: 0x88c0d0,  // nord8
    win: 0xa3be8c, // green
    loss: 0xbf616a, // red
    live: 0x8fbcbb, // nord7
    bronze: 0xd08770, // orange
    silver: 0x9aa5b8, // nord3-4
    gold: 0xebcb8b, // yellow
    diamond: 0x81a1c1, // blue
    legendary: 0xb48ead, // purple
    damage: 0xbf616a, // red
    shield: 0x81a1c1, // blue
    heal: 0xa3be8c, // green
    burn: 0xd08770, // orange
    poison: 0xb48ead, // purple
    regen: 0x8fbcbb, // nord7
    ammo: 0x9aa5b8, // nord3-4
    value: 0xebcb8b, // yellow
};
