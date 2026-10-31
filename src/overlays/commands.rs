//! The commands behind the ☰ menu and the command palette, and the state and keys of each,
//! testable without a window. The menu and the palette (Ctrl+P) offer the same commands.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Settings,
    ToggleSidebar,
    ToggleDetail,
    ToggleLog,
    /// Saved, but has no visible effect until run durations are shown somewhere.
    ToggleRunDuration,
    ChangeTheme,
    ResetLayout,
    Quit,
}

impl Command {
    /// In menu order.
    pub const ALL: [Command; 8] = [
        Command::Settings,
        Command::ToggleSidebar,
        Command::ToggleDetail,
        Command::ToggleLog,
        Command::ToggleRunDuration,
        Command::ChangeTheme,
        Command::ResetLayout,
        Command::Quit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Command::Settings => "Settings…",
            Command::ToggleSidebar => "Show sidebar",
            Command::ToggleDetail => "Show detail",
            Command::ToggleLog => "Show log",
            Command::ToggleRunDuration => "Show run duration",
            Command::ChangeTheme => "Change theme…",
            Command::ResetLayout => "Reset layout",
            Command::Quit => "Quit",
        }
    }

    /// Commands in the same group sit together; the menu draws a line between groups.
    pub fn group(self) -> u8 {
        match self {
            Command::Settings => 0,
            Command::ToggleSidebar | Command::ToggleDetail | Command::ToggleLog | Command::ToggleRunDuration => 1,
            Command::ChangeTheme | Command::ResetLayout => 2,
            Command::Quit => 3,
        }
    }

    /// The commands whose label matches `query` (see `search::matches`), in menu order.
    pub fn search(query: &str) -> Vec<Command> {
        Command::ALL.into_iter().filter(|command| crate::overlays::search::matches(query, command.label())).collect()
    }
}

/// The ☰ menu. It lists every command; only Escape is a key of its own.
#[derive(Default)]
pub struct Menu;

/// What a key press in the menu asks the app to do.
#[derive(Debug, PartialEq)]
pub enum MenuOutcome {
    Close,
    /// Nothing to change.
    None,
}

impl Menu {
    /// The commands the menu lists, in order: the same as the palette with an empty query.
    pub fn commands(&self) -> Vec<Command> {
        Command::ALL.to_vec()
    }

    /// `key` is gpui's name for the key ("escape", "a").
    pub fn press(&mut self, key: &str) -> MenuOutcome {
        match key {
            "escape" => MenuOutcome::Close,
            _ => MenuOutcome::None,
        }
    }
}

/// The command palette: type to filter, Up and Down to move, Enter to run, Escape to close.
#[derive(Default)]
pub struct CommandPalette {
    pub query: String,
    /// Which of the matches is highlighted.
    pub selected: usize,
}

/// What a key press or click in the palette asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Close the palette and run this command.
    Run(Command),
    Close,
    /// Nothing to change, apart from the palette itself.
    None,
}

impl CommandPalette {
    pub fn matches(&self) -> Vec<Command> {
        Command::search(&self.query)
    }

    /// `key` is gpui's name for the key ("up", "enter", "a"); `typed` is the text it types, if any.
    pub fn press(&mut self, key: &str, typed: Option<&str>) -> Outcome {
        let matches = self.matches();
        let count = matches.len();
        match key {
            "escape" => Outcome::Close,
            "enter" => matches.get(self.selected).map_or(Outcome::None, |command| Outcome::Run(*command)),
            "up" | "down" if count > 0 => {
                self.selected = if key == "down" { (self.selected + 1) % count } else { (self.selected + count - 1) % count };
                Outcome::None
            }
            "backspace" => {
                self.query.pop();
                self.selected = 0;
                Outcome::None
            }
            _ => {
                if let Some(text) = typed.filter(|text| !text.chars().any(char::is_control)) {
                    self.query.push_str(text);
                    self.selected = 0;
                }
                Outcome::None
            }
        }
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
