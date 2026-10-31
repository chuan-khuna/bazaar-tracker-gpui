//! Turning a line of `Player.log` into a typed log line. The patterns are those of the original
//! tracker (`docs/rea/bazaar-tracker/log-and-events.md` §2), each guarded by a plain substring check
//! so most lines never reach a regex.

use std::sync::LazyLock;

use regex::Regex;

/// A line of `Player.log` the tracker cares about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogLine {
    /// `[AppState] State changed from [A] to [B]`. The first state of a session is `null`.
    StateChanged { from: String, to: String },
    /// `[AppState] Selected skill skl_… to socket SkillSocket_0`.
    SkillSelected { skill_id: String, socket: String },
    /// `[GameSimHandler] GameSim cards: id=…, dealt=3, spawned=0, disposed=0`.
    GameSimSummary { dealt: u32, spawned: u32, disposed: u32 },
    /// `[BoardManager] Card Purchased: InstanceId: … - TemplateId… - Target:… - Section…`.
    CardPlaced { instance_id: String, template_id: String, target: String, section: String },
    /// `[HttpGameClient] Command completed: type=SellCardCommand, …, result=success`.
    CommandCompleted { command: String, succeeded: bool },
    /// `[GameMessageHandler] Handled: type=NetMessageRunInitialized, …`: a run begins.
    RunInitialized,
    /// `[CombatSimHandler] Combat prepared: frames=659, …`. `None` if the count is missing.
    CombatPrepared { frames: Option<u32> },
    /// `[CombatSimHandler] Combat simulation completed: …`. The tracker has no use for it yet.
    CombatCompleted,
}

static TIMESTAMP_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[\d{2}:\d{2}:\d{2}\.\d{3}\]\s*").unwrap());
static STATE_CHANGED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"State changed from \[([A-Za-z0-9_]+)\] to \[([A-Za-z0-9_]+)\]").unwrap());
static SKILL_SELECTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Selected skill ([\w-]+) to socket ([\w-]+)").unwrap());
static GAME_SIM_SUMMARY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"GameSim cards: id=\S*, dealt=(\d+), spawned=(\d+), disposed=(\d+)").unwrap());
static CARD_PLACED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"InstanceId:\s*([\w-]+)\s*-\s*TemplateId([0-9a-fA-F-]+)\s*-\s*Target:(\S+)\s*-\s*Section([A-Za-z]+)").unwrap());
static COMMAND_COMPLETED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Command completed: type=(\w+Command),.*\bresult=(\w+)").unwrap());
static COMBAT_PREPARED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Combat prepared: frames=(\d+)").unwrap());

/// Remove the `[hh:mm:ss.fff]` prefix and surrounding whitespace. The game's clock is never used:
/// Events are stamped when they are made.
pub fn strip_timestamp(raw: &str) -> &str {
    let rest = TIMESTAMP_PREFIX.find(raw).map_or(raw, |found| &raw[found.end()..]);
    rest.trim()
}

/// Parse one raw line, or `None` if it is not one the tracker uses. The checks run in a fixed order
/// and the first that matches wins.
pub fn parse_line(raw: &str) -> Option<LogLine> {
    let line = strip_timestamp(raw);

    if line.contains("] State changed from [")
        && let Some(found) = STATE_CHANGED.captures(line)
    {
        return Some(LogLine::StateChanged { from: found[1].to_owned(), to: found[2].to_owned() });
    }
    if line.contains("] Selected skill ")
        && let Some(found) = SKILL_SELECTED.captures(line)
    {
        return Some(LogLine::SkillSelected { skill_id: found[1].to_owned(), socket: found[2].to_owned() });
    }
    if line.contains("GameSim cards:")
        && let Some(found) = GAME_SIM_SUMMARY.captures(line)
        && let (Ok(dealt), Ok(spawned), Ok(disposed)) = (found[1].parse(), found[2].parse(), found[3].parse())
    {
        return Some(LogLine::GameSimSummary { dealt, spawned, disposed });
    }
    if line.contains("Card Purchased:")
        && let Some(found) = CARD_PLACED.captures(line)
    {
        return Some(LogLine::CardPlaced {
            instance_id: found[1].to_owned(),
            template_id: found[2].to_owned(),
            target: found[3].to_owned(),
            section: found[4].to_owned(),
        });
    }
    if line.contains("Command completed: type=")
        && let Some(found) = COMMAND_COMPLETED.captures(line)
    {
        return Some(LogLine::CommandCompleted { command: found[1].to_owned(), succeeded: &found[2] == "success" });
    }
    if line.contains("Handled: type=NetMessageRunInitialized") {
        return Some(LogLine::RunInitialized);
    }
    if line.contains("[CombatSimHandler]") && line.contains("Combat prepared:") {
        let frames = COMBAT_PREPARED.captures(line).and_then(|found| found[1].parse().ok());
        return Some(LogLine::CombatPrepared { frames });
    }
    if line.contains("[CombatSimHandler]") && line.contains("Combat simulation completed") {
        return Some(LogLine::CombatCompleted);
    }
    None
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
