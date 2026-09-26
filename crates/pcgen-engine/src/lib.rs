//! Deterministic runtime engine and character snapshot model.

pub mod bab;
pub mod draft;
pub mod effects;
pub mod engine;
pub mod progression;
pub mod snapshot;

pub use bab::{compute_bab, derive_prereq_bab};
pub use draft::{CharacterDraft, DraftClassLevel, DraftCompanion, DraftSpell};
pub use effects::{apply_effects, EffectState};
pub use engine::{lane_summary, validate, Engine};
pub use progression::ClassProgress;
pub use snapshot::{AbilityPoolUsage, CharacterSnapshot, ClassInteraction, CompanionSnapshot};
