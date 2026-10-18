//! Runtime class progression, shared by prerequisites, snapshots and validation.
//!
//! The engine re-exports the core `ClassProgression` type so there is one
//! progression representation shared by the importer and runtime.

use pcgen_core::domain::{ClassDefinition, RaceDefinition};
use pcgen_core::{AdvancementLane, CompiledRulePack, SaveKind, VariantDefinition};
use pcgen_formula::FormulaError;
use std::collections::BTreeMap;

pub use pcgen_core::domain::ClassProgression as ClassProgress;

/// Racial awards are added before the first character level's multiplier.
pub(crate) fn skill_points_for_level(
    progression: &ClassProgress,
    race: Option<&pcgen_core::domain::RaceDefinition>,
    intelligence_modifier: i32,
    first_level: bool,
) -> u32 {
    let extra = race
        .and_then(|race| race.extra_skill_points_per_level)
        .unwrap_or(0);
    let normal =
        (progression.skill_points_per_level as i32 + intelligence_modifier).max(1) as u32 + extra;
    let multiplier = if first_level {
        race.and_then(|race| race.skill_multiplier)
            .unwrap_or(progression.skill_multiplier)
    } else {
        1
    };
    normal * multiplier
}

pub(crate) fn find_race<'a>(
    pack: &'a CompiledRulePack,
    race_id: &str,
) -> Option<&'a RaceDefinition> {
    pack.races.get(race_id).or_else(|| {
        pack.races
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(race_id))
            .map(|(_, race)| race)
    })
}

/// One authoritative runtime path for both ordinary and gestalt progression.
pub(crate) fn class_value(
    pack: &CompiledRulePack,
    class_id: &str,
    level: u32,
    total_level: u32,
    save: Option<SaveKind>,
    variables: &BTreeMap<String, i32>,
) -> Result<i32, FormulaError> {
    if level == 0 {
        return Ok(0);
    }
    let class = find_class(pack, class_id).ok_or_else(|| FormulaError {
        message: format!("missing class `{class_id}`"),
        position: 0,
    })?;
    let formula = match save {
        None => &class.base_attack_formula,
        Some(SaveKind::Fortitude) => &class.fortitude_formula,
        Some(SaveKind::Reflex) => &class.reflex_formula,
        Some(SaveKind::Will) => &class.will_formula,
    };
    if let Some(formula) = formula {
        return evaluate_class_formula(formula, class_id, level, total_level, variables).map_err(
            |error| {
                crate::derived::context(
                    error,
                    &format!(
                        "progression {}",
                        save.map_or("BAB".into(), |kind| format!("{kind:?}"))
                    ),
                    formula,
                )
            },
        );
    }
    let progression = find_progression(pack, class_id).ok_or_else(|| FormulaError {
        message: format!("missing progression for `{class_id}`"),
        position: 0,
    })?;
    Ok(save.map_or_else(
        || progression.bab(level),
        |kind| progression.save(level, kind),
    ))
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

pub(crate) fn find_class<'a>(
    pack: &'a CompiledRulePack,
    class_id: &str,
) -> Option<&'a ClassDefinition> {
    let wanted = normalize_key(class_id);
    pack.classes.get(class_id).or_else(|| {
        pack.classes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_id) || normalize_key(key) == wanted)
            .map(|(_, class)| class)
    })
}

pub(crate) fn find_progression<'a>(
    pack: &'a CompiledRulePack,
    class_id: &str,
) -> Option<&'a ClassProgress> {
    let wanted = normalize_key(class_id);
    pack.progressions.get(class_id).or_else(|| {
        pack.progressions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_id) || normalize_key(key) == wanted)
            .map(|(_, progression)| progression)
    })
}

/// Returns all variants attached to a lane: archetype first, then
/// substitution-level.  Both may coexist on the same lane.
pub(crate) fn variants_for<'a>(
    pack: &'a CompiledRulePack,
    lane: &AdvancementLane,
    class_id: &str,
) -> Vec<&'a VariantDefinition> {
    let mut ids: Vec<&str> = Vec::new();
    if let Some(id) = lane.archetype.as_deref() {
        ids.push(id);
    }
    if let Some(id) = lane.substitution_level.as_deref() {
        if !ids.iter().any(|existing| existing.eq_ignore_ascii_case(id)) {
            ids.push(id);
        }
    }
    ids.into_iter()
        .filter_map(|id| {
            pack.variants.get(id).or_else(|| {
                pack.variants
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(id))
                    .map(|(_, variant)| variant)
            })
        })
        .filter(|variant| {
            variant
                .applies_to
                .as_deref()
                .is_none_or(|applies| applies.eq_ignore_ascii_case(class_id))
        })
        .collect()
}

pub(crate) struct ProgressSource<'a> {
    pub(crate) pack: &'a CompiledRulePack,
    pub(crate) race_id: &'a str,
    pub(crate) variables: &'a BTreeMap<String, i32>,
    pub(crate) total_level: u32,
    pub(crate) diagnostics: std::cell::RefCell<Vec<String>>,
}

impl pcgen_core::ClassProgressSource for ProgressSource<'_> {
    fn base_attack_bonus(&self, class_id: &str, class_level: u32) -> Option<i32> {
        crate::derived::report(
            class_value(
                self.pack,
                class_id,
                class_level,
                self.total_level,
                None,
                self.variables,
            ),
            &mut self.diagnostics.borrow_mut(),
        )
    }
    fn base_save(&self, class_id: &str, class_level: u32, kind: SaveKind) -> Option<i32> {
        crate::derived::report(
            class_value(
                self.pack,
                class_id,
                class_level,
                self.total_level,
                Some(kind),
                self.variables,
            ),
            &mut self.diagnostics.borrow_mut(),
        )
    }

    fn skill_points(
        &self,
        class_id: &str,
        intelligence_modifier: i32,
        first_level: bool,
    ) -> Option<u32> {
        find_progression(self.pack, class_id).map(|progression| {
            skill_points_for_level(
                progression,
                find_race(self.pack, self.race_id),
                intelligence_modifier,
                first_level,
            )
        })
    }

    fn class_skills(&self, class_id: &str) -> Option<Vec<String>> {
        find_progression(self.pack, class_id).map(|p| p.class_skills.clone())
    }

    fn gestalt_slot_cost(&self, class_id: &str) -> Option<pcgen_core::PrestigeSlotCost> {
        find_class(self.pack, class_id).and_then(|class| class.gestalt_slot_cost)
    }

    fn features_up_to_level(&self, class_id: &str, level: u32) -> Option<Vec<String>> {
        // The grants live on the class; the progression used to carry a copy.
        find_class(self.pack, class_id).map(|class| class.feature_rows_up_to(level))
    }

    fn features_for_class(&self, lane: &AdvancementLane, class_id: &str) -> Option<Vec<String>> {
        let base = self.features_up_to_level(class_id, lane.class_level)?;
        let variants = variants_for(self.pack, lane, class_id);
        // Level-aware: a variant's `GRANTS` may name the level it is gained at
        // (`Weapon Training|PRECLASS:1,Fighter=5`), which must not surface at
        // 1st level.
        Some(variants.iter().fold(base, |features, variant| {
            variant.apply_to_features_at_level(features, lane.class_level)
        }))
    }

    fn class_skills_for_lane(&self, lane: &AdvancementLane, class_id: &str) -> Option<Vec<String>> {
        let mut skills = self.class_skills(class_id)?;
        for variant in variants_for(self.pack, lane, class_id) {
            skills.extend(variant.class_skills.iter().cloned());
        }
        Some(skills)
    }
}

/// Evaluate a class progression formula.
///
/// The formula is already in the runtime vocabulary — lowering translated
/// PCGen's `CL` / `TL` / `classlevel(...)` into [`pcgen_core::formula_vocab`]
/// names — so there are no tokens to substitute here, only values to publish.
/// That is also why `CL` is now correct for a multiclass character: the class it
/// names was decided where the formula was written, not guessed from whichever
/// record the engine happened to be evaluating.
pub fn evaluate_class_formula(
    formula: &str,
    class_id: &str,
    class_level: u32,
    total_level: u32,
    variables: &BTreeMap<String, i32>,
) -> Result<i32, pcgen_formula::FormulaError> {
    pcgen_formula::evaluate_scoped_i32(
        formula,
        variables,
        &[
            (
                pcgen_core::formula_vocab::class_level_variable(class_id),
                class_level as i32,
            ),
            (
                pcgen_core::formula_vocab::CHARACTER_LEVEL.to_string(),
                total_level as i32,
            ),
        ],
    )
    .map_err(|error| {
        crate::derived::context(
            error,
            &format!("class `{class_id}` level {class_level}"),
            formula,
        )
    })
}

#[cfg(test)]
mod class_formula_tests {
    use super::evaluate_class_formula;
    use std::collections::BTreeMap;

    fn variables() -> BTreeMap<String, i32> {
        let mut variables = BTreeMap::new();
        variables.insert("Foo".to_string(), 7);
        variables
    }

    #[test]
    fn class_formulas_are_evaluated_in_the_runtime_vocabulary() {
        // The formulas here are what lowering produces: PCGen's `CL`, `TL` and
        // `classlevel(...)` have already become `ClassLevel_*` and
        // `CharacterLevel`.  Evaluating them is now a variable lookup.
        let variables = variables();
        let class = "Wizard";
        assert_eq!(
            evaluate_class_formula(
                &pcgen_core::formula_vocab::class_level_variable(class),
                class,
                5,
                9,
                &variables,
            ),
            Ok(5)
        );
        assert_eq!(
            evaluate_class_formula(
                &format!(
                    "{}/2+2",
                    pcgen_core::formula_vocab::class_level_variable(class)
                ),
                class,
                5,
                9,
                &variables,
            ),
            Ok(4)
        );
        assert_eq!(
            evaluate_class_formula(
                &format!("{}*2", pcgen_core::formula_vocab::CHARACTER_LEVEL),
                class,
                5,
                9,
                &variables,
            ),
            Ok(18)
        );
        // A plain variable resolves from the map.
        assert_eq!(
            evaluate_class_formula("Foo+3", class, 5, 9, &variables),
            Ok(10)
        );
        assert!(evaluate_class_formula("Missing", class, 5, 9, &variables)
            .unwrap_err()
            .message
            .contains("Missing"));
    }

    #[test]
    fn class_formula_is_stable_across_calls() {
        let variables = variables();
        let formula = pcgen_core::formula_vocab::class_level_variable("Cleric");
        let first = evaluate_class_formula(&formula, "Cleric", 5, 9, &variables);
        for _ in 0..50 {
            assert_eq!(
                evaluate_class_formula(&formula, "Cleric", 5, 9, &variables),
                first,
                "formula `{formula}` changed across calls"
            );
        }
    }
}
