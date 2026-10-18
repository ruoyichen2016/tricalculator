//! Runtime rule IR shared by the importer and the engine.
//!
//! This crate owns the *neutral* intermediate representation: lossless raw
//! records, patch operations, typed definitions, and the compiled rule pack
//! that the engine consumes.  It deliberately contains **no** PCGen file
//! format handling (`.pcc` graph resolution, `.pcg` drafts, `.lst`
//! tokenization) — that stays in `pcgen-import`.
//!
//! Dependency direction:
//!
//! ```text
//! pcgen-import  ->  pcgen-ir  <-  pcgen-engine
//! ```
//!
//! The engine depends on `pcgen-ir` (and `pcgen-core`), never on
//! `pcgen-import`.

pub mod draft;
pub mod patched;
pub mod patches;
pub mod rulepack;
pub mod source;
pub mod tags;
pub mod typed;

pub use draft::{PcgClassLevel, PcgDraft, PcgSpell};
pub use patched::apply_record_patches;
pub use patches::PatchOperation;
pub use rulepack::{
    categories, lower_records, patch_count, CompiledPackHeader, CompiledRulePack, ImportReport,
};
pub use source::{RawRecord, RawTag, SourceRef};
pub use tags::{is_known_tag, is_name_prefix_tag};
pub use typed::build_typed_definitions;

pub mod formula;
