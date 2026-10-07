//! The normalised on-disk data format.
//!
//! Design: `docs/DATA_FORMAT.md`.  The short version is that PCGen's data
//! carries meaning implicitly — a class level row belongs to whichever class
//! header most recently appeared, formulas are text, one concept has several
//! spellings — and `pcgen-ir` exists to recover that meaning at runtime.  Every
//! defect this project has hit sits on that boundary.
//!
//! This format states the same facts structurally:
//!
//! * **R1** each record names its own kind (`kind`);
//! * **R2** each record names its own owner (`class` on a level record) — there
//!   is no cursor, so there is nothing to hijack;
//! * **R3** there are no modification records: `.MOD` / `.CLEAR` are resolved
//!   before writing;
//! * **R4** one spelling per concept: `Archetype` and `ACF` both become
//!   [`Record::Variant`];
//! * **R5** progression is a table, not a formula string;
//! * **R6** level is a field, never a name suffix;
//! * **R7** each record can point back at the file and line it came from;
//! * **R8** the format is versioned and validated on load.
//!
//! Stage 1 covers the class domain (classes, class levels, variants), which is
//! where the defects clustered.  Fields outside that domain are carried through
//! unchanged and will be normalised in later stages.

use pcgen_core::{ClassDefinition, ClassProgression};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// On-disk format version.  A loader must reject versions it does not know.
pub const FORMAT_VERSION: u32 = 1;

/// Where a record came from, so a wrong value can be traced to its source line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub file: String,
    pub line: u32,
}

impl Origin {
    pub fn new(file: impl Into<String>, line: u32) -> Self {
        Self {
            file: file.into(),
            line,
        }
    }
}

/// One record in a `.jsonl` file.
///
/// The `kind` tag is what makes R1 hold: a record states what it is rather than
/// having its type decided by which `.pcc` directive happened to include the
/// file it lived in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    // Boxed because a class record carries the whole definition and
    // progression, which would otherwise dwarf the other variants.
    Class(Box<ClassRecord>),
    ClassLevel(ClassLevelRecord),
    Variant(VariantRecord),
}

impl Record {
    /// The `kind` string, for diagnostics and file routing.
    pub fn kind(&self) -> &'static str {
        match self {
            Record::Class(_) => "class",
            Record::ClassLevel(_) => "class_level",
            Record::Variant(_) => "variant",
        }
    }
}

/// The grant type is `pcgen_core::ClassGrant`; the format does not define its
/// own.  It used to, which meant the persisted form and the runtime model could
/// drift apart.
pub use pcgen_core::{ClassGrant, GrantMode};

/// A class definition, minus the per-level data that now lives in
/// [`ClassLevelRecord`].
///
/// Stage 1 normalises the parts the defects clustered on: level ownership moves
/// out into `class_level` records, and progression is carried as per-level
/// tables rather than a formula string (R5).  The remaining fields are the
/// runtime [`ClassDefinition`] with its level maps cleared, carried through
/// unchanged — they still contain PCGen conventions and are flattened in
/// Stage 2.  Keeping the model type here is deliberate: the design is to
/// *persist the translated model*, not to invent a parallel schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRecord {
    pub id: String,
    pub name: String,
    /// Progression, carried as the runtime type rather than re-listed: every
    /// field here is load-bearing — dropping `levels_per_feat` alone changes a
    /// fighter's feat count.  It no longer holds the level grants, which live
    /// on the class.
    pub progression: ClassProgression,
    /// Class definition with `level_abilities` / `level_bonuses` /
    /// `level_prerequisites` cleared, and the progression formulas removed:
    /// the tables in `progression` are authoritative (R5).
    pub definition: ClassDefinition,
    #[serde(default)]
    pub origin: Option<Origin>,
}

/// One class level.
///
/// `class` is the point of the whole exercise: ownership is stated, so no
/// ordering of records can change what this level belongs to (R2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassLevelRecord {
    pub class: String,
    pub level: u32,
    #[serde(default)]
    pub grants: Vec<ClassGrant>,
    #[serde(default)]
    pub origin: Option<Origin>,
}

/// How a variant removes a feature from its class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplaceRecord {
    /// Replacement anchor, e.g. `WizardFamiliar`.  Structured, so matching does
    /// not depend on guessing at names.
    pub anchor: String,
}

/// A feature a variant grants, and the class level it arrives at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantGrantRecord {
    pub feature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_level: Option<u32>,
}

/// A class variant: Pathfinder's `archetype` and 3.5e's `ACF` are the same
/// mechanism under two names, and both land here (R4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantRecord {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_to: Option<String>,
    #[serde(default)]
    pub replaces: Vec<ReplaceRecord>,
    #[serde(default)]
    pub grants: Vec<VariantGrantRecord>,
    #[serde(default)]
    pub class_skills: Vec<String>,
    #[serde(default)]
    pub origin: Option<Origin>,
}

/// `manifest.json`: what this pack is, where it came from, and what it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    pub format: u32,
    pub pack_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_mode: Option<String>,
    pub name: String,
    pub source: SourceInfo,
    #[serde(default)]
    pub counts: BTreeMap<String, usize>,
}

/// Provenance of the whole pack: the upstream revision and the tool that wrote it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Upstream kind, e.g. `pcgen`.
    pub kind: String,
    /// Entry point that was transpiled.
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_ref: Option<String>,
    /// Transpiler identity, so output can be attributed to a version.
    pub transpiler: String,
}

/// Errors a loader can raise.  R8: a malformed or inconsistent pack is rejected
/// rather than loaded into a wrong-but-plausible state.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("unsupported format version {found}, this build understands {expected}")]
    Version { found: u32, expected: u32 },
    #[error("io error: {0}")]
    Io(String),
    #[error("malformed record: {0}")]
    Malformed(String),
    #[error("class level for undeclared class `{class}` (level {level})")]
    UnknownClass { class: String, level: u32 },
    #[error("class `{class}` declares no levels")]
    ClassWithoutLevels { class: String },
    #[error("variant `{variant}` applies to undeclared class `{class}`")]
    VariantUnknownClass { variant: String, class: String },
}
