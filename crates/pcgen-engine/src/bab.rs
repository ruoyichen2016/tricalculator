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
use crate::engine::effective_class_levels;
use crate::progression::{class_value, ProgressSource};
use pcgen_core::gestalt::plan_gestalt;
use pcgen_core::CompiledRulePack;
use pcgen_core::{EntityCategory, GestaltPolicy, SaveKind};
use pcgen_formula::FormulaError;
use std::collections::BTreeMap;

/// Final displayed BAB: class progression/formula + external BASEAB modifiers.
pub fn compute_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
    external_modifiers: &BTreeMap<String, i32>,
) -> Result<i32, FormulaError> {
    let mut bab = class_bab(pack, draft, variables)?;
    for (target, value) in external_modifiers {
        let upper = target.to_ascii_uppercase();
        if upper == "BAB" || upper.contains("BASEAB") {
            bab += *value;
        }
    }
    Ok(bab)
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
    variables: &BTreeMap<String, i32>,
) -> Result<i32, FormulaError> {
    let (baseab, epicab) = static_bab_modifiers(pack, active_indexes);
    let mut bab = compute_bab(pack, draft, variables, &baseab)?;
    bab += epicab;
    Ok(bab)
}

/// The largest base attack bonus any single class contributes, for `PREUATT`.
///
/// The name is misleading in both directions.  The reference describes `PREUATT`
/// as an "unarmed attack bonus", and the tester's class is called
/// `PreUnarmedAttackTester`, but what it computes is the **maximum per-class**
/// base attack bonus rather than the character's total:
///
/// ```java
/// int att = 0;
/// for (PCClass aClass : display.getClassSet())
///     att = Math.max(att, aClass.baseAttackBonus(character));
/// ```
///
/// so a Fighter 4 / Wizard 4 has 4 here, not the 6 their levels add up to.
/// `effective_class_levels` also covers advancement lanes, so a gestalt draft
/// takes the maximum over its lanes — an analogue PCGen has no counterpart for.
pub fn max_class_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
) -> Result<i32, FormulaError> {
    let mut best = 0;
    for entry in effective_class_levels(pack, draft) {
        best = best.max(class_value(
            pack,
            &entry.class_id,
            entry.level,
            draft.total_level(),
            None,
            variables,
        )?);
    }
    Ok(best)
}

/// Stable base save totals used by `PRECHECKBASE` during predicate
/// evaluation: class progression base saves only (no ability modifiers, no
/// conditional bonuses), matching PCGen's "base save" semantics.
pub fn derive_base_saves(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
) -> Result<BTreeMap<SaveKind, i32>, FormulaError> {
    let mut totals = BTreeMap::new();
    if draft.is_gestalt() {
        let plan = gestalt_progression(pack, draft, variables)?;
        for frame in &plan.frames {
            *totals.entry(SaveKind::Fortitude).or_insert(0) += frame.fortitude_increment;
            *totals.entry(SaveKind::Reflex).or_insert(0) += frame.reflex_increment;
            *totals.entry(SaveKind::Will).or_insert(0) += frame.will_increment;
        }
        return Ok(totals);
    }
    for entry in effective_class_levels(pack, draft) {
        for kind in [SaveKind::Fortitude, SaveKind::Reflex, SaveKind::Will] {
            *totals.entry(kind).or_insert(0) += class_value(
                pack,
                &entry.class_id,
                entry.level,
                draft.total_level(),
                Some(kind),
                variables,
            )?;
        }
    }
    Ok(totals)
}

fn class_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
) -> Result<i32, FormulaError> {
    if draft.is_gestalt() {
        let plan = gestalt_progression(pack, draft, variables)?;
        return Ok(plan
            .frames
            .iter()
            .map(|frame| frame.base_attack_bonus_increment)
            .sum());
    }

    effective_class_levels(pack, draft)
        .iter()
        .try_fold(0, |total, entry| {
            Ok(total
                + class_value(
                    pack,
                    &entry.class_id,
                    entry.level,
                    draft.total_level(),
                    None,
                    variables,
                )?)
        })
}

fn gestalt_progression(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
) -> Result<pcgen_core::gestalt::GestaltPlan, FormulaError> {
    let source = ProgressSource {
        pack,
        race_id: &draft.race_id,
        variables,
        total_level: draft.total_level(),
        diagnostics: Default::default(),
    };
    // BAB and base saves do not depend on the character's Intelligence.
    let plan = plan_gestalt(&draft.advancement, &source, 0, &GestaltPolicy::default());
    if let Some(message) = source.diagnostics.into_inner().into_iter().next() {
        return Err(FormulaError {
            message,
            position: 0,
        });
    }
    Ok(plan)
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
