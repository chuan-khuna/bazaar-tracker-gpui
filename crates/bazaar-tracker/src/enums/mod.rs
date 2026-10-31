//! The game's enum values, built in: the number the game stores for a tier, size, hero and so on,
//! and its name. Reading them from the game's own DLL is out of scope, so these are the values
//! the original tracker falls back to (`docs/rea/bazaar-tracker/memory-reading.md` §5).
//!
//! An unknown number keeps its raw value: [`name_or_raw`] gives the number as text.
//!
//! The control flow of the tracker compares the raw numbers below (a Skill is 1, the Stash is 1),
//! so if the game renumbers these, the names stay right only as long as the table is updated.

/// Define a module for one enum with its values and lookups.
macro_rules! game_enum {
    ($module:ident, $game_name:literal, [$($value:literal => $name:literal),* $(,)?]) => {
        pub mod $module {
            /// The enum's name in the game.
            pub const GAME_NAME: &str = $game_name;
            /// Every known value and its name.
            pub const VALUES: &[(i64, &str)] = &[$(($value, $name)),*];

            /// The name of `value`, or `None` if the game has no such value (as far as we know).
            pub fn name(value: i64) -> Option<&'static str> {
                VALUES.iter().find(|(known, _)| *known == value).map(|(_, name)| *name)
            }

            /// The name of `value`, or the number itself as text if it is unknown.
            pub fn name_or_raw(value: i64) -> String {
                name(value).map_or_else(|| value.to_string(), str::to_owned)
            }

            /// The value called `name`, if there is one.
            pub fn value_of(name: &str) -> Option<i64> {
                VALUES.iter().find(|(_, known)| *known == name).map(|(value, _)| *value)
            }
        }
    };
}

game_enum!(tier, "ETier", [0 => "Bronze", 1 => "Silver", 2 => "Gold", 3 => "Diamond", 4 => "Legendary"]);
game_enum!(rank, "ERank", [0 => "Bronze", 1 => "Silver", 2 => "Gold", 3 => "Diamond", 4 => "Legendary"]);
game_enum!(card_size, "ECardSize", [1 => "Small", 2 => "Medium", 3 => "Large"]);
game_enum!(
    card_type,
    "ECardType",
    [0 => "Item", 1 => "Skill", 2 => "CombatEncounter", 3 => "EncounterStep", 4 => "EventEncounter", 5 => "PedestalEncounter", 6 => "PvpEncounter", 7 => "SocketEffect"]
);
game_enum!(
    hero,
    "EHero",
    [0 => "Common", 1 => "Pygmalien", 2 => "Vanessa", 3 => "Stelle", 4 => "Jules", 5 => "Dooley", 6 => "Mak", 7 => "Karnok", 8 => "Hero8"]
);
game_enum!(combatant, "ECombatantId", [0 => "Player", 1 => "Opponent"]);
game_enum!(inventory_section, "EInventorySection", [0 => "Hand", 1 => "Stash"]);
game_enum!(
    container_socket,
    "EContainerSocketId",
    [0 => "Socket_0", 1 => "Socket_1", 2 => "Socket_2", 3 => "Socket_3", 4 => "Socket_4", 5 => "Socket_5", 6 => "Socket_6", 7 => "Socket_7", 8 => "Socket_8", 9 => "Socket_9"]
);
game_enum!(
    enchantment,
    "EEnchantmentType",
    [0 => "Deadly", 2 => "Fiery", 3 => "Golden", 4 => "Heavy", 5 => "Icy", 7 => "Obsidian", 8 => "Radiant", 9 => "Restorative", 10 => "Shielded", 11 => "Shiny", 13 => "Toxic", 14 => "Turbo", 15 => "Mossy"]
);
game_enum!(
    run_state,
    "ERunState",
    [0 => "Choice", 1 => "Combat", 2 => "Encounter", 3 => "EndRunDefeat", 4 => "EndRunVictory", 5 => "LevelUp", 6 => "Loot", 7 => "NewRun", 8 => "Pedestal", 9 => "PVPCombat", 10 => "Shutdown"]
);
game_enum!(
    card_tag,
    "ECardTag",
    [
        0 => "Weapon", 1 => "Property", 2 => "Food", 3 => "Potion", 4 => "Tool", 5 => "Vehicle", 6 => "Aquatic", 7 => "Friend", 8 => "Core", 9 => "Unsellable", 10 => "Unstashable", 11 => "Merchant",
        12 => "Event", 13 => "Combat", 14 => "Loot", 15 => "Ray", 16 => "Dinosaur", 17 => "Apparel", 18 => "Toy", 19 => "Tech", 20 => "Dragon", 21 => "Ingredient", 22 => "Relic", 23 => "Reagent",
        24 => "Map", 25 => "Key", 26 => "Drone"
    ]
);
game_enum!(
    card_attribute,
    "ECardAttributeType",
    [
        0 => "Ammo", 1 => "AmmoMax", 2 => "ReloadAmount", 3 => "ReloadTargets", 4 => "Cooldown", 5 => "CooldownMax", 6 => "ChargeAmount", 7 => "ChargeTargets",
        9 => "Haste", 10 => "HasteAmount", 11 => "HasteTargets", 12 => "Slow", 13 => "SlowAmount", 14 => "SlowTargets", 15 => "Freeze", 16 => "FreezeAmount", 17 => "FreezeTargets",
        18 => "BurnApplyAmount", 19 => "BurnRemoveAmount", 20 => "PoisonApplyAmount", 21 => "PoisonRemoveAmount",
        24 => "Multicast", 25 => "Lifesteal", 26 => "CritChance", 27 => "DamageAmount", 28 => "DamageCrit", 29 => "HealAmount", 30 => "HealCrit",
        31 => "JoyApplyAmount", 32 => "JoyRemoveAmount", 33 => "JoyCrit", 34 => "ShieldApplyAmount", 35 => "ShieldRemoveAmount", 36 => "ShieldCrit",
        37 => "BuyPrice", 38 => "SellPrice", 39 => "Counter",
        40 => "Custom_0", 41 => "Custom_1", 42 => "Custom_2", 43 => "Custom_3", 44 => "Custom_4", 45 => "Custom_5", 46 => "Custom_6", 47 => "Custom_7", 48 => "Custom_8",
        49 => "ForceUseTargets", 50 => "EnchantTargets", 51 => "UpgradeTargets", 52 => "DisableTargets", 53 => "BurnCrit", 54 => "PoisonCrit", 55 => "DestroyTargets",
        56 => "RegenApplyAmount", 57 => "RegenRemoveAmount", 58 => "RegenCrit", 59 => "TransformTargets",
        60 => "FlatCooldownReduction", 61 => "PercentCooldownReduction", 62 => "EnchantRemoveTargets", 63 => "Flying", 64 => "FlyingTargets",
        65 => "PercentChargeReduction", 66 => "PercentHasteReduction", 67 => "PercentSlowReduction", 68 => "PercentFreezeReduction", 69 => "DestroyImmunity",
        70 => "QuestCompletedCount", 71 => "Quest_1", 72 => "Quest_2", 73 => "Quest_3", 74 => "Quest_4", 75 => "Quest_5", 76 => "Quest_6", 77 => "Quest_7", 78 => "Quest_8", 79 => "Quest_9",
        80 => "Quest_10", 81 => "Quest_11", 82 => "Quest_12", 90 => "Heated", 91 => "Chilled", 99 => "RepairTargets"
    ]
);
// The original has no full table for this enum, only the keys it reads from a player's attributes.
game_enum!(player_attribute, "EPlayerAttributeType", [4 => "Gold", 5 => "Income", 9 => "Prestige", 10 => "Health", 11 => "HealthMax", 15 => "Level"]);

/// The raw numbers the tracker's own logic compares against.
pub mod known {
    pub const CARD_TYPE_ITEM: i64 = 0;
    pub const CARD_TYPE_SKILL: i64 = 1;
    pub const CARD_TYPE_ENCOUNTER_STEP: i64 = 3;
    pub const CARD_TYPE_SOCKET_EFFECT: i64 = 7;
    pub const SECTION_HAND: i64 = 0;
    pub const SECTION_STASH: i64 = 1;
    pub const COMBATANT_PLAYER: i64 = 0;
    pub const COMBATANT_OPPONENT: i64 = 1;
    pub const RUN_STATE_END_RUN_DEFEAT: i64 = 3;
    pub const RUN_STATE_END_RUN_VICTORY: i64 = 4;
}

/// What a card's instance id says it is, from its prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Item,
    Encounter,
    EncounterStep,
    Skill,
    Combat,
    Unknown,
}

impl CardKind {
    /// `itm_` is an item, `enc_` an encounter, `ste_` an encounter step, `skl_` a skill, `com_` and
    /// `pvp_` a combat; anything else is unknown.
    pub fn from_instance_id(id: &str) -> Self {
        match id.get(..4) {
            Some("itm_") => Self::Item,
            Some("enc_") => Self::Encounter,
            Some("ste_") => Self::EncounterStep,
            Some("skl_") => Self::Skill,
            Some("com_" | "pvp_") => Self::Combat,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Item => "Item",
            Self::Encounter => "Encounter",
            Self::EncounterStep => "EncounterStep",
            Self::Skill => "Skill",
            Self::Combat => "Combat",
            Self::Unknown => "Unknown",
        }
    }
}

#[cfg(test)]
#[path = "enums_tests.rs"]
mod tests;
