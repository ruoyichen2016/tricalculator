//! Deterministic runtime engine and character snapshot model.

pub mod bab;
pub mod draft;
pub mod effects;
pub mod engine;
pub mod progression;
pub mod snapshot;

pub use bab::{compute_bab, derive_prereq_bab};
pub use draft::{
    default_character_type, CharacterDraft, ClassLevelSummary, DraftCompanion, DraftSpell,
};
pub use effects::{apply_effects, EffectState};
pub use engine::{lane_summary, Engine};
pub use progression::{evaluate_class_formula, ClassProgress};
pub use snapshot::{
    alignment_morality, AbilityPoolUsage, CharacterSnapshot, ClassInteraction, CompanionSnapshot,
    SpellEffectApplication, TurningMode, TurningSnapshot,
};

mod pools;

mod bonuses;
mod variables;

mod derived;

pub mod magic;

pub mod metamagic;

pub mod class_actions;

pub mod class_features;

pub mod class_play;

pub mod class_resources;

pub mod targeting;

pub mod item_casting;

mod encounter_health;
pub mod feature_resolution;
pub mod spell_defenses;

pub mod necromancy;

mod tactical_features;

mod combat;

mod spell_impacts;

mod touch_charges;

pub mod martial;

pub mod martial_case;

pub mod incarnum;
pub mod incarnum_case;
pub mod joker;
pub mod spell_features;

pub mod build;

pub mod execution;

pub mod cases;
