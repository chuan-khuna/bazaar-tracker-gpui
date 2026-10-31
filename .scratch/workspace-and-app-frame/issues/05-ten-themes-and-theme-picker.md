# 05: Ten themes and the theme picker

**What to build:** The player can restyle the app. **Change theme…** (menu, palette, Ctrl+K Ctrl+T) opens a picker of ten themes, dark ones first and light ones after under headings; the window previews each theme as the highlight moves, Escape restores the previous one, and the chosen theme is remembered in `settings.toml`. See the spec's Themes and Theme lookup decisions, including the Rosé Pine role mapping.

**Blocked by:** 04 (Menu, command palette and settings overlay)

**Status:** resolved

- [x] Nine more themes, each in its own file with every role filled: Catppuccin Mocha, Flexoki Dark, Tokyo Night, Everforest, Kanagawa Wave, Nord, Rosé Pine Dawn, Catppuccin Latte, Flexoki Light. Themes with a real orange or green use it for burn, heal and poison.
- [x] The theme list starts with Rosé Pine and lists dark themes before light ones.
- [x] The picker filters by name or "dark"/"light"; moving the highlight previews the theme; Enter chooses; Escape restores the theme from before opening.
- [x] The Change theme… menu row shows the current theme's name.
- [x] The chosen theme is saved as `theme` in `settings.toml`; an unknown or missing name gives Rosé Pine.
- [x] Tests (themes seam): every theme found by name; unknown name gives Rosé Pine; Rosé Pine first; dark before light; "cmo" finds Catppuccin Mocha and "rose" finds Rosé Pine.
- [x] Tests (overlays seam): the picker's preview-on-move, choose-on-Enter and restore-on-Escape outcomes.
