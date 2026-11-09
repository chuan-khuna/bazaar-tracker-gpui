//! The Advisor's own question (`ask_user`), as tau-app's chat shows and answers it
//! (`AskUserCard.tsx`): the question, its choices (one, or several with `multiple`), and the
//! player's own words when `allowFreeText`. The answer is an ordinary message, and the question is
//! answered by the player's next message, which also rebuilds the card when history loads.

use std::collections::BTreeSet;

use serde_json::Value;

/// One choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

/// An `ask_user` call's arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskUser {
    pub question: String,
    pub options: Vec<Choice>,
    /// Several choices may be picked; the answer lists them one per line.
    pub multiple: bool,
    pub allow_free_text: bool,
}

impl AskUser {
    /// From the call's arguments; none when they don't have that shape (drawn as a tool line).
    /// Options that aren't `{id, label}` strings are passed over; `allowFreeText` defaults to true.
    pub fn parse(arguments: &Value) -> Option<Self> {
        let question = arguments.get("question")?.as_str()?.to_owned();
        let options: Vec<Choice> = arguments
            .get("options")?
            .as_array()?
            .iter()
            .filter_map(|option| Some(Choice { id: option.get("id")?.as_str()?.to_owned(), label: option.get("label")?.as_str()?.to_owned() }))
            .collect();
        if options.is_empty() {
            return None;
        }
        let multiple = arguments.get("multiple").and_then(Value::as_bool) == Some(true);
        let allow_free_text = arguments.get("allowFreeText").and_then(Value::as_bool) != Some(false);
        Some(Self { question, options, multiple, allow_free_text })
    }

    /// The message that answers it: the picked choice's label (or the typed text when none is
    /// picked) for one choice; for several, a `- <label>` line per picked choice in the card's
    /// order, then the typed text as a last line. Typed text counts only with `allowFreeText`.
    /// Empty when there is nothing to send.
    pub fn answer(&self, picked: &BTreeSet<String>, typed: &str) -> String {
        let typed = if self.allow_free_text { typed.trim() } else { "" };
        let labels = self.options.iter().filter(|option| picked.contains(&option.id)).map(|option| option.label.as_str());
        if !self.multiple {
            return labels.into_iter().next().unwrap_or(typed).to_owned();
        }
        let mut lines: Vec<String> = labels.map(|label| format!("- {label}")).collect();
        if !typed.is_empty() {
            lines.push(typed.to_owned());
        }
        lines.join("\n")
    }

    /// What an answer picked, and anything else it said. One choice: the answer names a choice
    /// only when it is exactly that label. Several: read line by line, with or without `- `.
    pub fn read(&self, answer: &str) -> Answered {
        let id_of = |label: &str| self.options.iter().find(|option| option.label == label).map(|option| option.id.clone());
        if !self.multiple {
            return match id_of(answer.trim()) {
                Some(id) => Answered { picked: BTreeSet::from([id]), rest: String::new() },
                None => Answered { picked: BTreeSet::new(), rest: answer.to_owned() },
            };
        }
        let mut picked = BTreeSet::new();
        let mut rest = Vec::new();
        for line in answer.split('\n').map(str::trim).filter(|line| !line.is_empty()) {
            let label = line.strip_prefix('-').map_or(line, str::trim_start);
            match id_of(label) {
                Some(id) => {
                    picked.insert(id);
                }
                None => rest.push(line),
            }
        }
        Answered { picked, rest: rest.join("\n") }
    }
}

/// What an answer said, read back against the card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answered {
    /// The ids of the choices it picked.
    pub picked: BTreeSet<String>,
    /// Its other words ("You answered: …").
    pub rest: String,
}

/// The card's choices and words before Send, for the one question waiting.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    /// The `ask_user` call it answers.
    pub call_id: String,
    pub picked: BTreeSet<String>,
    pub typed: String,
}

impl Draft {
    /// A click on a choice: it toggles with several; with one it replaces the pick and the words.
    pub fn pick(&mut self, ask: &AskUser, id: &str) {
        if ask.multiple {
            if !self.picked.remove(id) {
                self.picked.insert(id.to_owned());
            }
        } else {
            self.picked = BTreeSet::from([id.to_owned()]);
            self.typed.clear();
        }
    }

    /// Words typed: with one choice they replace the pick.
    pub fn typed(&mut self, ask: &AskUser, text: String) {
        if !ask.multiple && !text.is_empty() {
            self.picked.clear();
        }
        self.typed = text;
    }
}

#[cfg(test)]
#[path = "ask_user_tests.rs"]
mod tests;
