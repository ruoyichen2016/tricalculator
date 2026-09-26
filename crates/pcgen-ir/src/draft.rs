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
}

/// Persisted draft inputs parsed from a `.pcg`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PcgDraft {
    pub name: String,
    pub race_id: String,
    pub campaign: String,
    #[serde(default)]
    pub alignment: String,
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
    #[serde(default)]
    pub abilities: Vec<(String, String)>,
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub advancement: Vec<AdvancementFrame>,
}
