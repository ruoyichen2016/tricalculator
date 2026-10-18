//! Ability pool capacity and expenditure are resolved together here.
use crate::effects::EffectState;
use pcgen_core::{CharacterDraft, CompiledRulePack, EffectAction};
use pcgen_formula::VariableResolver;
use std::collections::BTreeMap;
fn normalized_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}
pub(crate) fn ability_pools(
    pack: &CompiledRulePack,
    effect_state: &EffectState,
    draft: &CharacterDraft,
    feat_slots: u32,
    diagnostics: &mut Vec<String>,
) -> BTreeMap<String, i32> {
    let mut ability_pools: BTreeMap<String, i32> = pack
        .ability_categories
        .iter()
        .filter_map(|(id, category)| {
            category.pool.as_ref().and_then(|pool| {
                // A category backed only by an inactive ability's declared variable
                // has no pool on this character (e.g. Ranger pools on a Fighter).
                let declared = pack.entities.iter().flat_map(|entity| &entity.effects).any(|effect| matches!(&effect.action, EffectAction::DefineVariable { name, .. } | EffectAction::DeclareVariable { name, .. } if name == pool));
                let selected = draft.abilities.iter().any(|(selected, _)| normalized_name(selected) == normalized_name(id) || selected.eq_ignore_ascii_case(&category.name));
                if declared && effect_state.resolve(pool).is_none() && !selected { return None; }
                let value = match pcgen_formula::Formula::parse(pool).and_then(|formula| formula.eval_floor_i32(effect_state)) {
                    Ok(value) => value,
                    result => {
                        diagnostics.push(format!(
                            "unresolved pool `{id}` formula `{pool}`: {result:?}"
                        ));
                        0
                    }
                };
                Some((id.clone(), value))
            })
        })
        .collect();
    // The game-mode FEAT pool includes the level progression even when
    // its category is supplied by the game mode rather than a .pcc file.
    let feat_pool_id = pack
        .ability_categories
        .iter()
        .find(|(id, category)| {
            normalized_name(id) == "feat" || category.name.eq_ignore_ascii_case("FEAT")
        })
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "feat".to_string());
    if feat_slots > 0 {
        ability_pools
            .entry(feat_pool_id)
            .or_insert(feat_slots as i32);
    }
    for (target, value) in &effect_state.external_modifiers {
        if let Some(name) = target.strip_prefix("ABILITYPOOL.") {
            let id = pack
                .ability_categories
                .iter()
                .find(|(id, category)| {
                    normalized_name(id) == normalized_name(name)
                        || category.name.eq_ignore_ascii_case(name)
                })
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| normalized_name(name));
            *ability_pools.entry(id).or_default() += value;
        }
    }
    match crate::class_features::sudden_slots(pack, draft) {
        Ok(slots) => {
            for slot in slots {
                ability_pools.insert(slot.pool, i32::from(slot.available));
            }
        }
        Err(error) => diagnostics.push(error),
    }
    // Explicit class reward lists are restrictions, not merely display hints.
    for (selected_pool, key) in &draft.abilities {
        let Some(c) = pack
            .ability_categories
            .iter()
            .find(|(id, c)| {
                normalized_name(id) == normalized_name(selected_pool)
                    || c.name.eq_ignore_ascii_case(selected_pool)
            })
            .map(|(_, c)| c)
        else {
            continue;
        };
        if c.ability_list.is_empty() {
            continue;
        }
        if !c
            .ability_list
            .iter()
            .any(|member| normalized_name(member) == normalized_name(key))
        {
            diagnostics.push(format!("{key} is not an allowed reward in {selected_pool}"));
            continue;
        }
        if let Some(allowed) = c
            .choice_restrictions
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
        {
            if !draft
                .choices
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .is_some_and(|(_, value)| allowed.iter().any(|v| v.eq_ignore_ascii_case(value)))
            {
                diagnostics.push(format!(
                    "{selected_pool}: {key} must choose {}",
                    allowed.join(", ")
                ));
            }
        }
        if let Some(feat) = pack
            .entities
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(key) || e.id.key.eq_ignore_ascii_case(key))
        {
            let mut before = draft.clone();
            before
                .abilities
                .retain(|(_, name)| !name.eq_ignore_ascii_case(key));
            let prior = crate::apply_effects(pack, &before);
            if feat
                .requirements
                .iter()
                .any(|p| crate::effects::predicate_result(Some(p), &before, &prior) != Some(true))
            {
                diagnostics.push(format!(
                    "{selected_pool}: prerequisites unmet or unavailable for {key}"
                ));
            }
        }
    }
    for (selected_pool, key) in &draft.abilities {
        let Some(entity) = pack.entities.iter().find(|e| {
            e.facts.contains_key("NativeRuleId")
                && (e.name.eq_ignore_ascii_case(key) || e.id.key.eq_ignore_ascii_case(key))
        }) else {
            continue;
        };
        let category = pack.ability_categories.iter().find(|(id, c)| {
            normalized_name(id) == normalized_name(selected_pool)
                || c.name.eq_ignore_ascii_case(selected_pool)
        });
        let eligible = category.is_some_and(|(id, c)| {
            ability_pools.contains_key(id)
                && c.category.as_ref() == entity.ability_category.as_ref()
                && (c.type_tags.is_empty()
                    || c.type_tags
                        .iter()
                        .any(|t| entity.type_tags.iter().any(|et| et.eq_ignore_ascii_case(t))))
        });
        if !eligible {
            diagnostics.push(format!(
                "{} cannot be selected from pool {}",
                entity.name, selected_pool
            ));
        }
    }
    ability_pools
}

pub(crate) fn ability_pool_usage(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    pools: &BTreeMap<String, i32>,
) -> BTreeMap<String, crate::snapshot::AbilityPoolUsage> {
    let mut usage = BTreeMap::new();
    for (category_id, category) in &pack.ability_categories {
        let Some(pool) = pools.get(category_id).copied() else {
            continue;
        };
        let mut selection_counts: BTreeMap<String, u32> = BTreeMap::new();
        for (selected_pool, key) in &draft.abilities {
            let key_norm = normalized_name(key);
            let explicit = category
                .ability_list
                .iter()
                .any(|member| normalized_name(member) == key_norm);
            let selected_here = normalized_name(selected_pool) == normalized_name(category_id)
                || selected_pool.eq_ignore_ascii_case(&category.name);
            let inferred = category.ability_list.is_empty()
                    // Typed pools (e.g. Fighter Feat) share a category with
                    // general feats, but a selection spends only its chosen pool.
                    && (selected_here || category.type_tags.is_empty())
                    && pack.abilities.values().any(|ability| {
                        (ability.id == key_norm || normalized_name(&ability.name) == key_norm)
                            && (category.type_tags.is_empty() || category.type_tags.iter().any(|kind|
                                ability.type_tags.iter().any(|tag| tag.eq_ignore_ascii_case(kind))))
                            && ability.category.as_deref().is_some_and(|ability_category| {
                                category.category.as_deref().is_some_and(|pool_category| {
                                    ability_category.eq_ignore_ascii_case(pool_category)
                                })
                            })
                    });
            if selected_here && (explicit || inferred) {
                *selection_counts.entry(key.clone()).or_insert(0) += 1;
            }
        }
        let mut selected: Vec<String> = selection_counts.keys().cloned().collect();
        selected.sort();
        let spent = selection_counts.values().sum::<u32>() as i32;
        usage.insert(
            category_id.clone(),
            crate::snapshot::AbilityPoolUsage {
                pool,
                spent,
                remaining: (pool - spent).max(0),
                selected,
                selection_counts,
            },
        );
    }
    if !usage.contains_key("feat") {
        if let Some(&pool) = pools.get("feat") {
            let mut selection_counts = BTreeMap::new();
            for (category, key) in &draft.abilities {
                if category.eq_ignore_ascii_case("FEAT") {
                    *selection_counts.entry(key.clone()).or_insert(0u32) += 1;
                }
            }
            let spent = selection_counts.values().sum::<u32>() as i32;
            usage.insert(
                "feat".into(),
                crate::snapshot::AbilityPoolUsage {
                    pool,
                    spent,
                    remaining: (pool - spent).max(0),
                    selected: selection_counts.keys().cloned().collect(),
                    selection_counts,
                },
            );
        }
    }
    usage
}
