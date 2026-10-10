//! Persisted draft inputs, independent of any PCGen file format.
//!
//! These types are the *data* a parser produces. File-format handling
//! (`.pcg` syntax, I/O) stays in `pcgen-import`; this module only models the
//! values so the engine and the importer can share them without the engine
//! depending on the compatibility layer.

use pcgen_core::AdvancementFrame;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcgClassLevel {
    pub class_id: String,
    pub level: u32,
    #[serde(default)]
    pub hit_points_gained: i32,
    /// Per-level HP rolls from `CLASSABILITIESLEVEL` records.
    #[serde(default)]
    pub hit_points_by_level: Vec<i32>,
    #[serde(default)]
    pub experience_gained: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcgSpell {
    pub name: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub class_id: Option<String>,
    #[serde(default)]
    pub times_memorized: u32,
    /// PCGen `BOOK:` value, e.g. `Known Spells` or `Prepared Spells`.
    #[serde(default)]
    pub book: Option<String>,
}

/// Persisted draft inputs parsed from a `.pcg`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PcgDraft {
    pub name: String,
    pub race_id: String,
    pub campaign: String,
    #[serde(default)]
    pub alignment: String,
    /// PCGen `GENDER:`.
    #[serde(default)]
    pub gender: String,
    /// PCGen `CHARACTERTYPE:` (`PC`, `NPC`, `Monster`).  Empty means the file did
    /// not say, and the engine's default applies.
    #[serde(default)]
    pub character_type: String,
    pub stats: BTreeMap<String, i32>,
    #[serde(default)]
    pub class_levels: Vec<PcgClassLevel>,
    #[serde(default)]
    pub skills: BTreeMap<String, f64>,
    #[serde(default)]
    pub spells: Vec<PcgSpell>,
    #[serde(default)]
    pub equipment: Vec<String>,
    /// Equipment modifiers attached to equipped items (`EQMOD=` in `.pcg`
    /// `CUSTOMIZATION` data), keyed by item name, lower-cased.
    #[serde(default)]
    pub equipment_modifiers: BTreeMap<String, Vec<String>>,
    /// Base item name for customised equipment (`BASEITEM:` in `.pcg`
    /// `CUSTOMIZATION` data), keyed by item name, lower-cased.  A customised
    /// item such as `+2 scale mail` resolves its armour bonus from
    /// `Scale Mail`.
    #[serde(default)]
    pub equipment_base_items: BTreeMap<String, String>,
    /// `OUTPUTNAME:` display name per item, keyed by item name (lower-cased).
    #[serde(default)]
    pub equipment_display_names: BTreeMap<String, String>,
    /// Where each item sits in the character's equip set, keyed by item name
    /// (lower-cased).
    ///
    /// Read from the `.pcg`'s `EQUIPSET:` rows, which name a slot (`Primary Hand`,
    /// `Two Weapons`, `Carried`, ...) and the item in it; `CALCEQUIPSET:` names the
    /// working set, and only its own rows and its children's are used.  The value
    /// is the location the slot puts a weapon in — `primary`, `secondary`, `both`,
    /// `two_hands` or `neither` — or absent for a slot the reference assigns no
    /// location to at all, which is most of them.
    #[serde(default)]
    pub equip_slots: BTreeMap<String, String>,
    #[serde(default)]
    pub abilities: Vec<(String, String)>,
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub advancement: Vec<AdvancementFrame>,
    /// Explicit choices parsed from `.pcg` (e.g. `APPLIEDTO`).
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
    /// Selected subclass per class id (`CLASS:...+SUBCLASS:...`).
    #[serde(default)]
    pub class_subclasses: BTreeMap<String, String>,
    /// Selected deity (`.pcg DEITY`).
    #[serde(default)]
    pub deity: Option<String>,
    /// Selected domains (`.pcg DOMAIN` lines).
    #[serde(default)]
    pub domains: Vec<String>,
    /// Deity-allowed domain catalog from `DEITYDOMAINS:[DOMAIN:...]`.
    #[serde(default)]
    pub deity_domains: Vec<String>,
}
