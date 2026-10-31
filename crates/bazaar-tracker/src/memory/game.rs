//! The real [`GameMemory`]: The Bazaar's state, read from the running game through `unityspy-rs`
//! with the field paths of `docs/rea/bazaar-tracker/memory-reading.md` §2.
//!
//! - All memory access is serialised behind one lock; a [`BazaarMemory`] handle is cheap to clone.
//! - While the game is not attached, [`BazaarMemory::tick`] tries to attach (the connection loop
//!   calls it every 500 ms). It detaches when the process dies, or after five board reads in a row
//!   fail, and reports each change.
//! - A value the game has not set yet (no run, a class not initialised, a null reference) is
//!   `Ok(None)`: "not available yet", which never detaches.
//!
//! The paths here have been checked against the notes only. They must be checked by hand against
//! the live game (`cargo run -p bazaar-tracker --example snapshot`).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use unityspy_rs::error::Error;
use unityspy_rs::mem::process::Process;
use unityspy_rs::mem::reader::MemoryExt;
use unityspy_rs::mono::Mono;
use unityspy_rs::mono::class::Class;
use unityspy_rs::mono::object::Object;
use unityspy_rs::mono::value::Value;

use super::{BoardState, CombatFrames, CombatResult, GameMemory, Item, MemoryError, MemoryResult, PlayerRank, PlayerStats, PvpOpponent, RunInit, Snapshot};
use crate::enums::{card_attribute, card_size, card_tag, card_type, container_socket, enchantment, hero, known, player_attribute, rank, run_state, tier};

/// The game's process name.
pub const PROCESS_NAME: &str = "TheBazaar";
/// The assembly the game's own types live in.
pub const GAME_ASSEMBLY: &str = "TheBazaarRuntime";
/// How often a disconnected reader tries to attach again.
pub const RECONNECT_INTERVAL: Duration = Duration::from_millis(500);
/// How many board reads in a row may fail before the reader detaches and starts again.
pub const MAX_BOARD_FAILURES: u32 = 5;
/// How long to wait before looking again for an optional class that was missing.
const OPTIONAL_CLASS_RETRY: Duration = Duration::from_secs(5);

type Read<T> = Result<T, Error>;

/// A change in whether the game is attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionChange {
    Connected { pid: u32, unity_version: String },
    Disconnected { reason: String },
}

/// Counts failed board reads and says when to give up and detach.
#[derive(Debug, Default)]
pub struct BoardFailures {
    in_a_row: u32,
}

impl BoardFailures {
    /// Record a board read. Returns `true` once [`MAX_BOARD_FAILURES`] failures have come in a row.
    /// A read that got a board, or "not available yet", is not a failure.
    pub fn record(&mut self, failed: bool) -> bool {
        if failed {
            self.in_a_row += 1;
        } else {
            self.in_a_row = 0;
        }
        self.in_a_row >= MAX_BOARD_FAILURES
    }

    pub fn reset(&mut self) {
        self.in_a_row = 0;
    }
}

struct Attached {
    mono: Mono<Process>,
    image: u64,
    pid: u32,
    data: Arc<Class>,
    app_state: Option<Arc<Class>>,
    board_manager: Option<Arc<Class>>,
    client_cache: Option<Arc<Class>>,
    optional_retry_at: Instant,
}

struct Inner {
    process_name: String,
    attached: Option<Attached>,
    failures: BoardFailures,
    last_connect_error: Option<String>,
    changes: Option<Sender<ConnectionChange>>,
}

/// The Bazaar's memory. Clones share the one connection and the one lock.
#[derive(Clone)]
pub struct BazaarMemory {
    inner: Arc<Mutex<Inner>>,
}

impl BazaarMemory {
    pub fn new() -> Self {
        Self::for_process(PROCESS_NAME)
    }

    /// Read another process than the game (the name without `.exe`).
    pub fn for_process(name: &str) -> Self {
        let inner = Inner { process_name: name.to_owned(), attached: None, failures: BoardFailures::default(), last_connect_error: None, changes: None };
        Self { inner: Arc::new(Mutex::new(inner)) }
    }

    /// Send every connection change to `sender`.
    pub fn report_changes_to(&self, sender: Sender<ConnectionChange>) {
        self.lock().changes = Some(sender);
    }

    pub fn is_connected(&self) -> bool {
        self.lock().attached.is_some()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// One step of the connection loop: attach if detached, detach if the process has gone.
    pub fn tick(&self) {
        let mut inner = self.lock();
        if let Some(attached) = &inner.attached {
            if !attached.mono.memory().is_running() {
                inner.detach("the game process exited");
            }
            return;
        }
        match connect(&inner.process_name) {
            Ok(attached) => {
                let change = ConnectionChange::Connected { pid: attached.pid, unity_version: attached.mono.offsets().name.to_owned() };
                inner.attached = Some(attached);
                inner.failures.reset();
                inner.last_connect_error = None;
                inner.send(change);
            }
            Err(error) => {
                // A failure is remembered, not repeated: it is the same until it changes.
                let message = error.to_string();
                if inner.last_connect_error.as_deref() != Some(&message) {
                    tracing::info!("could not attach to the game: {message}");
                    inner.last_connect_error = Some(message);
                }
            }
        }
    }

    /// Run [`Self::tick`] every [`RECONNECT_INTERVAL`] until `stop` is set.
    pub fn run_connection_loop(&self, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
            self.tick();
            let mut slept = Duration::ZERO;
            while slept < RECONNECT_INTERVAL && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(25));
                slept += Duration::from_millis(25);
            }
        }
    }

    /// Run `read` against the attached game. Errors are classified: "not available yet" is `None`;
    /// a dead process detaches; other failures are reported. `is_board` marks the read that counts
    /// towards the five-strikes rule.
    fn with_game<T>(&self, is_board: bool, read: impl FnOnce(&mut Attached) -> Read<Option<T>>) -> MemoryResult<T> {
        let mut inner = self.lock();
        let Some(attached) = inner.attached.as_mut() else { return Err(MemoryError::Disconnected) };
        attached.refresh_optional_classes();
        let result = lenient(read(attached));
        let process_alive = attached.mono.memory().is_running();
        match result {
            Ok(value) => {
                if is_board {
                    inner.failures.record(false);
                }
                Ok(value)
            }
            Err(_) if !process_alive => {
                inner.detach("the game process exited");
                Err(MemoryError::Disconnected)
            }
            Err(error) => {
                if is_board && inner.failures.record(true) {
                    inner.detach("five board reads in a row failed");
                    return Err(MemoryError::Disconnected);
                }
                Err(MemoryError::Failed(error.to_string()))
            }
        }
    }
}

impl Default for BazaarMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Inner {
    fn send(&self, change: ConnectionChange) {
        if let Some(sender) = &self.changes {
            let _ = sender.send(change);
        }
    }

    fn detach(&mut self, reason: &str) {
        if self.attached.take().is_some() {
            self.failures.reset();
            self.send(ConnectionChange::Disconnected { reason: reason.to_owned() });
        }
    }
}

/// "Not initialised" is "not available yet", not a failure.
fn lenient<T>(result: Read<Option<T>>) -> Read<Option<T>> {
    match result {
        Err(Error::NotInitialised(_)) => Ok(None),
        other => other,
    }
}

fn connect(process_name: &str) -> Read<Attached> {
    let process = Process::open_by_name(process_name)?;
    let pid = process.pid();
    let (mono, _) = Mono::attach(process)?;
    let image = mono.assembly(GAME_ASSEMBLY)?.image;
    // `TheBazaar.Data` is required; the game may not have loaded it yet, which fails this attempt
    // and is retried on the next tick.
    let data = mono.find_class(image, "TheBazaar.Data")?;
    let mut attached = Attached { mono, image, pid, data, app_state: None, board_manager: None, client_cache: None, optional_retry_at: Instant::now() };
    attached.refresh_optional_classes();
    Ok(attached)
}

/// The compiler's name for an auto-property's storage.
fn prop(name: &str) -> String {
    format!("<{name}>k__BackingField")
}

/// A `Guid` struct's 16 bytes, as the usual text.
pub fn guid_from_bytes(bytes: &[u8; 16]) -> String {
    let a = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let b = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    let c = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
    format!(
        "{a:08x}-{b:04x}-{c:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Whether a rank record read at some offset looks real: season 1-10000, rank 0-16, division
/// 1-10, rating 0-1,000,000 (the original's checks for its raw scan).
pub fn plausible_rank(season: u32, rank: u32, division: u32, rating: u32) -> bool {
    (1..=10_000).contains(&season) && rank <= 16 && (1..=10).contains(&division) && rating <= 1_000_000
}

impl Attached {
    /// Look again for optional classes that were not there, at most every few seconds.
    fn refresh_optional_classes(&mut self) {
        if self.app_state.is_some() && self.board_manager.is_some() && self.client_cache.is_some() {
            return;
        }
        if Instant::now() < self.optional_retry_at {
            return;
        }
        self.optional_retry_at = Instant::now() + OPTIONAL_CLASS_RETRY;
        if self.app_state.is_none() {
            self.app_state = self.mono.find_class(self.image, "TheBazaar.AppState").ok();
        }
        if self.board_manager.is_none() {
            self.board_manager = self.mono.find_class(self.image, "BoardManager").ok();
        }
        if self.client_cache.is_none() {
            self.client_cache = self.mono.find_class(self.image, "TheBazaar.ClientCache").ok();
        }
    }

    // --- small readers -------------------------------------------------------------------

    /// A static of `class`, as an object (`None` when null or not initialised).
    fn static_object(&self, class: &Class, field: &str) -> Read<Option<Object>> {
        match self.mono.read_static(class, field) {
            Ok(Value::Object(object)) => Ok(Some(object)),
            Ok(Value::Null) | Err(Error::NotInitialised(_)) => Ok(None),
            Ok(other) => Err(Error::Unexpected(format!("{}.{field} is not an object: {other:?}", class.full_name))),
            Err(error) => Err(error),
        }
    }

    /// The first of `names` that `object` has as a field.
    fn field_any(&self, object: &Object, names: &[&str]) -> Read<Value> {
        for name in names {
            match self.mono.read_field(object, name) {
                Err(Error::FieldNotFound { .. }) => {}
                other => return other,
            }
        }
        Err(Error::FieldNotFound { class: object.class.full_name.clone(), field: names.join("|") })
    }

    fn object_field(&self, object: &Object, property: &str) -> Read<Option<Object>> {
        match self.field_any(object, &[&prop(property), property])? {
            Value::Object(object) => Ok(Some(object)),
            _ => Ok(None),
        }
    }

    fn int_field(&self, object: &Object, property: &str) -> Read<Option<i32>> {
        Ok(self.field_any(object, &[&prop(property), property])?.as_i64().map(|value| value as i32))
    }

    fn string_field(&self, object: &Object, property: &str) -> Read<Option<String>> {
        Ok(self.field_any(object, &[&prop(property), property])?.as_str().map(str::to_owned))
    }

    /// The text of a `Guid` or id struct field, or a string.
    fn guid_or_text(&self, value: &Value) -> Read<Option<String>> {
        match value {
            Value::Str(text) => Ok(Some(text.clone())),
            Value::Struct(inner) => {
                let mut bytes = [0u8; 16];
                if inner.class.full_name == "System.Guid" {
                    self.mono.memory().read_into(inner.address, &mut bytes)?;
                    return Ok(Some(guid_from_bytes(&bytes)));
                }
                // A struct wrapping the text, such as an instance id.
                for name in [&prop("Value")[..], "Value", "value", "_value"] {
                    if let Ok(Value::Str(text)) = self.mono.read_struct_field(inner, name) {
                        return Ok(Some(text));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// A `Nullable<enum>` or `Nullable<int>` stored inline: a flag byte, then the 32-bit value.
    fn nullable_int(&self, value: &Value) -> Read<Option<i64>> {
        let Value::Struct(inner) = value else { return Ok(value.as_i64()) };
        if self.mono.memory().read_u8(inner.address)? == 0 {
            return Ok(None);
        }
        Ok(Some(i64::from(self.mono.memory().read_i32(inner.address + 4)?)))
    }

    /// A `List<string>`, skipping null and empty entries.
    fn string_list(&self, object: &Object) -> Read<Vec<String>> {
        Ok(self.mono.read_list(object)?.into_iter().filter_map(|value| value.as_str().map(str::to_owned)).filter(|text| !text.is_empty()).collect())
    }

    /// A `Dictionary<int-ish, int>`: read with the entry class's fields, or, if its generic types
    /// are not resolved, at the layout the original used (hash at 0, key at 8, value at 12).
    fn int_dictionary(&self, object: &Object) -> Read<Vec<(i64, i64)>> {
        if let Ok(pairs) = self.mono.read_dictionary(object) {
            return Ok(pairs.iter().filter_map(|(key, value)| Some((key.as_i64()?, value.as_i64()?))).collect());
        }
        let count = self.field_any(object, &["_count", "count"])?.as_i64().unwrap_or(0).clamp(0, 1 << 16) as u64;
        let Value::Array(entries) = self.field_any(object, &["_entries", "entries"])? else { return Ok(Vec::new()) };
        let memory = self.mono.memory();
        let mut pairs = Vec::new();
        for i in 0..count.min(entries.length as u64) {
            let at = entries.data() + i * entries.stride as u64;
            if memory.read_i32(at)? >= 0 {
                pairs.push((i64::from(memory.read_i32(at + 8)?), i64::from(memory.read_i32(at + 12)?)));
            }
        }
        Ok(pairs)
    }

    /// A `HashSet<int-ish>`: through the slot class, or at the original's layout (hash at 0, value at 8).
    fn int_set(&self, object: &Object) -> Read<Vec<i64>> {
        if let Ok(values) = self.mono.read_hash_set(object) {
            return Ok(values.iter().filter_map(Value::as_i64).collect());
        }
        let count = self.field_any(object, &["_count", "m_count", "count"])?.as_i64().unwrap_or(0).clamp(0, 1 << 16) as u64;
        let Value::Array(slots) = self.field_any(object, &["_slots", "m_slots", "_entries", "entries"])? else { return Ok(Vec::new()) };
        let memory = self.mono.memory();
        let mut values = Vec::new();
        for i in 0..count.min(slots.length as u64) {
            let at = slots.data() + i * slots.stride as u64;
            if memory.read_i32(at)? >= 0 {
                values.push(i64::from(memory.read_i32(at + 8)?));
            }
        }
        Ok(values)
    }

    // --- cards ---------------------------------------------------------------------------

    fn read_card(&self, card: &Object) -> Read<Option<Card>> {
        let template = self.field_any(card, &[&prop("TemplateId")])?;
        let instance = self.field_any(card, &[&prop("InstanceId")])?;
        let Some(instance_id) = self.guid_or_text(&instance)? else { return Ok(None) };
        let int = |property: &str| self.int_field(card, property);
        let card_type_value = int("Type")?.map(i64::from);
        let size = int("Size")?.map(i64::from);
        let tier_value = int("Tier")?.map(i64::from);
        let socket = self.nullable_int(&self.field_any(card, &[&prop("LeftSocketId")])?)?;
        let section = self.nullable_int(&self.field_any(card, &[&prop("Section")])?)?;
        let enchantment_value = match self.field_any(card, &[&prop("Enchantment")]) {
            Ok(value) => self.nullable_int(&value)?,
            Err(Error::FieldNotFound { .. }) => None,
            Err(error) => return Err(error),
        };
        let mut attributes = BTreeMap::new();
        if let Some(dictionary) = self.object_field(card, "Attributes")? {
            for (key, value) in self.int_dictionary(&dictionary)? {
                attributes.insert(card_attribute::name_or_raw(key), value as i32);
            }
        }
        let mut tags = Vec::new();
        if let Some(set) = self.object_field(card, "Tags")? {
            tags = self.int_set(&set)?.into_iter().map(card_tag::name_or_raw).collect();
        }
        let owner = match self.object_field(card, "Owner")? {
            Some(owner) => self.int_field(&owner, "CombatantId")?.map(i64::from),
            None => None,
        };
        Ok(Some(Card {
            item: Item {
                instance_id,
                template_id: self.guid_or_text(&template)?,
                card_type: card_type_value.map_or_else(|| "Unknown".to_owned(), card_type::name_or_raw),
                tier: tier_value.map_or_else(|| "Unknown".to_owned(), tier::name_or_raw),
                size: size.map_or_else(|| "Unknown".to_owned(), card_size::name_or_raw),
                socket: socket.map(container_socket::name_or_raw),
                enchantment: enchantment_value.map(enchantment::name_or_raw),
                attributes,
                tags,
            },
            card_type: card_type_value,
            section,
            owner,
        }))
    }

    // --- the board -----------------------------------------------------------------------

    fn read_board_state(&self) -> Read<Option<BoardState>> {
        let data = &*self.data;
        let Some(state) = self.static_object(data, &prop("CurrentState"))? else { return Ok(None) };
        let Some(run) = self.static_object(data, &prop("Run"))? else { return Ok(None) };

        let run_state_value = self.int_field(&state, "StateName")?.map(i64::from);
        let selection_set = match self.object_field(&state, "SelectionSet")? {
            Some(list) => self.string_list(&list)?,
            None => Vec::new(),
        };
        let is_storage_open = self.mono.read_static(data, &prop("IsStorageOpen")).ok().and_then(|value| value.as_bool()).unwrap_or(false);
        let current_encounter = match self.mono.read_static(data, &prop("CurrentEncounterId")) {
            Ok(value @ Value::Struct(_)) => self.nullable_guid(&value)?,
            _ => None,
        };

        let player = self.object_field(&run, "Player")?;
        let opponent = self.object_field(&run, "Opponent")?;
        let mut player_snapshot = Snapshot { stats: self.read_stats(&run, player.as_ref())?, ..Snapshot::default() };
        let mut opponent_snapshot = Snapshot { stats: self.read_side_stats(opponent.as_ref())?, ..Snapshot::default() };

        let player_skill_order = self.skill_order(player.as_ref())?;
        let opponent_skill_order = self.skill_order(opponent.as_ref())?;

        let mut entities = Vec::new();
        if let Some(dictionary) = self.static_object(data, "Entities")? {
            let mut seen_encounter_templates: Vec<String> = Vec::new();
            for (_, value) in self.mono.read_dictionary(&dictionary)? {
                let Value::Object(card) = value else { continue };
                let Some(card) = self.read_card(&card)? else { continue };
                entities.push(card.item.clone());
                let is_skill = card.card_type == Some(known::CARD_TYPE_SKILL);
                match card.owner {
                    Some(known::COMBATANT_PLAYER) if is_skill => player_snapshot.skills.push(card.item),
                    Some(known::COMBATANT_PLAYER) if card.section == Some(known::SECTION_STASH) => player_snapshot.stash.push(card.item),
                    Some(known::COMBATANT_PLAYER) => player_snapshot.board.push(card.item),
                    Some(known::COMBATANT_OPPONENT) if is_skill => opponent_snapshot.skills.push(card.item),
                    Some(known::COMBATANT_OPPONENT) => {
                        // Encounter cards other than the current one are leftovers.
                        let always_kept = matches!(card.card_type, Some(known::CARD_TYPE_ITEM | known::CARD_TYPE_ENCOUNTER_STEP | known::CARD_TYPE_SOCKET_EFFECT));
                        let current = current_encounter.as_deref().is_some_and(|id| card.item.template_id.as_deref() == Some(id));
                        if !always_kept && !current {
                            continue;
                        }
                        if !always_kept {
                            let template = card.item.template_id.clone().unwrap_or_default();
                            if seen_encounter_templates.contains(&template) {
                                continue;
                            }
                            seen_encounter_templates.push(template);
                        }
                        if matches!(card.section, None | Some(known::SECTION_HAND)) {
                            opponent_snapshot.board.push(card.item);
                        }
                    }
                    _ => {}
                }
            }
        }
        sort_by_order(&mut player_snapshot.skills, &player_skill_order);
        sort_by_order(&mut opponent_snapshot.skills, &opponent_skill_order);

        Ok(Some(BoardState {
            player: player_snapshot,
            opponent: opponent_snapshot,
            run_state: run_state_value.map(run_state::name_or_raw),
            selection_set,
            is_storage_open,
            entities,
        }))
    }

    fn nullable_guid(&self, value: &Value) -> Read<Option<String>> {
        let Value::Struct(inner) = value else { return Ok(None) };
        if self.mono.memory().read_u8(inner.address)? == 0 {
            return Ok(None);
        }
        let mut bytes = [0u8; 16];
        self.mono.memory().read_into(inner.address + 4, &mut bytes)?;
        Ok(Some(guid_from_bytes(&bytes)))
    }

    fn skill_order(&self, side: Option<&Object>) -> Read<Vec<String>> {
        let Some(side) = side else { return Ok(Vec::new()) };
        let Some(list) = self.object_field(side, "Skills")? else { return Ok(Vec::new()) };
        let mut ids = Vec::new();
        for value in self.mono.read_list(&list)? {
            if let Value::Object(card) = value
                && let Ok(instance) = self.field_any(&card, &[&prop("InstanceId")]) {
                    ids.extend(self.guid_or_text(&instance)?);
                }
        }
        Ok(ids)
    }

    fn read_side_stats(&self, side: Option<&Object>) -> Read<PlayerStats> {
        let mut stats = PlayerStats::default();
        let Some(side) = side else { return Ok(stats) };
        if let Some(attributes) = self.object_field(side, "Attributes")? {
            for (key, value) in self.int_dictionary(&attributes)? {
                let value = Some(value as i32);
                match player_attribute::name(key) {
                    Some("Health") => stats.health = value,
                    Some("HealthMax") => stats.health_max = value,
                    Some("Gold") => stats.gold = value,
                    Some("Prestige") => stats.prestige = value,
                    Some("Income") => stats.income = value,
                    Some("Level") => stats.level = value,
                    _ => {}
                }
            }
        }
        stats.hero = self.int_field(side, "Hero")?.filter(|&id| id != 0).map(|id| hero::name_or_raw(i64::from(id)));
        Ok(stats)
    }

    fn read_stats(&self, run: &Object, player: Option<&Object>) -> Read<PlayerStats> {
        let mut stats = self.read_side_stats(player)?;
        stats.day = self.int_field(run, "Day")?;
        stats.hour = self.int_field(run, "Hour")?;
        stats.victories = self.int_field(run, "Victories")?;
        stats.losses = self.int_field(run, "Losses")?;
        Ok(stats)
    }

    // --- the other reads -----------------------------------------------------------------

    fn read_run_init_state(&self) -> Read<Option<RunInit>> {
        if let Some(app_state) = &self.app_state
            && let Some(init) = self.static_object(app_state, "_cachedRunInitialized")? {
                let run_id = self.string_field(&init, "RunId")?;
                if run_id.is_some() {
                    return Ok(Some(RunInit {
                        run_id,
                        play_mode: self.string_field(&init, "PlayMode")?,
                        build_id: self.string_field(&init, "BuildId")?,
                        environment: self.string_field(&init, "Environment")?,
                    }));
                }
            }
        // Fall back to the id on the run object itself.
        let Some(run) = self.static_object(&self.data, &prop("Run"))? else { return Ok(None) };
        for name in [prop("Id"), prop("RunId"), "Id".to_owned(), "RunId".to_owned(), "_id".to_owned()] {
            match self.mono.read_field(&run, &name) {
                Ok(value) => {
                    if let Some(run_id) = self.guid_or_text(&value)? {
                        return Ok(Some(RunInit { run_id: Some(run_id), ..RunInit::default() }));
                    }
                }
                Err(Error::FieldNotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }

    /// The last combat simulation: `BoardManager`'s singleton's `_lastCombatSim`, or `AppState`'s cached one.
    fn combat_sim(&self) -> Read<Option<Object>> {
        if let Some(board_manager) = &self.board_manager
            && let Some(instance) = self.static_object(board_manager, "Instance")?
                && let Value::Object(sim) = self.mono.read_field(&instance, "_lastCombatSim")? {
                    return Ok(Some(sim));
                }
        if let Some(app_state) = &self.app_state
            && let Some(cached) = self.static_object(app_state, "_cachedCombatSim")?
                && let Some(sim) = self.object_field(&cached, "Data")? {
                    return Ok(Some(sim));
                }
        Ok(None)
    }

    fn frame_count(&self, sim: &Object) -> Read<Option<i32>> {
        match self.field_any(sim, &["Frames", &prop("Frames")])? {
            Value::Object(frames) => Ok(self.mono.read_field(&frames, "_size")?.as_i64().map(|size| size as i32)),
            _ => Ok(None),
        }
    }

    fn read_pvp_opponent_state(&self) -> Read<Option<PvpOpponent>> {
        let Some(opponent) = self.static_object(&self.data, &prop("SimPvpOpponent"))? else { return Ok(None) };
        Ok(Some(PvpOpponent {
            name: self.string_field(&opponent, "Name")?,
            hero: self.int_field(&opponent, "Hero")?.map(|id| hero::name_or_raw(i64::from(id))),
            rating: self.int_field(&opponent, "Rating")?,
            level: self.int_field(&opponent, "Level")?,
        }))
    }

    fn read_selected_hero_state(&self) -> Read<Option<String>> {
        if let Some(run) = self.static_object(&self.data, &prop("Run"))?
            && let Some(player) = self.object_field(&run, "Player")?
                && let Some(id) = self.int_field(&player, "Hero")?.filter(|&id| id > 0) {
                    return Ok(Some(hero::name_or_raw(i64::from(id))));
                }
        if let Some(cache) = &self.client_cache
            && let Some(config) = self.static_object(cache, "RunConfig")?
                && let Value::Struct(value) = self.mono.read_field(&config, "_value")?
                    && let Some(id) = self.mono.read_struct_field(&value, &prop("Hero")).ok().and_then(|v| v.as_i64()).filter(|&id| id > 0) {
                        return Ok(Some(hero::name_or_raw(id)));
                    }
        Ok(None)
    }

    fn read_rank_state(&self) -> Read<Option<PlayerRank>> {
        let Some(cache) = &self.client_cache else { return Ok(None) };
        let Some(observable) = self.static_object(cache, "Rank")? else { return Ok(None) };
        let value_field = self.mono.find_field(&observable.class, "_value", None)?;
        let struct_class = self.mono.class_at(value_field.type_data())?;
        let rel = |name: &str| -> Read<u64> {
            let field = self.mono.find_field(&struct_class, &prop(name), None)?;
            Ok((field.offset as u64).wrapping_sub(16))
        };
        let (season_at, rank_at, division_at, rating_at) = (rel("SeasonID")?, rel("Rank")?, rel("Division")?, rel("Rating")?);
        // The offset of an inline struct inside a generic observable is unreliable, so scan a few
        // places for one that looks like a rank record.
        let memory = self.mono.memory();
        for start in (16..=48u64).step_by(4) {
            let base = observable.address + start;
            let (season, rank_id, division, rating) = (memory.read_u32(base + season_at)?, memory.read_u32(base + rank_at)?, memory.read_u32(base + division_at)?, memory.read_u32(base + rating_at)?);
            if plausible_rank(season, rank_id, division, rating) {
                return Ok(Some(PlayerRank { rating: rating as i32, rank: rank::name_or_raw(i64::from(rank_id)), division: division as i32 }));
            }
        }
        Ok(None)
    }
}

/// One card as the board read sees it, with what is needed to put it in the right list.
struct Card {
    item: Item,
    card_type: Option<i64>,
    section: Option<i64>,
    owner: Option<i64>,
}

fn sort_by_order(items: &mut [Item], order: &[String]) {
    items.sort_by_key(|item| order.iter().position(|id| *id == item.instance_id).unwrap_or(usize::MAX));
}

impl GameMemory for BazaarMemory {
    fn read_board(&mut self) -> MemoryResult<BoardState> {
        self.with_game(true, |game| game.read_board_state())
    }

    fn read_run_init(&mut self) -> MemoryResult<RunInit> {
        self.with_game(false, |game| game.read_run_init_state())
    }

    fn read_combat_result(&mut self) -> MemoryResult<CombatResult> {
        self.with_game(false, |game| {
            let Some(sim) = game.combat_sim()? else { return Ok(None) };
            Ok(Some(CombatResult {
                winner: game.int_field(&sim, "Winner")?,
                loser: game.int_field(&sim, "Loser")?,
                frame_count: game.frame_count(&sim)?,
            }))
        })
    }

    fn read_combat_frames(&mut self) -> MemoryResult<CombatFrames> {
        self.with_game(false, |game| {
            let Some(sim) = game.combat_sim()? else { return Ok(None) };
            Ok(game.frame_count(&sim)?.map(|frame_count| CombatFrames { frame_count }))
        })
    }

    fn read_pvp_opponent(&mut self) -> MemoryResult<PvpOpponent> {
        self.with_game(false, |game| game.read_pvp_opponent_state())
    }

    fn read_selected_hero(&mut self) -> MemoryResult<String> {
        self.with_game(false, |game| game.read_selected_hero_state())
    }

    fn read_player_rank(&mut self) -> MemoryResult<PlayerRank> {
        self.with_game(false, |game| game.read_rank_state())
    }
}

#[cfg(test)]
#[path = "game_tests.rs"]
mod tests;
