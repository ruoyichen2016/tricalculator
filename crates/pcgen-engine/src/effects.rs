//! Rule IR execution.
//!
//! Collects active entities, expands `Grant` / `AUTO` closures, evaluates
//! `DefineVariable` and `ModifyFormula`, applies modifiers to a runtime
//! variable table, and records `CHOOSE` resolution.

use crate::draft::CharacterDraft;
use pcgen_core::{CastingType, SpellPowerType};
use pcgen_core::{ChoiceKind, EntityCategory};
use pcgen_core::{CompiledEntity, EffectAction, EntityId, ModifierOperation, Predicate};
use pcgen_formula::{Value as FormulaValue, VariableResolver};
use pcgen_ir::CompiledRulePack;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct EffectState {
    pub variables: BTreeMap<String, i32>,
    pub external_modifiers: BTreeMap<String, i32>,
    pub active_abilities: BTreeSet<String>,
    pub choices: BTreeMap<String, String>,
    pub unresolved_choices: Vec<String>,
    pub diagnostics: Vec<String>,
    pub applied_effects: usize,
    pub structural_effects_skipped: usize,
    /// Base creature size (from the draft's race), used by size predicates.
    pub size: String,
    /// Number of legs, when known; defaults to biped (2).
    pub legs: u32,
    /// Race type tags (lower-cased), for `PRERACETYPE` predicates.
    pub race_types: Vec<String>,
    /// Total character class levels, excluding Monster (Racial) HD.
    pub character_class_levels: u32,
    /// Stable total BAB used by `PRETOTALAB` during predicate evaluation.
    pub total_bab: i32,
    /// Spell power types the character can cast (stage-A derived).
    pub spell_powers: BTreeSet<SpellPowerType>,
    /// Highest spell level with a slot per power type.
    pub max_spell_level: BTreeMap<SpellPowerType, u32>,
    /// Total spell slots keyed by `(power, spell level)`.
    pub slots_by_power_level: BTreeMap<(SpellPowerType, u32), u32>,
    /// Character has a class that casts from a spellbook.
    pub has_spellbook: bool,
    /// Casting mode per class id.
    pub class_casting: BTreeMap<String, CastingType>,
    /// Inherent (race) plus applied (draft) templates, lower-cased.
    pub templates: BTreeSet<String>,
    /// Number of hands.
    pub hands: u32,
    /// Movement modes in feet (lower-cased mode -> rate).
    pub movement: BTreeMap<String, u32>,
}

impl VariableResolver for EffectState {
    fn resolve(&self, name: &str) -> Option<FormulaValue> {
        self.variables
            .get(name)
            .copied()
            .map(|value| FormulaValue::Int(value as i64))
    }
}

/// Apply all effects reachable from the draft.
pub fn apply_effects(pack: &CompiledRulePack, draft: &CharacterDraft) -> EffectState {
    // Resolve base size from the draft's race so PREBASESIZE* predicates work.
    let base_size = pack
        .races
        .get(&draft.race_id)
        .map(|race| race.size.clone())
        .filter(|size| !size.trim().is_empty())
        .unwrap_or_else(|| "Medium".to_string());
    let race_types = pack
        .races
        .get(&draft.race_id)
        .map(|race| {
            race.type_tags
                .iter()
                .map(|tag| tag.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let class_levels = character_class_levels(pack, draft);
    let initial = initial_entities(pack, draft);
    let prereq_bab = crate::bab::derive_prereq_bab(pack, draft, &initial);
    let (spell_powers, max_spell_level, slots_by_power_level, has_spellbook, class_casting) =
        derive_spellcasting_prereq(pack, draft);
    let race = pack.races.get(&draft.race_id);
    let mut templates: BTreeSet<String> = race
        .map(|race| race.templates.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    templates.extend(draft.templates.iter().map(|name| name.to_ascii_lowercase()));
    let hands = race.map(|race| race.hands).unwrap_or(2);
    let movement = race.map(|race| race.movement.clone()).unwrap_or_default();
    let mut state = EffectState {
        size: base_size,
        legs: 2,
        race_types,
        character_class_levels: class_levels,
        total_bab: prereq_bab,
        spell_powers,
        max_spell_level,
        slots_by_power_level,
        has_spellbook,
        class_casting,
        templates,
        hands,
        movement,
        ..EffectState::default()
    };
    state
        .diagnostics
        .extend(crate::bab::bab_cycle_diagnostics(pack, &initial));
    let mut active: BTreeSet<usize> = BTreeSet::new();
    let mut queued: BTreeSet<usize> = BTreeSet::new();
    let mut processed_variant_effects: BTreeSet<(String, usize)> = BTreeSet::new();

    for index in initial {
        queued.insert(index);
    }

    let mut variable_defs: BTreeMap<String, String> = BTreeMap::new();
    let mut pending_formulas: Vec<(String, String)> = Vec::new();

    // Fixed-point closure over granted entities and variant effects.
    for _ in 0..16 {
        let current: Vec<usize> = queued.iter().copied().collect();
        queued.clear();
        let mut changed = false;
        for index in current {
            if !active.insert(index) {
                continue;
            }
            let Some(entity) = pack.entities.get(index) else {
                continue;
            };
            for effect in &entity.effects {
                if !predicate_holds(effect.condition.as_ref(), draft, &state) {
                    continue;
                }
                state.applied_effects += 1;
                apply_action(
                    effect,
                    entity.category.clone(),
                    Some(&entity.id),
                    pack,
                    draft,
                    &mut state,
                    &mut variable_defs,
                    &mut pending_formulas,
                    &mut active,
                    &mut queued,
                    &mut changed,
                );
            }
        }

        for frame in &draft.advancement {
            for lane in &frame.lanes {
                let Some(variant_id) = lane
                    .archetype
                    .as_deref()
                    .or(lane.substitution_level.as_deref())
                else {
                    continue;
                };
                let Some(variant) = pack.variants.get(variant_id).or_else(|| {
                    pack.variants
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(variant_id))
                        .map(|(_, variant)| variant)
                }) else {
                    continue;
                };
                for (index, effect) in variant.effects.iter().enumerate() {
                    if !processed_variant_effects.insert((variant.id.clone(), index)) {
                        continue;
                    }
                    if !predicate_holds(effect.condition.as_ref(), draft, &state) {
                        continue;
                    }
                    state.applied_effects += 1;
                    apply_action(
                        effect,
                        EntityCategory::Class,
                        None,
                        pack,
                        draft,
                        &mut state,
                        &mut variable_defs,
                        &mut pending_formulas,
                        &mut active,
                        &mut queued,
                        &mut changed,
                    );
                }
            }
        }

        if !changed {
            break;
        }
    }

    // Resolve DEFINE/VAR first, then evaluate formulas and re-resolve any
    // variables that depend on formula results.
    for _ in 0..16 {
        resolve_variables(&mut state, &variable_defs, draft);
        if pending_formulas.is_empty() {
            break;
        }
        let mut progress = false;
        let mut remaining = Vec::new();
        for (target, expression) in pending_formulas.drain(..) {
            let normalized = normalize_formula(&expression, draft, &state);
            match pcgen_formula::evaluate(&normalized, &state) {
                Ok(value) => {
                    add_target(&mut state, &target, formula_to_i32(&value));
                    progress = true;
                }
                Err(_) => remaining.push((target, expression)),
            }
        }
        pending_formulas = remaining;
        if !progress {
            break;
        }
    }
    for (target, expression) in pending_formulas {
        state.diagnostics.push(format!(
            "unresolved formula `{expression}` for target `{target}`"
        ));
    }
    state
}

#[allow(clippy::too_many_arguments)]
fn apply_action(
    effect: &pcgen_core::Effect,
    source_category: EntityCategory,
    source_entity: Option<&pcgen_core::EntityId>,
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    state: &mut EffectState,
    variable_defs: &mut BTreeMap<String, String>,
    pending_formulas: &mut Vec<(String, String)>,
    active: &mut BTreeSet<usize>,
    queued: &mut BTreeSet<usize>,
    changed: &mut bool,
) {
    match &effect.action {
        EffectAction::DefineVariable { name, expression } => {
            variable_defs.insert(name.clone(), expression.clone());
        }
        EffectAction::DeclareVariable { name, initial } => {
            state
                .variables
                .entry(name.clone())
                .or_insert(initial.floor() as i32);
        }
        EffectAction::SetVariable { name, value } => {
            state.variables.insert(name.clone(), value.floor() as i32);
        }
        EffectAction::Modify { modifier } => {
            if modifier.operation == ModifierOperation::Add {
                if structural_effect_skipped(
                    source_category.clone(),
                    source_entity,
                    pack,
                    &modifier.target,
                ) {
                    state.structural_effects_skipped += 1;
                } else {
                    add_target(state, &modifier.target, modifier.value.floor() as i32);
                }
            }
        }
        EffectAction::ModifyFormula {
            target, expression, ..
        } => {
            if structural_effect_skipped(source_category.clone(), source_entity, pack, target) {
                state.structural_effects_skipped += 1;
            } else {
                pending_formulas.push((target.clone(), expression.clone()));
            }
        }
        EffectAction::Grant { entity } => {
            let resolved = EntityId {
                key: resolve_placeholders(&entity.key, state),
                ..entity.clone()
            };
            if let Some(target) = find_entity(pack, &resolved) {
                if !grant_allowed(&pack.entities[target], draft, state) {
                    state.diagnostics.push(format!(
                        "grant `{}` failed embedded prerequisites",
                        resolved.key
                    ));
                    return;
                }
                if !active.contains(&target) && queued.insert(target) {
                    *changed = true;
                }
                state.active_abilities.insert(resolved.key.clone());
            } else {
                state.active_abilities.insert(resolved.key.clone());
            }
        }
        EffectAction::GrantTag { tag } => {
            state.active_abilities.insert(tag.clone());
        }
        EffectAction::Choose { spec } => {
            apply_choose(spec, draft, state);
        }
    }
}

fn apply_choose(spec: &str, draft: &CharacterDraft, state: &mut EffectState) {
    if let Some(choice) = draft.choices.get(spec) {
        state.choices.insert(spec.to_string(), choice.clone());
    } else if spec.contains('|') {
        let choices: Vec<&str> = spec.split('|').map(str::trim).collect();
        let candidate = choices.iter().find(|choice| {
            !choice.is_empty() && !choice.contains(':') && !ChoiceKind::is_keyword(choice)
        });
        if let Some(first) = candidate {
            state.choices.insert(spec.to_string(), (*first).to_string());
        } else {
            state.unresolved_choices.push(spec.to_string());
        }
    } else {
        state.unresolved_choices.push(spec.to_string());
    }
}

fn initial_entities(pack: &CompiledRulePack, draft: &CharacterDraft) -> Vec<usize> {
    let mut indexes = BTreeSet::new();
    for (index, entity) in pack.entities.iter().enumerate() {
        let key = normalized(&entity.id.key);
        let active = match &entity.category {
            EntityCategory::Race => key == normalized(&draft.race_id),
            EntityCategory::Class => {
                draft.level_in_class(&entity.id.key) > 0 || draft.level_in_class(&entity.name) > 0
            }
            EntityCategory::ClassLevel => {
                entity
                    .id
                    .key
                    .split_once('#')
                    .is_some_and(|(class_id, level)| {
                        draft.level_in_class(class_id) >= level.parse::<u32>().unwrap_or(0)
                    })
            }
            EntityCategory::Equipment => draft.equipment.iter().any(|item| {
                normalized(item) == key || normalized(&entity.name) == normalized(item)
            }),
            category if category.is_ability_like() => draft.abilities.iter().any(|(_, ability)| {
                ability.eq_ignore_ascii_case(&entity.id.key)
                    || ability.eq_ignore_ascii_case(&entity.name)
            }),
            _ => false,
        };
        if active {
            indexes.insert(index);
        }
    }
    indexes.into_iter().collect()
}

fn find_entity(pack: &CompiledRulePack, entity: &pcgen_core::EntityId) -> Option<usize> {
    pack.entities
        .iter()
        .position(|candidate| candidate.id == *entity)
        .or_else(|| {
            // A grant's `entity_type` is the PCGen ability *pool*
            // (`ABILITY:Special Ability|...`), while the entity's own
            // `ability_category` records that pool.  Prefer the entity whose
            // pool matches, so two abilities with the same key in different
            // pools resolve to the right one.
            let target_key = normalized(&entity.key);
            let mut fallback: Option<usize> = None;
            for (index, candidate) in pack.entities.iter().enumerate() {
                if normalized(&candidate.id.key) != target_key {
                    continue;
                }
                if candidate
                    .ability_category
                    .as_deref()
                    .is_some_and(|pool| pool.eq_ignore_ascii_case(&entity.entity_type))
                {
                    return Some(index);
                }
                fallback.get_or_insert(index);
            }
            fallback
        })
}

fn add_target(state: &mut EffectState, target: &str, value: i32) {
    if let Some(name) = target.strip_prefix("VAR.") {
        *state.variables.entry(name.to_string()).or_insert(0) += value;
    } else {
        *state
            .external_modifiers
            .entry(target.to_string())
            .or_insert(0) += value;
    }
}

/// Count character class levels, excluding Monster (Racial) classes.
///
/// When the draft only has advancement frames (no explicit class_levels), fall
/// back to `total_level()` rather than guessing which lanes are racial HD.
fn character_class_levels(pack: &CompiledRulePack, draft: &CharacterDraft) -> u32 {
    if draft.class_levels.is_empty() {
        return draft.total_level();
    }
    draft
        .class_levels
        .iter()
        .filter_map(|entry| {
            let class = pack.classes.get(&entry.class_id).or_else(|| {
                pack.classes
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(&entry.class_id))
                    .map(|(_, class)| class)
            });
            match class {
                Some(class) if class.is_monster => None,
                _ => Some(entry.level),
            }
        })
        .sum()
}

/// Stage-A spellcasting state: derived only from class levels and static
/// class definitions, never from effects, so `PRESPELL*` predicates can be
/// evaluated without a fixed-point loop.
#[allow(clippy::type_complexity)]
fn derive_spellcasting_prereq(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> (
    BTreeSet<SpellPowerType>,
    BTreeMap<SpellPowerType, u32>,
    BTreeMap<(SpellPowerType, u32), u32>,
    bool,
    BTreeMap<String, CastingType>,
) {
    let mut powers = BTreeSet::new();
    let mut max_spell_level: BTreeMap<SpellPowerType, u32> = BTreeMap::new();
    let mut slots: BTreeMap<(SpellPowerType, u32), u32> = BTreeMap::new();
    let mut has_spellbook = false;
    let mut class_casting = BTreeMap::new();
    for entry in crate::engine::effective_class_levels(draft) {
        let Some(class) = pack.classes.get(&entry.class_id).or_else(|| {
            pack.classes
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&entry.class_id))
                .map(|(_, class)| class)
        }) else {
            continue;
        };
        class_casting.insert(class.id.clone(), class.casting_type);
        if class.requires_spellbook {
            has_spellbook = true;
        }
        if class.spellcasting_ability.is_none() {
            continue;
        }
        if class.spell_power != SpellPowerType::Unknown {
            powers.insert(class.spell_power);
        }
        if let Some(table) = class.spell_slots.get(&entry.level) {
            for (level, count) in table.iter().enumerate() {
                if *count == 0 {
                    continue;
                }
                let level = level as u32;
                *slots.entry((class.spell_power, level)).or_insert(0) += *count;
                max_spell_level
                    .entry(class.spell_power)
                    .and_modify(|current| *current = (*current).max(level))
                    .or_insert(level);
            }
        }
    }
    (powers, max_spell_level, slots, has_spellbook, class_casting)
}

fn predicate_holds(
    predicate: Option<&Predicate>,
    draft: &CharacterDraft,
    state: &EffectState,
) -> bool {
    let Some(predicate) = predicate else {
        return true;
    };
    match predicate {
        Predicate::Always => true,
        Predicate::All(items) => items
            .iter()
            .all(|item| predicate_holds(Some(item), draft, state)),
        Predicate::Any(items) => items
            .iter()
            .any(|item| predicate_holds(Some(item), draft, state)),
        Predicate::Not(item) => !predicate_holds(Some(item), draft, state),
        Predicate::Stat { stat, min, max } => {
            let value = draft.stats.get(stat).copied().unwrap_or(10);
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::ClassLevel { class_id, min, max } => {
            let level = draft.level_in_class(class_id);
            level >= *min && max.is_none_or(|max| level <= max)
        }
        Predicate::HasEntity { entity } => state
            .active_abilities
            .iter()
            .any(|ability| ability.eq_ignore_ascii_case(&entity.key)),
        Predicate::Variable { name, min, max } => {
            let value = state.variables.get(name).copied().unwrap_or(0);
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::SkillRank { skill, min } => {
            let ranks = draft.skills.get(skill).copied().unwrap_or(0.0);
            ranks >= f64::from(*min)
        }
        Predicate::Alignment { allowed } => allowed
            .iter()
            .any(|alignment| alignment.eq_ignore_ascii_case(&draft.alignment)),
        Predicate::CharacterLevel { min } => draft.total_level() >= *min,
        Predicate::CharacterClassLevel { min, max } => {
            let value = state.character_class_levels;
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::TotalBab { min } => state.total_bab >= *min as i32,
        Predicate::HasTag { tag } => state
            .active_abilities
            .iter()
            .any(|ability| ability.eq_ignore_ascii_case(tag)),
        Predicate::SizeAtLeast { size } => pcgen_core::size_at_least(&state.size, size),
        Predicate::SizeAtMost { size } => pcgen_core::size_at_most(&state.size, size),
        Predicate::LegsAtLeast { count } => state.legs >= *count,
        Predicate::RuleEnabled { rule } => rule_enabled(rule, draft),
        Predicate::AtLeast { n, items } => {
            items
                .iter()
                .filter(|item| predicate_holds(Some(item), draft, state))
                .count() as u32
                >= *n
        }
        Predicate::RaceType { type_tag } => state
            .race_types
            .iter()
            .any(|candidate| candidate == &type_tag.to_ascii_lowercase()),
        Predicate::AgeCategory { category } => draft
            .choices
            .get("AGE.CATEGORY")
            .is_some_and(|value| value.eq_ignore_ascii_case(category)),
        Predicate::SpellType {
            power,
            min_level,
            count,
        } => {
            let available: u32 = state
                .slots_by_power_level
                .iter()
                .filter(|((candidate, level), _)| candidate == power && *level >= *min_level)
                .map(|(_, slots)| *slots)
                .sum();
            available >= (*count).max(1)
        }
        Predicate::HasSpellbook => state.has_spellbook,
        Predicate::HasSpellPower { power } => state.spell_powers.contains(power),
        Predicate::TemplateCount { names, min_count } => {
            names
                .iter()
                .filter(|name| state.templates.contains(*name))
                .count() as u32
                >= *min_count
        }
        Predicate::HandsAtLeast { count } => state.hands >= *count,
        Predicate::HandsAtMost { count } => state.hands <= *count,
        Predicate::MovementAtLeast { mode, feet } => {
            state.movement.get(mode).is_some_and(|rate| *rate >= *feet)
        }
        Predicate::CastingTypeIs { prepared } => state.class_casting.values().any(|mode| {
            if *prepared {
                *mode == CastingType::Prepared
            } else {
                matches!(mode, CastingType::Spontaneous | CastingType::Innate)
            }
        }),
    }
}

/// PCGen game-mode rules (`PRERULE`) default to *on*; characters may disable
/// them explicitly via the draft's choice map (`RULE:<name> = off`).
fn rule_enabled(rule: &str, draft: &CharacterDraft) -> bool {
    !matches!(
        draft
            .choices
            .get(&format!("RULE:{rule}"))
            .map(String::as_str),
        Some("off") | Some("false") | Some("0") | Some("no")
    )
}

fn resolve_variables(
    state: &mut EffectState,
    definitions: &BTreeMap<String, String>,
    draft: &CharacterDraft,
) {
    if definitions.is_empty() {
        return;
    }
    let names: Vec<String> = definitions.keys().cloned().collect();
    let index: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    let mut adjacency = vec![Vec::new(); names.len()];
    for (position, name) in names.iter().enumerate() {
        let expression = definitions.get(name).cloned().unwrap_or_default();
        for dependency in variable_dependencies(&expression) {
            if let Some(&dependency_index) = index.get(dependency.as_str()) {
                // dependency -> dependent
                adjacency[dependency_index].push(position);
            }
        }
    }
    let Some(order) = pcgen_core::topological_sort(&adjacency) else {
        let cyclic: Vec<&str> = names
            .iter()
            .filter(|name| {
                let expression = definitions.get(*name).cloned().unwrap_or_default();
                !variable_dependencies(&expression).is_empty()
            })
            .map(String::as_str)
            .collect();
        state
            .diagnostics
            .push(format!("variable dependency cycle involving {cyclic:?}"));
        return;
    };

    for position in order {
        let name = &names[position];
        let expression = definitions.get(name).cloned().unwrap_or_default();
        let normalized = normalize_formula(&expression, draft, state);
        match pcgen_formula::evaluate(&normalized, state) {
            Ok(value) => {
                state.variables.insert(name.clone(), formula_to_i32(&value));
            }
            Err(error) => state
                .diagnostics
                .push(format!("variable `{name}` failed: {}", error.message)),
        }
    }
}

fn variable_dependencies(expression: &str) -> BTreeSet<String> {
    let normalized = dependency_expression(expression);
    pcgen_formula::Formula::parse(&normalized)
        .map(|formula| formula.variables().into_iter().collect())
        .unwrap_or_default()
}

fn dependency_expression(expression: &str) -> String {
    let mut normalized = expression.to_string();
    let classlevel = Regex::new(r#"classlevel\s*\(\s*["']?[^"')]+["']?\s*\)"#).unwrap();
    normalized = classlevel.replace_all(&normalized, "0").to_string();
    let var_call = Regex::new(r#"var\s*\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap();
    normalized = var_call
        .replace_all(&normalized, |captures: &regex::Captures| {
            let name = captures[1].trim();
            if name.contains('=') {
                "0".to_string()
            } else {
                name.to_string()
            }
        })
        .to_string();
    normalized.replace("%CHOICE%", "0").replace("%LIST%", "0")
}

fn normalize_formula(expression: &str, draft: &CharacterDraft, state: &EffectState) -> String {
    let mut normalized = expression.to_string();
    let classlevel = Regex::new(r#"classlevel\s*\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap();
    normalized = classlevel
        .replace_all(&normalized, |captures: &regex::Captures| {
            let class_id = captures[1].trim();
            let level = if class_id.eq_ignore_ascii_case("APPLIEDAS=NONEPIC") {
                draft.total_level()
            } else {
                draft.level_in_class(class_id)
            };
            level.to_string()
        })
        .to_string();
    let var_call = Regex::new(r#"var\s*\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap();
    normalized = var_call
        .replace_all(&normalized, |captures: &regex::Captures| {
            let name = captures[1].trim();
            if let Some(class_id) = name.strip_prefix("BL=") {
                return draft.level_in_class(class_id).to_string();
            }
            state.variables.get(name).copied().unwrap_or(0).to_string()
        })
        .to_string();
    let total_level = Regex::new(r"\bTL\b").unwrap();
    normalized = total_level
        .replace_all(&normalized, draft.total_level().to_string())
        .to_string();
    normalized = normalized.replace("%CHOICE%", "0").replace("%LIST%", "0");
    normalized
}

fn formula_to_i32(value: &FormulaValue) -> i32 {
    match value {
        FormulaValue::Int(value) => *value as i32,
        FormulaValue::Float(value) => *value as i32,
        FormulaValue::Bool(value) => *value as i32,
        FormulaValue::String(_) => 0,
    }
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn resolve_placeholders(value: &str, state: &EffectState) -> String {
    let first = state.choices.values().next().cloned().unwrap_or_default();
    let list = state
        .choices
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    value
        .replace("%CHOICE%", &first)
        .replace("%choice%", &first)
        .replace("%LIST%", &list)
        .replace("%list%", &list)
}

fn grant_allowed(entity: &CompiledEntity, draft: &CharacterDraft, state: &EffectState) -> bool {
    entity
        .requirements
        .iter()
        .all(|predicate| predicate_holds(Some(predicate), draft, state))
}

fn structural_effect_skipped(
    category: EntityCategory,
    source_entity: Option<&pcgen_core::EntityId>,
    pack: &CompiledRulePack,
    target: &str,
) -> bool {
    let target = target.to_ascii_uppercase();
    match category {
        EntityCategory::Race => target.starts_with("STAT."),
        EntityCategory::Class | EntityCategory::ClassLevel => {
            if target.starts_with("STAT.") || target == "HP" {
                return true;
            }
            // Class BASEAB/BASE.SAVE is normally represented by the typed
            // progression table, so skip the raw BONUS to avoid double
            // counting.  Monster classes (Giant, Monstrous humanoid, ...)
            // have *no* progression table: their BAB/saves come only from
            // these formula BONUS tags, so they must be applied.
            let has_progression = source_entity
                .and_then(|id| class_progression_for(pack, &id.key))
                .is_some_and(class_progression_is_populated);
            (target.contains("BASEAB") || target.contains("BASE.")) && has_progression
        }
        // Equipment armor/shield AC is already represented by
        // EquipmentDefinition.armor_bonus; do not apply the raw BONUS twice.
        EntityCategory::Equipment => target.contains("AC") || target.contains("ARMOR"),
        _ => false,
    }
}

/// A progression is "populated" when it actually has table data.  Monster
/// classes can have an entry with empty vectors, which must be treated as
/// absent so their formula BONUS tags are applied instead.
fn class_progression_for<'a>(
    pack: &'a CompiledRulePack,
    entity_key: &str,
) -> Option<&'a pcgen_core::ClassProgression> {
    // Class-level entities carry keys such as `charlie_class#1`; the
    // progression is keyed by the class id only.
    let class_key = entity_key.split('#').next().unwrap_or(entity_key);
    let wanted = normalized(class_key);
    pack.progressions.get(class_key).or_else(|| {
        pack.progressions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_key) || normalized(key) == wanted)
            .map(|(_, progression)| progression)
    })
}

fn class_progression_is_populated(progression: &pcgen_core::ClassProgression) -> bool {
    !progression.base_attack_bonus.is_empty()
        || !progression.fortitude.is_empty()
        || !progression.reflex.is_empty()
        || !progression.will.is_empty()
}
