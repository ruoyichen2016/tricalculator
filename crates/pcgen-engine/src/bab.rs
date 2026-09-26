//! Base attack bonus derivation shared by predicates and the final snapshot.
//!
//! `PRETOTALAB` has to be evaluated before effect predicates run, while the
//! final snapshot BAB is only known after effects have been applied.  This
//! module keeps both calculations on one path:
//!
//! * `derive_prereq_bab` -> stable, deterministic BAB used by `PRETOTALAB`;
//! * `compute_bab`        -> final snapshot BAB, including external BASEAB.
//!
//! The v1 rule is deliberately conservative: BAB modifiers that are themselves
//! gated by `PRETOTALAB` are excluded from `derive_prereq_bab` to avoid a
//! self-referential fixed point; they still contribute to the final snapshot.

use crate::draft::CharacterDraft;
use crate::engine::{effective_class_levels, evaluate_class_formula, stat_modifier};
use crate::progression::ClassProgress;
use pcgen_core::domain::ClassDefinition;
use pcgen_core::gestalt::{plan_gestalt, ClassProgressSource};
use pcgen_core::{EntityCategory, GestaltPolicy, SaveKind};
use pcgen_ir::CompiledRulePack;
use std::collections::BTreeMap;

/// Final displayed BAB: class progression/formula + external BASEAB modifiers.
pub fn compute_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
    external_modifiers: &BTreeMap<String, i32>,
) -> i32 {
    let mut bab = class_bab(pack, draft, variables);
    for (target, value) in external_modifiers {
        let upper = target.to_ascii_uppercase();
        if upper == "BAB" || upper.contains("BASEAB") {
            bab += *value;
        }
    }
    bab
}

/// Stable BAB used for `PRETOTALAB` during effect predicate evaluation.
///
/// v1 intentionally uses class-derived BAB and only **unconditional numeric**
/// external BASEAB/EPICAB modifiers from the entities active at the start of
/// effect evaluation.  Conditional BAB modifiers are deferred to the bounded
/// refinement described in `docs/P4_PRE2_PRETOTALAB_DESIGN.md`.
pub fn derive_prereq_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    active_indexes: &[usize],
) -> i32 {
    let (baseab, epicab) = static_bab_modifiers(pack, active_indexes);
    let mut bab = compute_bab(pack, draft, &BTreeMap::new(), &baseab);
    bab += epicab;
    bab
}

/// Stable base save totals used by `PRECHECKBASE` during predicate
/// evaluation: class progression base saves only (no ability modifiers, no
/// conditional bonuses), matching PCGen's "base save" semantics.
pub fn derive_base_saves(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> BTreeMap<SaveKind, i32> {
    let mut totals = BTreeMap::new();
    for entry in effective_class_levels(draft) {
        let Some(progression) = find_progression(pack, &entry.class_id) else {
            continue;
        };
        for kind in [SaveKind::Fortitude, SaveKind::Reflex, SaveKind::Will] {
            *totals.entry(kind).or_insert(0) += progression.save(entry.level, kind);
        }
    }
    totals
}

fn class_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
) -> i32 {
    if draft.is_gestalt() {
        let int_mod = draft.stats.get("INT").copied().unwrap_or(10);
        let plan = plan_gestalt(
            &draft.advancement,
            &PackProgressSource { pack },
            stat_modifier(int_mod),
            &GestaltPolicy::default(),
        );
        return plan
            .frames
            .iter()
            .map(|frame| frame.base_attack_bonus_increment)
            .sum();
    }

    let total_level = draft.total_level();
    let mut bab = 0;
    for entry in effective_class_levels(draft) {
        let Some(class) = find_class(pack, &entry.class_id) else {
            continue;
        };
        let progression = find_progression(pack, &entry.class_id);
        bab += class
            .base_attack_formula
            .as_deref()
            .map(|formula| evaluate_class_formula(formula, entry.level, total_level, variables))
            .or_else(|| progression.map(|progress| progress.bab(entry.level)))
            .unwrap_or(0);
    }
    bab
}

/// Unconditional numeric BASEAB/EPICAB modifiers on directly active entities.
///
/// Class / ClassLevel entities are skipped because their BASEAB is already
/// represented by the typed progression (`ClassDefinition.base_attack_formula`
/// / `ClassProgression`).  Formula modifiers are skipped in v1; they need the
/// bounded refinement phase.
fn static_bab_modifiers(
    pack: &CompiledRulePack,
    active_indexes: &[usize],
) -> (BTreeMap<String, i32>, i32) {
    let mut baseab = BTreeMap::new();
    let mut epicab = 0;
    for &index in active_indexes {
        let Some(entity) = pack.entities.get(index) else {
            continue;
        };
        if matches!(
            entity.category,
            EntityCategory::Class | EntityCategory::ClassLevel
        ) {
            continue;
        }
        for effect in &entity.effects {
            // Conditional modifiers are not part of the v1 pre-BAB pass.
            if effect.condition.is_some() {
                continue;
            }
            let target = match &effect.action {
                pcgen_core::EffectAction::Modify { modifier } => Some(modifier.target.as_str()),
                pcgen_core::EffectAction::ModifyFormula { target, .. } => Some(target.as_str()),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            let upper = target.to_ascii_uppercase();
            let value = match &effect.action {
                pcgen_core::EffectAction::Modify { modifier } => modifier.value.floor() as i32,
                // Formula modifiers are intentionally deferred in v1.
                pcgen_core::EffectAction::ModifyFormula { .. } => continue,
                _ => unreachable!(),
            };
            if upper == "BAB" || upper.contains("BASEAB") {
                *baseab.entry(target.to_string()).or_insert(0) += value;
            } else if upper.contains("EPICAB") {
                epicab += value;
            }
        }
    }
    (baseab, epicab)
}

/// Emit diagnostics for effects that both modify BAB and are gated by
/// `PRETOTALAB`.  v1 excludes them from `BAB_prereq`; the diagnostic makes the
/// conservative choice visible instead of silently changing semantics.
pub fn bab_cycle_diagnostics(pack: &CompiledRulePack, active_indexes: &[usize]) -> Vec<String> {
    let mut diagnostics = Vec::new();
    for &index in active_indexes {
        let Some(entity) = pack.entities.get(index) else {
            continue;
        };
        for effect in &entity.effects {
            let Some(condition) = &effect.condition else {
                continue;
            };
            if !contains_total_bab(condition) {
                continue;
            }
            let target = match &effect.action {
                pcgen_core::EffectAction::Modify { modifier } => Some(modifier.target.as_str()),
                pcgen_core::EffectAction::ModifyFormula { target, .. } => Some(target.as_str()),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            let upper = target.to_ascii_uppercase();
            if upper == "BAB" || upper.contains("BASEAB") || upper.contains("EPICAB") {
                diagnostics.push(format!(
                    "PRETOTALAB_BAB_CYCLE: {} ({})",
                    entity.name, target
                ));
            }
        }
    }
    diagnostics
}

fn contains_total_bab(predicate: &pcgen_core::Predicate) -> bool {
    use pcgen_core::Predicate;
    match predicate {
        Predicate::TotalBab { .. } => true,
        Predicate::All(items) | Predicate::Any(items) | Predicate::AtLeast { items, .. } => {
            items.iter().any(contains_total_bab)
        }
        Predicate::Not(item) => contains_total_bab(item),
        _ => false,
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn find_class<'a>(pack: &'a CompiledRulePack, class_id: &str) -> Option<&'a ClassDefinition> {
    let wanted = normalize_key(class_id);
    pack.classes.get(class_id).or_else(|| {
        pack.classes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_id) || normalize_key(key) == wanted)
            .map(|(_, class)| class)
    })
}

fn find_progression<'a>(pack: &'a CompiledRulePack, class_id: &str) -> Option<&'a ClassProgress> {
    let wanted = normalize_key(class_id);
    pack.progressions.get(class_id).or_else(|| {
        pack.progressions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_id) || normalize_key(key) == wanted)
            .map(|(_, progression)| progression)
    })
}

/// Minimal `ClassProgressSource` for gestalt BAB derivation without an Engine.
struct PackProgressSource<'a> {
    pack: &'a CompiledRulePack,
}

impl ClassProgressSource for PackProgressSource<'_> {
    fn base_attack_bonus(&self, class_id: &str, class_level: u32) -> Option<i32> {
        find_progression(self.pack, class_id).map(|progress| progress.bab(class_level))
    }

    fn base_save(&self, class_id: &str, class_level: u32, kind: SaveKind) -> Option<i32> {
        find_progression(self.pack, class_id).map(|progress| progress.save(class_level, kind))
    }

    fn skill_points(
        &self,
        class_id: &str,
        intelligence_modifier: i32,
        first_level: bool,
    ) -> Option<u32> {
        find_progression(self.pack, class_id).map(|progress| {
            let base = if first_level {
                progress.first_level_skill_points
            } else {
                progress.skill_points_per_level
            };
            (base as i32 + intelligence_modifier).max(1) as u32
        })
    }

    fn class_skills(&self, class_id: &str) -> Option<Vec<String>> {
        find_progression(self.pack, class_id).map(|progress| progress.class_skills.clone())
    }

    fn features_up_to_level(&self, class_id: &str, level: u32) -> Option<Vec<String>> {
        find_progression(self.pack, class_id).map(|progress| progress.features_up_to(level))
    }

    fn gestalt_slot_cost(&self, class_id: &str) -> Option<pcgen_core::PrestigeSlotCost> {
        find_class(self.pack, class_id).and_then(|class| class.gestalt_slot_cost)
    }
}
