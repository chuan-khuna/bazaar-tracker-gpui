//! Tokyo Night, dark: https://github.com/folke/tokyonight.nvim

use super::Theme;

pub static TOKYO_NIGHT: Theme = Theme {
    name: "Tokyo Night",
    dark: true,
    background: 0x16161e,     // bg_dark
    surface: 0x1a1b26,        // bg
    overlay: 0x292e42,        // bg_highlight
    highlight_low: 0x1f2335,
    highlight_med: 0x3b4261,  // fg_gutter
    highlight_high: 0x545c7e, // dark3
    text: 0xc0caf5,   // fg
    subtle: 0xa9b1d6, // fg_dark
    muted: 0x565f89,  // comment
    heading: 0xff9e64, // orange
    error: 0xf7768e,   // red
    success: 0x1abc9c, // teal
    accent: 0xbb9af7,  // purple
    win: 0x9ece6a, // green
    loss: 0xf7768e, // red
    live: 0x1abc9c, // teal
    bronze: 0xff9e64, // orange
    silver: 0xa9b1d6, // fg_dark
    gold: 0xe0af68, // yellow
    diamond: 0x7aa2f7, // blue
    legendary: 0xbb9af7, // purple
    damage: 0xf7768e, // red
    shield: 0x7aa2f7, // blue
    heal: 0x9ece6a, // green
    burn: 0xff9e64, // orange
    poison: 0x73daca, // green1
    regen: 0x1abc9c, // teal
    ammo: 0xa9b1d6, // fg_dark
    value: 0xe0af68, // yellow
};
