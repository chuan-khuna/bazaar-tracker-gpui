use super::*;

fn type_text(palette: &mut CommandPalette, text: &str) {
    for c in text.chars() {
        palette.press(&c.to_string(), Some(&c.to_string()));
    }
}

#[test]
fn the_commands_are_the_agreed_ones_in_menu_order() {
    assert_eq!(
        Command::ALL.map(Command::label),
        ["Settings…", "Extract card art", "Import item odds…", "Clear data…", "Show sidebar", "Show detail", "Show log", "Show run duration", "Change theme…", "Reset layout", "Quit"]
    );
}

#[test]
fn the_menu_and_the_palette_offer_the_same_commands() {
    assert_eq!(Menu.commands(), CommandPalette::default().matches());
    assert_eq!(Menu.commands(), Command::ALL);
}

#[test]
fn search_filters_by_label() {
    assert_eq!(Command::search("theme"), [Command::ChangeTheme]);
    assert_eq!(Command::search("layout"), [Command::ResetLayout]);
    assert_eq!(Command::search("quit"), [Command::Quit]);
    assert_eq!(Command::search("show"), [Command::ToggleSidebar, Command::ToggleDetail, Command::ToggleLog, Command::ToggleRunDuration]);
    assert_eq!(Command::search("").len(), Command::ALL.len());
    assert!(Command::search("zzz").is_empty());
}

#[test]
fn search_is_loose_like_the_theme_picker() {
    assert_eq!(Command::search("rdur"), [Command::ToggleRunDuration]);
    assert_eq!(Command::search("SETTINGS"), [Command::Settings]);
}

#[test]
fn enter_runs_the_highlighted_command() {
    let mut palette = CommandPalette::default();
    assert_eq!(palette.press("enter", None), Outcome::Run(Command::Settings));
    palette.press("down", None);
    assert_eq!(palette.press("enter", None), Outcome::Run(Command::ExtractArt));
}

#[test]
fn up_and_down_move_the_highlight_and_wrap() {
    let mut palette = CommandPalette::default();
    palette.press("up", None);
    assert_eq!(palette.press("enter", None), Outcome::Run(Command::Quit));
    palette.press("down", None);
    assert_eq!(palette.press("enter", None), Outcome::Run(Command::Settings));
}

#[test]
fn typing_filters_and_highlights_the_first_match() {
    let mut palette = CommandPalette::default();
    palette.press("down", None);
    type_text(&mut palette, "show dt");
    assert_eq!(palette.matches(), [Command::ToggleDetail, Command::ToggleRunDuration]);
    assert_eq!(palette.press("enter", None), Outcome::Run(Command::ToggleDetail));
}

#[test]
fn backspace_widens_the_filter_again() {
    let mut palette = CommandPalette::default();
    type_text(&mut palette, "quit");
    assert_eq!(palette.matches(), [Command::Quit]);
    palette.press("backspace", None);
    palette.press("backspace", None);
    palette.press("backspace", None);
    palette.press("backspace", None);
    assert_eq!(palette.matches().len(), Command::ALL.len());
}

#[test]
fn control_keys_are_not_typed() {
    let mut palette = CommandPalette::default();
    palette.press("tab", Some("\t"));
    assert_eq!(palette.query, "");
}

#[test]
fn enter_with_no_matches_does_nothing() {
    let mut palette = CommandPalette::default();
    type_text(&mut palette, "zzz");
    assert_eq!(palette.press("enter", None), Outcome::None);
    assert_eq!(palette.press("down", None), Outcome::None);
}

#[test]
fn escape_closes_the_palette_and_the_menu() {
    assert_eq!(CommandPalette::default().press("escape", None), Outcome::Close);
    assert_eq!(Menu.press("escape"), MenuOutcome::Close);
    assert_eq!(Menu.press("a"), MenuOutcome::None);
}
