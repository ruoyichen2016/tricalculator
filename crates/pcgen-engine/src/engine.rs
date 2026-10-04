//! Deterministic runtime engine.

use crate::draft::{CharacterDraft, DraftClassLevel};
use crate::effects::apply_effects;
use crate::progression::ClassProgress;
use crate::snapshot::CharacterSnapshot;
use pcgen_core::domain::{ClassDefinition, EquipmentDefinition, RaceDefinition};
use pcgen_core::{
    evaluate_faith, plan_gestalt, AdvancementLane, DivineTrack, FaithPolicy, GestaltPolicy, LaneId,
    SaveKind, VariantDefinition,
};
use pcgen_ir::CompiledRulePack;
use std::collections::{BTreeMap, BTreeSet, HashMap};

type SpellcastingMaps = (
    BTreeMap<String, i32>,
    BTreeMap<String, i32>,
    BTreeMap<String, u32>,
    BTreeMap<String, u32>,
    BTreeMap<String, u32>,
);

type PsionicMaps = (
    BTreeMap<String, i32>,
    BTreeMap<String, i32>,
    BTreeMap<String, Vec<String>>,
);

/// Aggregated creature traits merged from all active owner entities.
#[derive(Debug, Default, PartialEq, Eq)]
struct CreatureTraitAccumulator {
    vision: Vec<String>,
    damage_reduction: Vec<String>,
    spell_resistance: Option<i32>,
    natural_attacks: Vec<String>,
    reach_feet: Option<i32>,
    reach_mult: Option<i32>,
    unarmed_damage: Vec<String>,
    unarmed_mult: Option<i32>,
    attack_cycle: Vec<String>,
    move_clone: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Engine {
    pub pack: CompiledRulePack,
}

impl Engine {
    pub fn new(pack: CompiledRulePack) -> Self {
        Self { pack }
    }

    /// Override progression tables.  Useful for tests and small demos.
    pub fn with_progressions(mut self, progressions: BTreeMap<String, ClassProgress>) -> Self {
        self.pack.progressions = progressions;
        self
    }

    fn ability_pool_usage(
        &self,
        draft: &CharacterDraft,
        pools: &BTreeMap<String, i32>,
    ) -> BTreeMap<String, crate::snapshot::AbilityPoolUsage> {
        let mut usage = BTreeMap::new();
        for (category_id, category) in &self.pack.ability_categories {
            let Some(pool) = pools.get(category_id).copied() else {
                continue;
            };
            let mut selection_counts: BTreeMap<String, u32> = BTreeMap::new();
            for (_, key) in &draft.abilities {
                let key_norm = normalized_name(key);
                let explicit = category
                    .ability_list
                    .iter()
                    .any(|member| normalized_name(member) == key_norm);
                let inferred = category.ability_list.is_empty()
                    && self.pack.abilities.values().any(|ability| {
                        (ability.id == key_norm || normalized_name(&ability.name) == key_norm)
                            && ability.category.as_deref().is_some_and(|ability_category| {
                                category.category.as_deref().is_some_and(|pool_category| {
                                    ability_category.eq_ignore_ascii_case(pool_category)
                                })
                            })
                    });
                if explicit || inferred {
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
        usage
    }

    /// Ability-score modifiers for all core stats, honouring typed `STATMOD`
    /// formulas where present.
    fn ability_modifiers(&self, stats: &BTreeMap<String, i32>) -> BTreeMap<String, i32> {
        let mut modifiers = BTreeMap::new();
        for stat in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
            let score = stats.get(stat).copied().unwrap_or(10);
            modifiers.insert(
                stat.to_string(),
                stat_modifier_for_pack(&self.pack, stat, score),
            );
        }
        modifiers
    }

    /// Apply `DEFINESTAT:MINVALUE` from the active race and templates.
    fn apply_stat_minimums(
        &self,
        stats: &mut BTreeMap<String, i32>,
        draft: &CharacterDraft,
        effect_state: &crate::effects::EffectState,
    ) {
        let mut minimums: BTreeMap<String, i32> = BTreeMap::new();
        if let Some(race) = self.race(&draft.race_id) {
            for (stat, minimum) in &race.stat_minimums {
                minimums
                    .entry(stat.to_ascii_uppercase())
                    .and_modify(|value| *value = (*value).max(*minimum))
                    .or_insert(*minimum);
            }
        }
        for template_name in &effect_state.templates {
            if let Some(template) = find_template(&self.pack, template_name) {
                for (stat, minimum) in &template.stat_minimums {
                    minimums
                        .entry(stat.to_ascii_uppercase())
                        .and_modify(|value| *value = (*value).max(*minimum))
                        .or_insert(*minimum);
                }
            }
        }
        for (stat, minimum) in minimums {
            let entry = stats.entry(stat).or_insert(minimum);
            *entry = (*entry).max(minimum);
        }
    }

    /// Rank the draft race size, using the compiled `SIZE` records'
    /// `SIZENUM` values when available and falling back to the canonical d20
    /// order otherwise.
    fn size_rank(&self, race_id: &str) -> usize {
        let Some(race) = self.race(race_id) else {
            return 4;
        };
        let normalized = pcgen_core::normalize_size(&race.size);
        let abbreviation = normalized
            .chars()
            .next()
            .map(|ch| ch.to_ascii_uppercase().to_string())
            .unwrap_or_default();
        let definition = self.pack.sizes.get(&abbreviation).or_else(|| {
            self.pack.sizes.iter().find_map(|(key, definition)| {
                (key.eq_ignore_ascii_case(&abbreviation)
                    || definition.id.eq_ignore_ascii_case(&abbreviation)
                    || pcgen_core::normalize_size(&definition.name) == normalized)
                    .then_some(definition)
            })
        });
        if let Some(size_number) = definition
            .and_then(|definition| definition.size_number.as_deref())
            .and_then(|raw| raw.trim().parse::<f64>().ok())
        {
            // RSRD uses 010..090 for Fine..Colossal.
            if size_number >= 10.0 {
                return (size_number / 10.0).round().max(1.0) as usize - 1;
            }
        }
        pcgen_core::size_rank(&race.size)
    }

    pub fn evaluate(&self, draft: &CharacterDraft) -> CharacterSnapshot {
        let mut diagnostics = Vec::new();
        let effect_state = apply_effects(&self.pack, draft);
        diagnostics.extend(effect_state.diagnostics.clone());
        let class_levels = effective_class_levels(&self.pack, draft);
        let mut stats = draft.stats.clone();
        if let Some(race) = self.race(&draft.race_id) {
            for (stat, adjustment) in &race.ability_adjustments {
                *stats.entry(stat.clone()).or_insert(10) += adjustment;
            }
        }
        // Template/class STAT modifiers are resolved through the effect
        // pipeline, then folded into the ability scores before derived values
        // (HP, saves, AC, skills) are computed.
        for (target, value) in &effect_state.external_modifiers {
            if let Some(stats_part) = target.strip_prefix("STAT.") {
                for stat in stats_part.split(',') {
                    let stat = stat.trim().to_ascii_uppercase();
                    if matches!(stat.as_str(), "STR" | "DEX" | "CON" | "INT" | "WIS" | "CHA") {
                        *stats.entry(stat).or_insert(10) += value;
                    }
                }
            }
        }
        self.apply_stat_minimums(&mut stats, draft, &effect_state);
        let size_rank = self.size_rank(&draft.race_id);
        let size_modifier = size_modifier_from_rank(size_rank);
        let total_level = if draft.is_gestalt() {
            draft.total_level()
        } else {
            class_levels.iter().map(|entry| entry.level).sum()
        };
        let stat_mods = self.ability_modifiers(&stats);
        let con_mod = stat_mods.get("CON").copied().unwrap_or(0);
        let dex_mod = stat_mods.get("DEX").copied().unwrap_or(0);
        let equipment = self.resolve_equipment(draft, &mut diagnostics);
        let (armor_bonus, shield_bonus, max_dex, armor_check_penalty) =
            equipment_totals(&equipment);
        let dex_bonus = match max_dex {
            Some(cap) => dex_mod.min(cap),
            None => dex_mod,
        };
        let gestalt = if draft.is_gestalt() {
            Some(plan_gestalt(
                &draft.advancement,
                self,
                stat_mods.get("INT").copied().unwrap_or(0),
                &GestaltPolicy::default(),
            ))
        } else {
            None
        };
        let divine_tracks = collect_divine_tracks(draft);
        let faith_decisions = evaluate_faith(&divine_tracks, FaithPolicy::default());
        for decision in &faith_decisions {
            for error in &decision.errors {
                diagnostics.push(error.clone());
            }
            for warning in &decision.warnings {
                diagnostics.push(warning.clone());
            }
        }
        let domain_spells = {
            let mut spells = BTreeMap::new();
            for track in &divine_tracks {
                for domain in &track.domains {
                    if let Some(definition) = self.domain(domain) {
                        spells.insert(definition.id.clone(), definition.domain_spells.clone());
                    }
                }
            }
            for entry in &class_levels {
                for domain in self.class_added_domains(&entry.class_id, &effect_state.templates) {
                    if let Some(definition) = self.domain(&domain) {
                        spells.insert(definition.id.clone(), definition.domain_spells.clone());
                    }
                }
            }
            spells
        };
        // P5-8: fold numeric prepared/domain spell bonuses into the same
        // derived values the effect pipeline feeds.  The map is computed here
        // because it depends on deity/domain resolution above.
        let spell_bonuses = self.spell_numeric_bonuses(draft, &domain_spells);
        for bonus in &spell_bonuses {
            if let Some(stat) = bonus.target.to_ascii_uppercase().strip_prefix("STAT.") {
                let stat = stat.trim().to_ascii_uppercase();
                if matches!(stat.as_str(), "STR" | "DEX" | "CON" | "INT" | "WIS" | "CHA") {
                    *stats.entry(stat).or_insert(10) += bonus.value;
                }
            }
        }
        let all_spell_typed_bonuses: Vec<crate::effects::TypedBonus> = spell_bonuses
            .iter()
            .map(|bonus| crate::effects::TypedBonus {
                target: bonus.target.clone(),
                value: bonus.value,
                bonus_type: Some("spell".to_string()),
                stack_key: None,
            })
            .collect();
        let retain_dex_flat_footed = effect_state.active_abilities.iter().any(|ability| {
            let ability = ability.to_ascii_lowercase();
            ability.contains("retain_dex") || ability.contains("uncanny_dodge")
        });
        let mut all_typed_bonuses = effect_state.typed_bonuses.clone();
        all_typed_bonuses.extend(all_spell_typed_bonuses.iter().cloned());
        let (effect_ac, effect_touch, effect_flat) = ac_bonus_components(&all_typed_bonuses);
        let armor_class = 10 + armor_bonus + shield_bonus + dex_bonus + size_modifier + effect_ac;
        let touch_ac = 10 + dex_mod + size_modifier + effect_touch;
        let flat_footed_dex = if retain_dex_flat_footed {
            dex_bonus
        } else {
            dex_bonus.min(0)
        };
        let flat_footed_ac =
            10 + armor_bonus + shield_bonus + flat_footed_dex + size_modifier + effect_flat;

        let variant_effects = {
            let mut effects = BTreeMap::new();
            for frame in &draft.advancement {
                for lane in &frame.lanes {
                    for variant in self.variants_for(lane, &lane.class_id) {
                        if !variant.bonus_tags.is_empty() {
                            effects.insert(variant.id.clone(), variant.bonus_tags.clone());
                        }
                    }
                }
            }
            effects
        };
        diagnostics.extend(self.validate(draft));

        // Hit points are resolved per class level so PCGen's per-level
        // minimum of 1 HP is preserved.  Classes with zero HD/zero rolls stay
        // at 0, matching PCGen test/monster class records.
        let hit_points = if let Some(plan) = &gestalt {
            (plan
                .frames
                .iter()
                .map(|frame| frame.hit_points)
                .sum::<i32>()
                + con_mod * total_level as i32)
                .max(1)
        } else if class_levels.is_empty() {
            1
        } else {
            class_levels
                .iter()
                .map(|entry| self.hit_points_for_entry(draft, entry, con_mod))
                .sum()
        };

        let mut saves = if let Some(plan) = &gestalt {
            let mut saves = BTreeMap::new();
            for (kind, key) in [
                (SaveKind::Fortitude, "Fortitude"),
                (SaveKind::Reflex, "Reflex"),
                (SaveKind::Will, "Will"),
            ] {
                let total = plan
                    .frames
                    .iter()
                    .map(|frame| match kind {
                        SaveKind::Fortitude => frame.fortitude_increment,
                        SaveKind::Reflex => frame.reflex_increment,
                        SaveKind::Will => frame.will_increment,
                    })
                    .sum::<i32>()
                    + save_ability_modifier(&stat_mods, kind);
                saves.insert(key.to_string(), total);
            }
            saves
        } else {
            let mut fort = save_ability_modifier(&stat_mods, SaveKind::Fortitude);
            let mut reflex = save_ability_modifier(&stat_mods, SaveKind::Reflex);
            let mut will = save_ability_modifier(&stat_mods, SaveKind::Will);
            for entry in &class_levels {
                let Some(class) = self.class(&entry.class_id) else {
                    diagnostics.push(format!(
                        "missing progression for class `{}`",
                        entry.class_id
                    ));
                    continue;
                };
                let progress = self.progression(&entry.class_id);
                fort += class
                    .fortitude_formula
                    .as_deref()
                    .map(|formula| {
                        evaluate_class_formula(
                            formula,
                            entry.level,
                            total_level,
                            &effect_state.variables,
                        )
                    })
                    .or_else(|| {
                        progress.map(|progress| progress.save(entry.level, SaveKind::Fortitude))
                    })
                    .unwrap_or(0);
                reflex += class
                    .reflex_formula
                    .as_deref()
                    .map(|formula| {
                        evaluate_class_formula(
                            formula,
                            entry.level,
                            total_level,
                            &effect_state.variables,
                        )
                    })
                    .or_else(|| {
                        progress.map(|progress| progress.save(entry.level, SaveKind::Reflex))
                    })
                    .unwrap_or(0);
                will += class
                    .will_formula
                    .as_deref()
                    .map(|formula| {
                        evaluate_class_formula(
                            formula,
                            entry.level,
                            total_level,
                            &effect_state.variables,
                        )
                    })
                    .or_else(|| progress.map(|progress| progress.save(entry.level, SaveKind::Will)))
                    .unwrap_or(0);
            }
            let mut saves = BTreeMap::new();
            saves.insert("Fortitude".to_string(), fort);
            saves.insert("Reflex".to_string(), reflex);
            saves.insert("Will".to_string(), will);
            saves
        };
        for (save, total) in stacked_components(&all_typed_bonuses, save_bonus_components) {
            *saves.entry(save).or_insert(0) += total;
        }

        let base_attack_bonus = crate::bab::compute_bab(
            &self.pack,
            draft,
            &effect_state.variables,
            &effect_state.external_modifiers,
        );

        // Recompute ability modifiers for skill math using post-spell stats:
        // `stat_mods` was computed before spell STAT bonuses were folded.
        let skill_stat_mods = self.ability_modifiers(&stats);
        let mut skills = self.skills(draft, &skill_stat_mods, armor_check_penalty);
        for (skill, total) in stacked_components(&all_typed_bonuses, skill_bonus_components) {
            // Apply typed skill bonuses.  Skills already tracked by the draft
            // are updated in place; untrained-but-usable skills that only
            // receive a typed bonus are materialized with their 0-rank base
            // (ability modifier + armor check), unless they are trained-only.
            if let Some(entry) = skills
                .iter_mut()
                .find(|(name, _)| name.eq_ignore_ascii_case(&skill))
            {
                *entry.1 += total;
                continue;
            }
            if let Some(base) =
                self.untrained_skill_base(&skill, &skill_stat_mods, armor_check_penalty)
            {
                skills.insert(skill.clone(), base + total);
            }
        }
        let attack_bonuses = stacked_components(&all_typed_bonuses, attack_bonus_targets);
        let (caster_levels, spell_dcs, spell_slots, known_limits, prepared_spells) =
            self.spellcasting(draft, &class_levels, &stat_mods);
        let (power_points, manifester_levels, powers_known) =
            self.psionics(&class_levels, &stat_mods);
        let skill_points = if let Some(plan) = &gestalt {
            plan.frames
                .iter()
                .map(|frame| frame.skill_points as i32)
                .sum()
        } else {
            self.total_skill_points(draft, &class_levels, &stat_mods)
        };
        let feat_slots = self.feat_slots(&class_levels);
        let class_interactions = class_levels
            .iter()
            .filter_map(|entry| {
                let progression = self.progression(&entry.class_id)?;
                if progression.excluded_class.is_none() && progression.exchange_level.is_none() {
                    return None;
                }
                Some(crate::snapshot::ClassInteraction {
                    class_id: entry.class_id.clone(),
                    excluded_class: progression.excluded_class.clone(),
                    exchange_level: progression.exchange_level.clone(),
                })
            })
            .collect();
        let prohibited_costs: BTreeMap<String, i32> = self
            .pack
            .entities
            .iter()
            .filter_map(|entity| {
                entity
                    .metadata
                    .prohibited_cost
                    .map(|cost| (entity.id.key.clone(), cost))
            })
            .collect();
        let ability_lists: BTreeMap<String, Vec<String>> = self
            .pack
            .ability_categories
            .iter()
            .map(|(id, category)| (id.clone(), category.ability_list.clone()))
            .collect();
        let ability_pools: BTreeMap<String, i32> = self
            .pack
            .ability_categories
            .iter()
            .filter_map(|(id, category)| {
                category.pool.as_ref().map(|pool| {
                    (
                        id.clone(),
                        effect_state.variables.get(pool).copied().unwrap_or(0),
                    )
                })
            })
            .collect();
        let ability_pool_usage = self.ability_pool_usage(draft, &ability_pools);
        for (category_id, usage) in &ability_pool_usage {
            if usage.spent > usage.pool {
                diagnostics.push(format!(
                    "ability pool `{}` overspent: spent {} of {}",
                    category_id, usage.spent, usage.pool
                ));
            }
            for (ability, count) in &usage.selection_counts {
                if *count > 1 {
                    let multi = self
                        .pack
                        .abilities
                        .get(&normalized_name(ability))
                        .and_then(|definition| definition.multi)
                        .unwrap_or(false);
                    if !multi {
                        diagnostics.push(format!(
                            "ability `{ability}` selected {count} times without MULT"
                        ));
                    }
                }
            }
        }
        let mut repeatable_abilities: Vec<String> = self
            .pack
            .abilities
            .iter()
            .filter(|(_, ability)| ability.stack == Some(true) || ability.multi == Some(true))
            .map(|(id, _)| id.clone())
            .collect();
        repeatable_abilities.sort();
        repeatable_abilities.dedup();
        let mut languages: Vec<String> = effect_state.languages.iter().cloned().collect();
        languages.sort();
        languages.dedup();
        let class_choices: BTreeMap<String, String> = self
            .pack
            .classes
            .iter()
            .filter_map(|(id, class)| {
                class
                    .subclass_choice
                    .as_ref()
                    .map(|choice| (id.clone(), choice.clone()))
            })
            .collect();
        let CreatureTraitAccumulator {
            vision,
            damage_reduction,
            spell_resistance,
            natural_attacks,
            reach_feet,
            reach_mult,
            unarmed_damage,
            unarmed_mult,
            attack_cycle,
            move_clone,
        } = self.creature_traits(draft, &effect_state);
        let movement = resolve_movement(&practiced_movement(draft, &effect_state), &move_clone);
        let companions = self.companions(draft);
        let mut spell_effects = Vec::new();
        for (domain, spells) in &domain_spells {
            for spell in spells {
                spell_effects.push(format!(
                    "domain:{}:level {}:{}",
                    domain, spell.level, spell.name
                ));
            }
        }
        for (spell, count) in &prepared_spells {
            spell_effects.push(format!("prepared:{}:x{}", spell, count));
        }
        spell_effects.sort();
        spell_effects.dedup();
        let applied_spell_effects: BTreeMap<String, Vec<crate::snapshot::SpellEffectApplication>> =
            spell_bonuses
                .iter()
                .fold(BTreeMap::new(), |mut acc, bonus| {
                    acc.entry(format!("{}:{}", bonus.target, bonus.spell))
                        .or_default()
                        .push(bonus.clone());
                    acc
                });

        let explanations = BTreeMap::from([
            (
                "base_attack_bonus".to_string(),
                vec![pcgen_core::Contribution::included(
                    "class progression",
                    base_attack_bonus,
                    "class BAB progression + external BASEAB modifiers",
                )],
            ),
            (
                "armor_class".to_string(),
                vec![
                    pcgen_core::Contribution::included("base", 10, "base armor class"),
                    pcgen_core::Contribution::included("armor", armor_bonus, "worn armor"),
                    pcgen_core::Contribution::included("shield", shield_bonus, "worn shield"),
                    pcgen_core::Contribution::included("DEX", dex_bonus, "dexterity modifier"),
                    pcgen_core::Contribution::included("size", size_modifier, "size modifier"),
                ],
            ),
        ]);

        let explicit_prepared_count = draft
            .spells
            .iter()
            .filter(|spell| spell.times_memorized > 0 && is_prepared_book(&spell.book))
            .map(|spell| spell.times_memorized)
            .sum();
        let strength_mod = stat_mods.get("STR").copied().unwrap_or(0);
        let melee_attack = base_attack_bonus + strength_mod + size_modifier;
        let ranged_attack = base_attack_bonus + dex_mod + size_modifier;
        let grapple_attack =
            base_attack_bonus + strength_mod + grapple_size_modifier_from_rank(size_rank);
        let caster_level_bonus = self.domain_caster_level_bonus(draft);
        let mut snapshot = CharacterSnapshot {
            draft_id: draft.id.clone(),
            name: draft.name.clone(),
            race_id: draft.race_id.clone(),
            total_level,
            size_rank,
            stats,
            hit_points,
            skill_points,
            feat_slots,
            base_attack_bonus,
            melee_attack,
            ranged_attack,
            grapple_attack,
            class_interactions,
            prohibited_costs,
            ability_lists,
            ability_pools,
            ability_pool_usage,
            repeatable_abilities,
            languages,
            class_choices,
            deity_domains: draft.deity_domains.clone(),
            initiative: dex_mod,
            armor_class,
            armor_bonus,
            shield_bonus,
            touch_ac,
            flat_footed_ac,
            saves,
            skills,
            attack_bonuses,
            caster_levels,
            caster_level_bonus,
            explicit_prepared_count,
            spell_dcs,
            spell_slots,
            known_limits,
            prepared_spells,
            spell_power_types: effect_state.spell_powers.clone(),
            max_spell_level: effect_state.max_spell_level.clone(),
            has_spellbook: effect_state.has_spellbook,
            power_points,
            manifester_levels,
            powers_known,
            vision,
            damage_reduction,
            spell_resistance,
            natural_attacks,
            reach_feet,
            reach_mult,
            unarmed_damage,
            unarmed_mult,
            attack_cycle,
            movement,
            equipment,
            companions,
            gestalt,
            faith_decisions,
            domain_spells,
            spell_effects,
            applied_spell_effects,
            variant_effects,
            explanations,
            diagnostics,
            variables: effect_state.variables,
            external_modifiers: effect_state.external_modifiers,
            active_abilities: effect_state.active_abilities.into_iter().collect(),
            choices: effect_state.choices,
            choice_bindings: effect_state.choice_bindings,
            active_states: {
                let mut states = draft.active_states.clone();
                states.sort();
                states.dedup();
                states
            },
            unresolved_choices: effect_state.unresolved_choices,
            applied_effects: effect_state.applied_effects,
            provenance: pcgen_core::BuildProvenance::new(
                self.pack.header.rules_fingerprint.clone(),
                self.pack.header.data_ref.clone(),
                self.pack.header.game_mode.clone(),
            ),
        };
        let external_modifiers = snapshot.external_modifiers.clone();
        apply_external_modifiers(&mut snapshot, &external_modifiers);
        snapshot
    }

    fn creature_traits(
        &self,
        draft: &CharacterDraft,
        effect_state: &crate::effects::EffectState,
    ) -> CreatureTraitAccumulator {
        let mut merged = CreatureTraitAccumulator::default();
        let class_levels = effective_class_levels(&self.pack, draft);

        for entity in &self.pack.entities {
            if !entity_is_active_for_traits(entity, draft, effect_state, &class_levels) {
                continue;
            }
            merge_creature_traits(&entity.traits, &mut merged);
        }
        for traits in self.equipmod_traits(draft) {
            merge_creature_traits(&traits, &mut merged);
        }

        merged.vision.sort();
        merged.vision.dedup();
        merged.damage_reduction.sort();
        merged.damage_reduction.dedup();
        merged.natural_attacks.sort();
        merged.natural_attacks.dedup();
        merged.unarmed_damage.sort();
        merged.unarmed_damage.dedup();
        merged.attack_cycle.sort();
        merged.attack_cycle.dedup();
        merged.move_clone.sort();
        merged.move_clone.dedup();
        merged
    }

    /// Creature traits contributed by equipment modifiers (`EQMOD`)
    /// referenced by the character's equipped items.
    fn equipmod_traits(&self, draft: &CharacterDraft) -> Vec<pcgen_core::CreatureTraits> {
        let mut traits = Vec::new();
        for item in &draft.equipment {
            let key = item.trim().to_ascii_lowercase();
            let Some(modifiers) = draft.equipment_modifiers.get(&key) else {
                continue;
            };
            for modifier in modifiers {
                if let Some(definition) = self.equipment_modifier(modifier) {
                    if definition.traits != pcgen_core::CreatureTraits::default() {
                        traits.push(definition.traits.clone());
                    }
                }
            }
        }
        traits
    }

    fn equipment_modifier(&self, key: &str) -> Option<&pcgen_core::EquipmentModifierDefinition> {
        self.pack.equipment_modifiers.get(key).or_else(|| {
            self.pack
                .equipment_modifiers
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
                .map(|(_, definition)| definition)
        })
    }

    /// Validate structural references and Gestalt policy constraints.
    pub fn validate(&self, draft: &CharacterDraft) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for entry in effective_class_levels(&self.pack, draft) {
            if self.class(&entry.class_id).is_none() {
                diagnostics.push(format!("unknown class `{}`", entry.class_id));
            }
        }
        if draft.is_gestalt() {
            let plan = plan_gestalt(
                &draft.advancement,
                self,
                self.ability_modifiers(&draft.stats)
                    .get("INT")
                    .copied()
                    .unwrap_or(0),
                &GestaltPolicy::default(),
            );
            for diagnostic in &plan.diagnostics {
                if diagnostic.code.starts_with("gestalt.prestige_")
                    || diagnostic.code == "gestalt.dual_progression"
                {
                    diagnostics.push(diagnostic.message.clone());
                }
            }
        }
        diagnostics.extend(self.faith_checks(draft));
        diagnostics.extend(self.variant_checks(draft));
        diagnostics
    }

    /// Diagnose variant selections that cannot be applied as written.
    ///
    /// Two lanes can legitimately pick different archetypes, but two variants
    /// that replace the *same* feature on the same class leave the result
    /// undefined (whichever is applied last wins), so it is reported instead of
    /// silently resolved.
    fn variant_checks(&self, draft: &CharacterDraft) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for frame in &draft.advancement {
            for lane in &frame.lanes {
                let variants = self.variants_for(lane, &lane.class_id);
                if variants.len() < 2 {
                    continue;
                }
                for (index, left) in variants.iter().enumerate() {
                    for right in variants.iter().skip(index + 1) {
                        let left_targets = left.replace_targets();
                        let overlap: Vec<&str> = left_targets
                            .iter()
                            .filter(|target| {
                                right.replace_targets().iter().any(|other| other == *target)
                            })
                            .map(|target| match target {
                                pcgen_core::ReplaceTarget::TypeTag(tag) => tag.as_str(),
                                pcgen_core::ReplaceTarget::Name(name) => name.as_str(),
                            })
                            .collect();
                        if !overlap.is_empty() {
                            diagnostics.push(format!(
                                "variants `{}` and `{}` both replace `{}` on class `{}`; \
                                 the result is undefined",
                                left.name,
                                right.name,
                                overlap.join("`, `"),
                                lane.class_id
                            ));
                        }
                    }
                }
            }
        }
        diagnostics
    }

    fn faith_checks(&self, draft: &CharacterDraft) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for track in &collect_divine_tracks(draft) {
            if let Some(deity) = &track.deity {
                if !draft.deity_domains.is_empty() {
                    for domain in &track.domains {
                        if !draft
                            .deity_domains
                            .iter()
                            .any(|known| known.eq_ignore_ascii_case(domain))
                        {
                            diagnostics.push(format!(
                                "domain `{domain}` is not granted by deity `{deity}`"
                            ));
                        }
                    }
                }
                match self.deity(deity) {
                    Some(definition) => {
                        if definition.valid_for_deity == Some(false) {
                            diagnostics
                                .push(format!("deity `{deity}` is not valid for deity selection"));
                        }
                        if definition.valid_for_follower == Some(false) {
                            diagnostics.push(format!("deity `{deity}` is not valid for followers"));
                        }
                        let catalog: Vec<String> = if draft.deity_domains.is_empty() {
                            definition.domains.clone()
                        } else {
                            draft.deity_domains.clone()
                        };
                        for domain in &track.domains {
                            if !catalog.is_empty()
                                && !catalog
                                    .iter()
                                    .any(|known| known.eq_ignore_ascii_case(domain))
                            {
                                diagnostics.push(format!(
                                    "domain `{domain}` is not granted by deity `{deity}`"
                                ));
                            }
                        }
                        if let Some(alignment) = &definition.alignment {
                            if !alignment.eq_ignore_ascii_case(&draft.alignment) {
                                diagnostics.push(format!(
                                    "deity `{deity}` alignment `{alignment}` does not match character alignment `{}`",
                                    draft.alignment
                                ));
                            }
                        }
                        if let Some(weapon) = &definition.favored_weapon {
                            let has_weapon = draft.equipment.iter().any(|item| {
                                item.eq_ignore_ascii_case(weapon)
                                    || item
                                        .to_ascii_lowercase()
                                        .contains(&weapon.to_ascii_lowercase())
                            });
                            if !has_weapon {
                                diagnostics.push(format!(
                                    "deity `{deity}` favored weapon `{weapon}` is not equipped"
                                ));
                            }
                        }
                    }
                    None => diagnostics.push(format!("unknown deity `{deity}`")),
                }
            }
            for domain in &track.domains {
                if self.domain(domain).is_none() {
                    diagnostics.push(format!("unknown domain `{domain}`"));
                }
            }
        }
        if let Some(alignment) = self.alignment(&draft.alignment) {
            if alignment.valid_for_deity == Some(false) {
                diagnostics.push(format!(
                    "alignment `{}` is not valid for deity selection",
                    draft.alignment
                ));
            }
            if alignment.valid_for_follower == Some(false) {
                diagnostics.push(format!(
                    "alignment `{}` is not valid for followers",
                    draft.alignment
                ));
            }
        }
        diagnostics
    }

    fn alignment(&self, alignment: &str) -> Option<&pcgen_core::AlignmentDefinition> {
        self.pack.alignments.get(alignment).or_else(|| {
            self.pack
                .alignments
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(alignment))
                .map(|(_, definition)| definition)
        })
    }

    fn deity(&self, deity: &str) -> Option<&pcgen_core::DeityDefinition> {
        self.pack.deities.get(deity).or_else(|| {
            self.pack
                .deities
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(deity))
                .map(|(_, deity)| deity)
        })
    }

    fn class_added_domains(&self, class_id: &str, templates: &BTreeSet<String>) -> Vec<String> {
        let raw_values = self
            .class(class_id)
            .map(|class| class.added_domains.clone())
            .or_else(|| {
                self.pack
                    .entities
                    .iter()
                    .find(|entity| {
                        entity.category == pcgen_core::EntityCategory::Class
                            && (normalized_name(&entity.id.key) == normalized_name(class_id)
                                || normalized_name(&entity.name) == normalized_name(class_id))
                    })
                    .map(|entity| entity.metadata.added_domains.clone())
            })
            .unwrap_or_default();
        raw_values
            .iter()
            .filter_map(|raw| resolve_added_domain(raw, templates))
            .collect()
    }

    fn domain(&self, domain: &str) -> Option<&pcgen_core::DomainDefinition> {
        self.pack.domains.get(domain).or_else(|| {
            self.pack
                .domains
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(domain))
                .map(|(_, domain)| domain)
        })
    }

    /// Returns all variants attached to a lane: archetype first, then
    /// substitution-level.  Both may coexist on the same lane.
    fn variants_for(&self, lane: &AdvancementLane, class_id: &str) -> Vec<&VariantDefinition> {
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
                self.pack.variants.get(id).or_else(|| {
                    self.pack
                        .variants
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

    fn race(&self, race_id: &str) -> Option<&RaceDefinition> {
        self.pack.races.get(race_id).or_else(|| {
            self.pack
                .races
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(race_id))
                .map(|(_, race)| race)
        })
    }

    pub fn progression(&self, class_id: &str) -> Option<&ClassProgress> {
        self.pack.progressions.get(class_id).or_else(|| {
            self.pack
                .progressions
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(class_id))
                .map(|(_, progression)| progression)
        })
    }

    fn class(&self, class_id: &str) -> Option<&ClassDefinition> {
        self.pack.classes.get(class_id).or_else(|| {
            self.pack
                .classes
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(class_id))
                .map(|(_, class)| class)
        })
    }

    fn resolve_equipment(
        &self,
        draft: &CharacterDraft,
        diagnostics: &mut Vec<String>,
    ) -> Vec<EquipmentDefinition> {
        let mut resolved: Vec<EquipmentDefinition> = draft
            .equipment
            .iter()
            .filter_map(|item| {
                let customised = draft
                    .equipment_base_items
                    .get(&item.to_ascii_lowercase())
                    .cloned();
                let display = draft
                    .equipment_display_names
                    .get(&item.to_ascii_lowercase())
                    .cloned();
                let lookup = display
                    .as_deref()
                    .or(customised.as_deref())
                    .unwrap_or(item.as_str())
                    .trim();
                let found = self
                    .pack
                    .equipment
                    .get(lookup)
                    .or_else(|| {
                        self.pack
                            .equipment
                            .iter()
                            .find(|(key, definition)| {
                                key.eq_ignore_ascii_case(lookup)
                                    || definition.name.eq_ignore_ascii_case(lookup)
                            })
                            .map(|(_, definition)| definition)
                    })
                    .cloned()
                    .map(|mut definition| {
                        // Keep the customised display name so slot/AC output
                        // still identifies the equipped item, but resolve the
                        // mechanical fields from the base item.
                        if let Some(base) = &customised {
                            definition.name = format!("{item} [{base}]");
                        }
                        // Apply `EQMOD=` enhancement bonuses (`PLUS2A`, ...)
                        // so customised armour/shield AC matches PCGen.
                        if let Some(modifiers) =
                            draft.equipment_modifiers.get(&item.to_ascii_lowercase())
                        {
                            for modifier in modifiers {
                                let upper = modifier.to_ascii_uppercase();
                                // `PLUS2A` = +2 to armour AC; `PLUS3S` = +3
                                // to shield AC.  Other modifiers (STEEL,
                                // MWORKA, BOWSTR) do not change AC directly.
                                if let Some(rest) = upper.strip_prefix("PLUS") {
                                    if let Some(value) = rest
                                        .strip_suffix('A')
                                        .and_then(|digits| digits.parse::<i32>().ok())
                                    {
                                        definition.armor_bonus += value;
                                    } else if let Some(value) = rest
                                        .strip_suffix('S')
                                        .and_then(|digits| digits.parse::<i32>().ok())
                                    {
                                        // Shield enhancement is additive on the
                                        // shield's AC component.
                                        definition.armor_bonus += value;
                                    }
                                }
                            }
                        }
                        definition
                    });
                if found.is_none() {
                    diagnostics.push(format!("missing equipment definition for `{item}`"));
                }
                found
            })
            .collect();
        pcgen_core::equipment::sort_equipment_by_slot(&mut resolved, |definition| definition.slot);
        for (slot, first, second) in pcgen_core::equipment::equipment_conflicts(
            resolved
                .iter()
                .map(|definition| (definition.slot, definition.id.as_str())),
        ) {
            diagnostics.push(format!(
                "equipment slot conflict: `{first}` and `{second}` both use {slot:?}"
            ));
        }
        resolved
    }

    fn skills(
        &self,
        draft: &CharacterDraft,
        stat_mods: &BTreeMap<String, i32>,
        armor_check_penalty: i32,
    ) -> BTreeMap<String, i32> {
        let mut skills = BTreeMap::new();
        for (name, ranks) in &draft.skills {
            let definition = self.pack.skills.get(name).or_else(|| {
                self.pack
                    .skills
                    .iter()
                    .find(|(key, skill)| {
                        key.eq_ignore_ascii_case(name) || skill.name.eq_ignore_ascii_case(name)
                    })
                    .map(|(_, skill)| skill)
            });
            let ability_mod = definition
                .and_then(|skill| skill.key_stat.as_deref())
                .and_then(|key_stat| stat_mods.get(&key_stat.to_ascii_uppercase()).copied())
                .unwrap_or(0);
            let armor_penalty = if definition.is_some_and(|skill| skill.armor_check) {
                armor_check_penalty
            } else {
                0
            };
            skills.insert(
                name.clone(),
                (ranks + f64::from(ability_mod) + f64::from(armor_penalty)).floor() as i32,
            );
        }
        skills
    }

    /// 0-rank base total for a skill that can be used untrained, or `None`
    /// when the skill is unknown or trained-only.
    fn untrained_skill_base(
        &self,
        skill: &str,
        stat_mods: &BTreeMap<String, i32>,
        armor_check_penalty: i32,
    ) -> Option<i32> {
        let definition = self.skill(skill)?;
        if definition.trained_only {
            return None;
        }
        let ability_mod = definition
            .key_stat
            .as_deref()
            .and_then(|key_stat| stat_mods.get(&key_stat.to_ascii_uppercase()).copied())
            .unwrap_or(0);
        let armor_penalty = if definition.armor_check {
            armor_check_penalty
        } else {
            0
        };
        Some(ability_mod + armor_penalty)
    }

    /// Max caster-level contribution from active domain grants.
    ///
    /// PCGen encodes this as free text in `DOMAINGRANTS`; this first step
    /// only exposes the max contribution needed by the deep-field oracle.
    fn domain_caster_level_bonus(&self, draft: &CharacterDraft) -> i32 {
        for domain in &draft.domains {
            if let Some(definition) = self.pack.domains.get(domain) {
                if definition
                    .granted_power
                    .as_deref()
                    .is_some_and(|grant| grant.to_ascii_lowercase().contains("caster level"))
                {
                    return 1;
                }
            }
        }
        0
    }

    fn skill(&self, name: &str) -> Option<&pcgen_core::SkillDefinition> {
        self.pack.skills.get(name).or_else(|| {
            self.pack
                .skills
                .iter()
                .find(|(key, skill)| {
                    key.eq_ignore_ascii_case(name) || skill.name.eq_ignore_ascii_case(name)
                })
                .map(|(_, skill)| skill)
        })
    }

    fn spellcasting(
        &self,
        draft: &CharacterDraft,
        class_levels: &[DraftClassLevel],
        stat_mods: &BTreeMap<String, i32>,
    ) -> SpellcastingMaps {
        let mut caster_levels = BTreeMap::new();
        let mut spell_slots = BTreeMap::new();
        let mut known_limits = BTreeMap::new();

        // PRC `ADD:SPELLCASTER|Arcane/Divine` levels stack onto an existing
        // base casting track instead of creating a separate track.
        let mut advanced_arcane = 0u32;
        let mut advanced_divine = 0u32;
        let mut advanced_any = 0u32;
        for entry in class_levels {
            let Some(class) = self.class(&entry.class_id) else {
                continue;
            };
            let advance_count = class
                .level_spellcaster_advance
                .keys()
                .filter(|level| **level <= entry.level)
                .count() as u32;
            let mut arcane = false;
            let mut divine = false;
            let mut any = false;
            for value in class.level_spellcaster_advance.values().flatten() {
                let upper = value.to_ascii_uppercase();
                if upper.contains("PSIONIC") {
                    continue;
                }
                arcane |= upper.contains("ARCANE");
                divine |= upper.contains("DIVINE");
                any |= upper.contains("SPELLCASTER");
            }
            if arcane {
                advanced_arcane += advance_count;
            } else if divine {
                advanced_divine += advance_count;
            } else if any {
                advanced_any += advance_count;
            }
        }
        let mut any_applied = false;

        for entry in class_levels {
            let Some(class) = self.class(&entry.class_id) else {
                continue;
            };
            if class.spellcasting_ability.is_none() || class.is_psionic {
                // Psionic classes are handled by `psionics`; keep them out of
                // the arcane/divine caster-level map.
                continue;
            }
            let advances_caster = class
                .level_spellcaster_advance
                .values()
                .flatten()
                .any(|value| {
                    let upper = value.to_ascii_uppercase();
                    !upper.contains("PSIONIC")
                        && (upper.contains("ARCANE")
                            || upper.contains("DIVINE")
                            || upper.contains("SPELLCASTER"))
                });
            if class.is_prestige && advances_caster {
                continue;
            }
            let power_advance = match class.spell_power {
                pcgen_core::SpellPowerType::Arcane => advanced_arcane,
                pcgen_core::SpellPowerType::Divine => advanced_divine,
                _ => 0,
            };
            let any_advance = if !any_applied && power_advance == 0 {
                any_applied = true;
                advanced_any
            } else {
                0
            };
            let variant_advance = draft.advancement.iter().any(|frame| {
                frame.lanes.iter().any(|lane| {
                    lane.class_id.eq_ignore_ascii_case(&class.id)
                        && self
                            .variants_for(lane, &class.id)
                            .iter()
                            .any(|variant| variant.spellcaster_advance)
                })
            });
            let effective_level =
                entry.level + power_advance + any_advance + u32::from(variant_advance);
            caster_levels.insert(class.id.clone(), effective_level as i32);
            if let Some(slots) = class.spell_slots.get(&effective_level) {
                for (level, count) in slots.iter().enumerate() {
                    spell_slots.insert(format!("{}|{}", class.id, level), *count);
                }
            }
            if let Some(known) = class.spells_known.get(&effective_level) {
                for (level, count) in known.iter().enumerate() {
                    known_limits.insert(format!("{}|{}", class.id, level), *count);
                }
            }
        }

        let mut spell_dcs = BTreeMap::new();
        let mut prepared_spells = BTreeMap::new();
        for spell in &draft.spells {
            let class_id = spell.class_id.clone().or_else(|| {
                class_levels.iter().find_map(|entry| {
                    self.class(&entry.class_id)
                        .filter(|class| class.spellcasting_ability.is_some())
                        .map(|class| class.id.clone())
                })
            });
            let Some(class_id) = class_id else {
                continue;
            };
            let class = self.class(&class_id);
            let stat = class
                .and_then(|class| class.spellcasting_ability.clone())
                .unwrap_or_else(|| "CHA".to_string());
            let level = if spell.level > 0 {
                spell.level
            } else {
                self.pack
                    .spells
                    .iter()
                    .find(|(key, candidate)| {
                        key.eq_ignore_ascii_case(&spell.name)
                            || candidate.name.eq_ignore_ascii_case(&spell.name)
                    })
                    .and_then(|(_, candidate)| candidate.class_levels.get(&class_id).copied())
                    .unwrap_or(0)
            };
            let dc = 10 + level as i32 + stat_mods.get(&stat).copied().unwrap_or(0);
            spell_dcs.insert(format!("{}|{}", spell.name, class_id), dc);
            if spell.times_memorized > 0 {
                prepared_spells.insert(
                    format!("{}|{}", class_id, spell.name),
                    spell.times_memorized,
                );
            }
        }
        (
            caster_levels,
            spell_dcs,
            spell_slots,
            known_limits,
            prepared_spells,
        )
    }

    fn psionics(
        &self,
        class_levels: &[DraftClassLevel],
        stat_mods: &BTreeMap<String, i32>,
    ) -> PsionicMaps {
        let mut base_tracks: BTreeMap<String, u32> = BTreeMap::new();
        let mut advanced_levels = 0u32;
        for entry in class_levels {
            let Some(class) = self.class(&entry.class_id) else {
                continue;
            };
            let advances_psionic = class
                .level_spellcaster_advance
                .values()
                .flatten()
                .any(|value| value.to_ascii_uppercase().contains("PSIONIC"));
            if class.is_psionic && !class.is_prestige {
                base_tracks.insert(class.id.clone(), entry.level);
            } else if advances_psionic {
                advanced_levels += entry.level;
            }
        }
        // A single PRC advancement track extends the (first) base manifester
        // class.  Full multi-track choice is a later UI concern.
        if advanced_levels > 0 {
            if let Some(first) = base_tracks.keys().next().cloned() {
                *base_tracks.entry(first).or_insert(0) += advanced_levels;
            }
        }
        let mut power_points = BTreeMap::new();
        let mut manifester_levels = BTreeMap::new();
        let mut powers_known = BTreeMap::new();
        for (class_id, level) in &base_tracks {
            let stat = self
                .class(class_id)
                .and_then(|class| class.spellcasting_ability.as_deref())
                .unwrap_or("INT");
            let ability_mod = stat_mods.get(stat).copied().unwrap_or(0);
            let total = pcgen_core::total_power_points(class_id, *level, ability_mod, 0);
            power_points.insert(class_id.clone(), total);
            manifester_levels.insert(class_id.clone(), *level as i32);
            let mut known: Vec<String> = self
                .pack
                .spells
                .values()
                .filter(|spell| {
                    spell.is_psionic
                        && spell.class_levels.iter().any(|(name, power_level)| {
                            name.eq_ignore_ascii_case(class_id) && *power_level <= *level
                        })
                })
                .map(|spell| spell.name.clone())
                .collect();
            known.sort();
            known.dedup();
            powers_known.insert(class_id.clone(), known);
        }
        (power_points, manifester_levels, powers_known)
    }

    /// Numeric spell effects applied to the character's derived values.
    ///
    /// v1 covers the small but deterministic subset of RSRD spells whose
    /// bonuses are unconditional (`BONUS:*` on the spell record): these are
    /// folded into the snapshot as typed bonuses and reported back so the
    /// Java differential can compare them.
    /// Numeric prepared/domain spell bonuses that should be folded into
    /// derived AC / save / stat / skill values (P5-7/P5-8).
    fn spell_numeric_bonuses(
        &self,
        draft: &CharacterDraft,
        domain_spells: &BTreeMap<String, Vec<pcgen_core::DomainSpell>>,
    ) -> Vec<crate::snapshot::SpellEffectApplication> {
        let mut bonus_values: std::collections::BTreeMap<
            String,
            crate::snapshot::SpellEffectApplication,
        > = std::collections::BTreeMap::new();
        // P5-8: v1 only folds *prepared* spells.  Domain spell lists and
        // spellbook-known lists are catalogs, not active buffs; applying them
        // would inflate AC/saves for ordinary characters.
        // P5-8 v2: numeric spell effects are gated to a small, explicit set
        // of unconditional buff spells.  PCGen test fixtures list many
        // prepared spells whose effects are instantaneous or situational;
        // folding those blindly would break Java agreement.
        const ACTIVE_BUFFS: &[&str] = &[
            "mage armor",
            "shield",
            "shield of faith",
            "bull's strength",
            "bear's endurance",
            "cat's grace",
            "eagle's splendor",
            "fox's cunning",
            "owl's wisdom",
            "bless",
            "bull's strength skill",
        ];
        let prepared: BTreeSet<String> = draft
            .spells
            .iter()
            .filter(|spell| spell.times_memorized > 0)
            .map(|spell| spell.name.to_ascii_lowercase())
            .collect();
        // `.pcg` `CANCASTPERDAY`/known spellbook entries do not mean the buff
        // is active; only an explicit prepared/memorized count does.  When the
        // draft has no prepared spell at all, keep the allowlist disabled so
        // existing characters are not inflated.
        let _ = domain_spells;
        for (spell_id, spell) in &self.pack.spells {
            let name_key = spell.name.to_ascii_lowercase();
            let active = ACTIVE_BUFFS.contains(&name_key.as_str()) && prepared.contains(&name_key);
            if !active {
                continue;
            }
            let effects = self
                .pack
                .entities
                .iter()
                .find(|entity| {
                    entity.category == pcgen_core::EntityCategory::Spell
                        && (normalized_name(&entity.id.key) == normalized_name(spell_id)
                            || normalized_name(&entity.name) == normalized_name(&spell.name))
                })
                .map(|entity| entity.effects.clone())
                .unwrap_or_default();
            for effect in effects {
                let (target, value) = match &effect.action {
                    pcgen_core::EffectAction::Modify { modifier } => {
                        (modifier.target.clone(), modifier.value.floor() as i32)
                    }
                    _ => continue,
                };
                let upper = target.to_ascii_uppercase();
                let applies = upper == "AC"
                    || upper == "COMBAT.AC"
                    || upper.starts_with("SAVE")
                    || upper.starts_with("STAT.")
                    || upper.starts_with("SKILL")
                    || upper.contains("TOHIT");
                if !applies {
                    continue;
                }
                let key = format!("{}:{}", upper, spell.name);
                let entry =
                    bonus_values
                        .entry(key)
                        .or_insert(crate::snapshot::SpellEffectApplication {
                            spell: spell.name.clone(),
                            class_id: spell.class_levels.keys().next().cloned(),
                            target: target.clone(),
                            value: 0,
                        });
                entry.value += value;
            }
        }
        bonus_values.into_values().collect()
    }

    /// Resolve HP for one class-level entry.
    ///
    /// Explicit `.pcg` HP rolls / gains always win.  When a class or template
    /// supplies `HITDIE`, a deterministic average (`die / 2 + 1` plus CON) is
    /// used for synthetic drafts that do not carry per-level HP input.
    fn hit_points_for_entry(
        &self,
        draft: &CharacterDraft,
        entry: &DraftClassLevel,
        con_mod: i32,
    ) -> i32 {
        if !entry.hit_points_by_level.is_empty() {
            let all_zero = entry.hit_points_by_level.iter().all(|roll| *roll == 0);
            return entry
                .hit_points_by_level
                .iter()
                .map(|roll| {
                    let adjusted = *roll + con_mod;
                    if all_zero {
                        adjusted.max(0)
                    } else {
                        adjusted.max(1)
                    }
                })
                .sum();
        }
        if entry.hit_points_gained != 0 {
            let raw = entry.hit_points_gained + con_mod * entry.level as i32;
            return raw.max(entry.level as i32);
        }
        let die = self.hit_die_override(draft, &entry.class_id).or_else(|| {
            self.class(&entry.class_id)
                .filter(|class| class.is_monster)
                .and_then(|class| parse_hit_die(&class.hit_die))
        });
        if let Some(die) = die {
            let per_level = (die / 2 + 1 + con_mod).max(1);
            return per_level * entry.level as i32;
        }
        // Preserve the legacy zero-HD / zero-rolls behavior for classes
        // without an explicit HITDIE override or monster hit die.
        (con_mod * entry.level as i32).max(0)
    }

    fn hit_die_override(&self, draft: &CharacterDraft, class_id: &str) -> Option<i32> {
        if let Some(class) = self.class(class_id) {
            if let Some(value) = class.hit_die_override.as_deref() {
                if let Some(die) = parse_hit_die(value) {
                    return Some(die);
                }
            }
        }
        for template_name in &draft.templates {
            if let Some(template) = find_template(&self.pack, template_name) {
                if let Some(value) = template.hit_die_override.as_deref() {
                    if let Some(die) = parse_hit_die(value) {
                        return Some(die);
                    }
                }
            }
        }
        None
    }

    /// Total skill points from class levels plus race
    /// `XTRASKILLPTSPERLVL`, using `SKILLMULT` for the first character level.
    fn total_skill_points(
        &self,
        draft: &CharacterDraft,
        class_levels: &[DraftClassLevel],
        stat_mods: &BTreeMap<String, i32>,
    ) -> i32 {
        let int_mod = stat_mods.get("INT").copied().unwrap_or(0);
        let race_extra = self
            .race(&draft.race_id)
            .and_then(|race| race.extra_skill_points_per_level)
            .unwrap_or(0) as i32;
        let race_mult = self
            .race(&draft.race_id)
            .and_then(|race| race.skill_multiplier);
        let mut total = 0;
        let mut first = true;
        for entry in class_levels {
            let Some(progression) = self.progression(&entry.class_id) else {
                continue;
            };
            let normal = (progression.skill_points_per_level as i32 + race_extra + int_mod).max(1);
            if first {
                let first_base = if let Some(multiplier) = race_mult {
                    progression
                        .skill_points_per_level
                        .saturating_mul(multiplier)
                } else {
                    progression.first_level_skill_points
                };
                total += (first_base as i32 + int_mod).max(1);
                total += normal * entry.level.saturating_sub(1) as i32;
                first = false;
            } else {
                total += normal * entry.level as i32;
            }
        }
        total
    }

    /// Feat slots from class `LEVELSPERFEAT` / default progression.
    fn feat_slots(&self, class_levels: &[DraftClassLevel]) -> u32 {
        let mut total = 0;
        for entry in class_levels {
            if let Some(progression) = self.progression(&entry.class_id) {
                total += progression.feat_slots(entry.level);
            } else {
                total += 1 + entry.level / 3;
            }
        }
        total
    }

    /// Derive companion states from the master's companions and the typed
    /// `FOLLOWER:` companion modifiers.
    fn companions(&self, draft: &CharacterDraft) -> Vec<crate::snapshot::CompanionSnapshot> {
        let mut models = Vec::new();
        for companion in &draft.companions {
            let mut model = crate::snapshot::CompanionSnapshot {
                companion_type: companion.companion_type.clone(),
                master_level: companion.master_level,
                race_id: companion.race_id.clone(),
                ..crate::snapshot::CompanionSnapshot::default()
            };
            for rows in self.pack.companion_mods.values() {
                for definition in rows {
                    if !definition
                        .companion_type
                        .eq_ignore_ascii_case(&companion.companion_type)
                    {
                        continue;
                    }
                    if !companion_row_matches(definition, companion) {
                        continue;
                    }
                    if definition.master_level > companion.master_level {
                        continue;
                    }
                    if let Some(raw) = &definition.spell_resistance {
                        if let Some(value) = evaluate_companion_sr(
                            raw,
                            definition.master_level,
                            companion.master_level,
                        ) {
                            model.spell_resistance = Some(
                                model
                                    .spell_resistance
                                    .map_or(value, |current| current.max(value)),
                            );
                        }
                    }
                    model.hit_dice_bonus += definition.hit_dice_bonus;
                    for ability in &definition.abilities {
                        if !model.abilities.iter().any(|existing| existing == ability) {
                            model.abilities.push(ability.clone());
                        }
                    }
                    merge_companion_traits(&definition.traits, &mut model.traits);
                    for bonus in &definition.bonuses {
                        let Some(value) =
                            evaluate_companion_formula(&bonus.formula, companion.master_level)
                        else {
                            continue;
                        };
                        let target = bonus.target.to_ascii_uppercase();
                        if target == "COMBAT.AC" || target == "AC" {
                            model.armor_class_bonus += value.floor() as i32;
                        } else if let Some(stat) = target.strip_prefix("STAT.") {
                            *model.stat_adjustments.entry(stat.to_string()).or_insert(0) +=
                                value.floor() as i32;
                        }
                    }
                }
            }
            model.abilities.sort();
            model.abilities.dedup();
            model.traits.vision.sort();
            model.traits.vision.dedup();
            model.traits.damage_reduction.sort();
            model.traits.damage_reduction.dedup();
            model.traits.natural_attacks.sort();
            model.traits.natural_attacks.dedup();
            model.traits.unarmed_damage.sort();
            model.traits.unarmed_damage.dedup();
            models.push(model);
        }
        models
    }
}

/// Evaluate a companion `SR:` expression.
///
/// The RSRD data uses `mastervar("SpecialMountLVL")+5` and plain constants.
/// `mastervar` is resolved to the companion's master level.
fn evaluate_companion_sr(raw: &str, _row_level: u32, master_level: u32) -> Option<i32> {
    evaluate_companion_formula(raw, master_level).map(|value| value.floor() as i32)
}

fn evaluate_companion_formula(raw: &str, master_level: u32) -> Option<f64> {
    let mastervar = regex::Regex::new(r#"mastervar\s*\(\s*["']?[^"')]*["']?\s*\)"#).ok()?;
    let normalized = mastervar.replace_all(raw, master_level.to_string());
    pcgen_formula::evaluate_f64(&normalized, &()).ok()
}

fn companion_row_matches(
    definition: &pcgen_core::CompanionModDefinition,
    companion: &crate::draft::DraftCompanion,
) -> bool {
    definition
        .requirements
        .iter()
        .all(|requirement| companion_requirement_matches(requirement, companion))
}

fn companion_requirement_matches(
    requirement: &str,
    companion: &crate::draft::DraftCompanion,
) -> bool {
    let Some((tag, body)) = requirement.split_once(':') else {
        return false;
    };
    match tag.trim().to_ascii_uppercase().as_str() {
        "PRERACE" => {
            let Some(race_id) = companion.race_id.as_deref() else {
                return false;
            };
            let wanted = normalized_name(race_id);
            body.split(',')
                .skip(1)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .any(|value| {
                    let candidate = normalized_name(value);
                    candidate == wanted
                        || candidate.contains(&wanted)
                        || wanted.contains(&candidate)
                })
        }
        // Only PRERACE appears on RSRD `FOLLOWER` rows.  Unknown predicate
        // forms must not be applied blindly.
        _ => false,
    }
}

fn merge_companion_traits(
    traits: &pcgen_core::CreatureTraits,
    merged: &mut pcgen_core::CreatureTraits,
) {
    merged.vision.extend(traits.vision.iter().cloned());
    merged
        .damage_reduction
        .extend(traits.damage_reduction.iter().cloned());
    merged
        .natural_attacks
        .extend(traits.natural_attacks.iter().cloned());
    merged
        .unarmed_damage
        .extend(traits.unarmed_damage.iter().cloned());
    merged
        .attack_cycle
        .extend(traits.attack_cycle.iter().cloned());
    merged.move_clone.extend(traits.move_clone.iter().cloned());
    if let Some(value) = traits.reach_feet {
        merged.reach_feet = Some(
            merged
                .reach_feet
                .map_or(value, |current| current.max(value)),
        );
    }
    if let Some(value) = traits.reach_mult {
        merged.reach_mult = Some(
            merged
                .reach_mult
                .map_or(value, |current| current.max(value)),
        );
    }
    if let Some(value) = traits.unarmed_mult {
        merged.unarmed_mult = Some(
            merged
                .unarmed_mult
                .map_or(value, |current| current.max(value)),
        );
    }
    if let Some(raw) = &traits.spell_resistance {
        if merged.spell_resistance.is_none() {
            merged.spell_resistance = Some(raw.clone());
        }
    }
}

impl pcgen_core::ClassProgressSource for Engine {
    fn base_attack_bonus(&self, class_id: &str, class_level: u32) -> Option<i32> {
        self.progression(class_id).map(|p| p.bab(class_level))
    }

    fn base_save(&self, class_id: &str, class_level: u32, kind: SaveKind) -> Option<i32> {
        self.progression(class_id)
            .map(|p| p.save(class_level, kind))
    }

    fn skill_points(
        &self,
        class_id: &str,
        intelligence_modifier: i32,
        first_level: bool,
    ) -> Option<u32> {
        self.progression(class_id).map(|p| {
            let base = if first_level {
                p.first_level_skill_points
            } else {
                p.skill_points_per_level
            };
            (base as i32 + intelligence_modifier).max(1) as u32
        })
    }

    fn class_skills(&self, class_id: &str) -> Option<Vec<String>> {
        self.progression(class_id).map(|p| p.class_skills.clone())
    }

    fn gestalt_slot_cost(&self, class_id: &str) -> Option<pcgen_core::PrestigeSlotCost> {
        self.class(class_id)
            .and_then(|class| class.gestalt_slot_cost)
    }

    fn features_up_to_level(&self, class_id: &str, level: u32) -> Option<Vec<String>> {
        self.progression(class_id)
            .map(|progression| progression.features_up_to(level))
    }

    fn features_for_class(&self, lane: &AdvancementLane, class_id: &str) -> Option<Vec<String>> {
        let base = self.features_up_to_level(class_id, lane.class_level)?;
        let variants = self.variants_for(lane, class_id);
        Some(variants.iter().fold(base, |features, variant| {
            variant.apply_to_features(features)
        }))
    }

    fn class_skills_for_lane(&self, lane: &AdvancementLane, class_id: &str) -> Option<Vec<String>> {
        let mut skills = self.class_skills(class_id)?;
        for variant in self.variants_for(lane, class_id) {
            skills.extend(variant.class_skills.iter().cloned());
        }
        Some(skills)
    }
}

pub fn validate(_pack: &CompiledRulePack, _draft: &CharacterDraft) -> Vec<String> {
    Vec::new()
}

fn parse_hit_die(value: &str) -> Option<i32> {
    let raw = value.trim().trim_start_matches(['d', 'D']);
    raw.parse::<i32>().ok().filter(|value| *value > 0)
}

pub(crate) fn effective_class_levels(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> Vec<DraftClassLevel> {
    let mut levels: BTreeMap<String, DraftClassLevel> = BTreeMap::new();
    if !draft.class_levels.is_empty() {
        for entry in &draft.class_levels {
            push_level(
                &mut levels,
                &entry.class_id,
                entry.level,
                entry.hit_points_gained,
                entry.experience_gained,
            );
            if !entry.hit_points_by_level.is_empty() {
                if let Some(existing) = levels.get_mut(&normalized_name(&entry.class_id)) {
                    existing.hit_points_by_level = entry.hit_points_by_level.clone();
                }
            }
        }
    } else {
        for frame in &draft.advancement {
            for lane in &frame.lanes {
                push_level(
                    &mut levels,
                    &lane.class_id,
                    lane.class_level,
                    lane.hit_points_gained,
                    lane.experience_gained,
                );
            }
        }
    }

    // `ADDLEVEL` grants extra class levels without changing the base draft.
    // Template rows are the documented PCGen source; class rows are also
    // preserved and consumed here for consistency.
    let mut additions: Vec<(String, u32)> = Vec::new();
    for template_name in &draft.templates {
        if let Some(template) = find_template(pack, template_name) {
            for (class_id, count) in &template.add_levels {
                additions.push((class_id.clone(), *count));
            }
        }
    }
    let active_classes: Vec<String> = levels
        .values()
        .map(|entry| entry.class_id.clone())
        .collect();
    for class_id in active_classes {
        if let Some(progression) = find_progression(pack, &class_id) {
            for (target, count) in &progression.add_levels {
                additions.push((target.clone(), *count));
            }
        }
    }
    for (target, count) in additions {
        if count == 0 {
            continue;
        }
        let key = normalized_name(&target);
        let entry = levels.entry(key).or_insert_with(|| DraftClassLevel {
            class_id: target.clone(),
            level: 0,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        });
        entry.level += count;
    }

    levels.into_values().collect()
}

fn push_level(
    levels: &mut BTreeMap<String, DraftClassLevel>,
    class_id: &str,
    level: u32,
    hit_points_gained: i32,
    experience_gained: i32,
) {
    let key = normalized_name(class_id);
    let entry = levels.entry(key).or_insert_with(|| DraftClassLevel {
        class_id: class_id.to_string(),
        level: 0,
        hit_points_gained: 0,
        hit_points_by_level: Vec::new(),
        experience_gained: 0,
    });
    entry.level = entry.level.max(level);
    entry.hit_points_gained += hit_points_gained;
    entry.experience_gained += experience_gained;
}

fn find_template<'a>(
    pack: &'a CompiledRulePack,
    name: &str,
) -> Option<&'a pcgen_core::TemplateDefinition> {
    let wanted = normalized_name(name);
    pack.templates.get(name).or_else(|| {
        pack.templates
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name) || normalized_name(key) == wanted)
            .map(|(_, template)| template)
    })
}

fn find_progression<'a>(pack: &'a CompiledRulePack, class_id: &str) -> Option<&'a ClassProgress> {
    let wanted = normalized_name(class_id);
    pack.progressions.get(class_id).or_else(|| {
        pack.progressions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_id) || normalized_name(key) == wanted)
            .map(|(_, progression)| progression)
    })
}

pub(crate) fn stat_modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// Resolve one ability-score modifier, honouring a typed `STATMOD` formula
/// when the compiled pack defines one for `stat`.
pub(crate) fn stat_modifier_for_pack(pack: &CompiledRulePack, stat: &str, score: i32) -> i32 {
    let wanted = stat.trim().to_ascii_uppercase();
    let definition = pack.stats.get(&wanted).or_else(|| {
        pack.stats.iter().find_map(|(key, definition)| {
            (key.eq_ignore_ascii_case(&wanted)
                || definition.id.eq_ignore_ascii_case(&wanted)
                || definition.name.eq_ignore_ascii_case(&wanted))
            .then_some(definition)
        })
    });
    if let Some(formula) = definition.and_then(|definition| definition.modifier_formula.as_deref())
    {
        let mut variables = HashMap::new();
        variables.insert("SCORE".to_string(), score);
        variables.insert("score".to_string(), score);
        if let Ok(value) = pcgen_formula::evaluate_f64(formula, &variables) {
            return value.floor() as i32;
        }
    }
    stat_modifier(score)
}

fn save_ability_modifier(stat_mods: &BTreeMap<String, i32>, kind: SaveKind) -> i32 {
    let stat = match kind {
        SaveKind::Fortitude => "CON",
        SaveKind::Reflex => "DEX",
        SaveKind::Will => "WIS",
    };
    stat_mods.get(stat).copied().unwrap_or(0)
}

fn is_prepared_book(book: &Option<String>) -> bool {
    match book.as_deref() {
        // Manual drafts (no BOOK value) keep the previous prepared meaning.
        None => true,
        Some(value) => value.to_ascii_lowercase().contains("prepared"),
    }
}

fn grapple_size_modifier_from_rank(rank: usize) -> i32 {
    // Fine .. Colossal: smaller creatures are worse at grappling.
    const MODIFIERS: [i32; 9] = [-16, -12, -8, -4, 0, 4, 8, 12, 16];
    MODIFIERS.get(rank).copied().unwrap_or(0)
}

fn size_modifier_from_rank(rank: usize) -> i32 {
    const MODIFIERS: [i32; 9] = [8, 4, 2, 1, 0, -1, -2, -4, -8];
    MODIFIERS.get(rank).copied().unwrap_or(-8)
}

fn equipment_totals(equipment: &[EquipmentDefinition]) -> (i32, i32, Option<i32>, i32) {
    let mut armor = 0;
    let mut shield = 0;
    let mut max_dex: Option<i32> = None;
    let mut armor_check_penalty = 0;
    for item in equipment {
        let tags: BTreeSet<String> = item
            .type_tags
            .iter()
            .map(|tag| tag.to_ascii_lowercase())
            .collect();
        let proficiency = item
            .proficiency
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        if tags.contains("shield") || proficiency.starts_with("shield") {
            shield += item.armor_bonus;
        } else if tags.contains("armor") || proficiency.starts_with("armor") {
            armor += item.armor_bonus;
        }
        if let Some(cap) = item.max_dex_bonus {
            max_dex = Some(max_dex.map_or(cap, |existing: i32| existing.min(cap)));
        }
        armor_check_penalty += item.armor_check_penalty;
    }
    (armor, shield, max_dex, armor_check_penalty)
}

/// Human-readable lane summary used by CLI/debug output.
pub fn lane_summary(lane: &pcgen_core::AdvancementLane) -> String {
    format!("{}:{}", lane.lane_id, lane.class_id)
}

fn merge_creature_traits(
    traits: &pcgen_core::CreatureTraits,
    merged: &mut CreatureTraitAccumulator,
) {
    merged.vision.extend(traits.vision.iter().cloned());
    merged
        .damage_reduction
        .extend(traits.damage_reduction.iter().cloned());
    merged
        .natural_attacks
        .extend(traits.natural_attacks.iter().cloned());
    merged
        .unarmed_damage
        .extend(traits.unarmed_damage.iter().cloned());
    merged
        .attack_cycle
        .extend(traits.attack_cycle.iter().cloned());
    merged.move_clone.extend(traits.move_clone.iter().cloned());
    if let Some(value) = traits.reach_feet {
        merged.reach_feet = Some(
            merged
                .reach_feet
                .map_or(value, |current| current.max(value)),
        );
    }
    if let Some(value) = traits.reach_mult {
        merged.reach_mult = Some(
            merged
                .reach_mult
                .map_or(value, |current| current.max(value)),
        );
    }
    if let Some(value) = traits.unarmed_mult {
        merged.unarmed_mult = Some(
            merged
                .unarmed_mult
                .map_or(value, |current| current.max(value)),
        );
    }
    if let Some(raw) = &traits.spell_resistance {
        let value = raw
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>()
            .parse::<i32>()
            .ok();
        if let Some(value) = value {
            merged.spell_resistance = Some(
                merged
                    .spell_resistance
                    .map_or(value, |current| current.max(value)),
            );
        }
    }
}

fn entity_is_active_for_traits(
    entity: &pcgen_core::CompiledEntity,
    draft: &CharacterDraft,
    effect_state: &crate::effects::EffectState,
    class_levels: &[DraftClassLevel],
) -> bool {
    match &entity.category {
        pcgen_core::EntityCategory::Race => {
            normalized_name(&entity.id.key) == normalized_name(&draft.race_id)
                || normalized_name(&entity.name) == normalized_name(&draft.race_id)
        }
        pcgen_core::EntityCategory::Template => effect_state.templates.iter().any(|template| {
            normalized_name(template) == normalized_name(&entity.id.key)
                || normalized_name(template) == normalized_name(&entity.name)
        }),
        pcgen_core::EntityCategory::ClassLevel => {
            entity
                .id
                .key
                .split_once('#')
                .is_some_and(|(class_id, level)| {
                    crate::effects::class_level_is_active(
                        class_levels,
                        class_id,
                        level.parse::<u32>().unwrap_or(0),
                    )
                })
        }
        pcgen_core::EntityCategory::Ability | pcgen_core::EntityCategory::Feat => {
            crate::effects::ability_matches_draft(draft, entity)
                || effect_state.active_abilities.iter().any(|ability| {
                    ability.eq_ignore_ascii_case(&entity.id.key)
                        || ability.eq_ignore_ascii_case(&entity.name)
                })
        }
        pcgen_core::EntityCategory::Equipment => {
            crate::effects::equipment_matches_draft(draft, entity)
        }
        _ => false,
    }
}

fn resolve_added_domain(raw: &str, templates: &BTreeSet<String>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (domain, condition) = match raw.split_once('[') {
        Some((domain, condition)) => (domain.trim(), condition.trim_end_matches(']').trim()),
        None => (raw, ""),
    };
    if condition.is_empty() {
        return Some(domain.to_string());
    }
    if let Some(body) = condition
        .strip_prefix("PRETEMPLATE:")
        .or_else(|| condition.strip_prefix("pretemplate:"))
    {
        let wanted: Vec<String> = body
            .split(',')
            .skip(1)
            .map(normalized_name)
            .filter(|value| !value.is_empty())
            .collect();
        if wanted.iter().any(|name| {
            templates
                .iter()
                .any(|template| normalized_name(template) == *name)
        }) {
            return Some(domain.to_string());
        }
    }
    None
}

fn normalized_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

/// Base movement modes for the draft: race movement plus `MOVEADD` modifiers
/// applied by the effect pipeline.
fn practiced_movement(
    draft: &CharacterDraft,
    effect_state: &crate::effects::EffectState,
) -> BTreeMap<String, u32> {
    let mut movement: BTreeMap<String, u32> = effect_state.movement.clone();
    for (target, value) in &effect_state.external_modifiers {
        let upper = target.to_ascii_uppercase();
        if let Some(mode) = upper.strip_prefix("MOVEADD.") {
            let mode = mode.to_ascii_lowercase();
            let current = movement.get(&mode).copied().unwrap_or(0) as i32;
            if *value != 0 {
                movement.insert(mode, (current + value).max(0) as u32);
            }
        } else if upper == "MOVEADD" {
            let current = movement.get("walk").copied().unwrap_or(0) as i32;
            movement.insert("walk".to_string(), (current + value).max(0) as u32);
        }
    }
    let _ = draft;
    movement
}

/// Apply PCGen `MOVECLONE:Source,Target,*N` entries.
///
/// Each entry clones the `Source` movement rate into `Target`, multiplied by
/// `N` (default 1).  `*N` scaling is applied to the current source rate; a
/// leading `+`/`-` offset style is not used by the RSRD data.
fn resolve_movement(base: &BTreeMap<String, u32>, move_clone: &[String]) -> BTreeMap<String, u32> {
    let mut movement = base.clone();
    for entry in move_clone {
        for part in entry.split('|') {
            let fields: Vec<&str> = part.split(',').map(str::trim).collect();
            if fields.len() < 2 {
                continue;
            }
            let source = fields[0].to_ascii_lowercase();
            let target = fields[1].to_ascii_lowercase();
            let factor = fields
                .get(2)
                .and_then(|raw| raw.strip_prefix('*'))
                .and_then(|raw| raw.parse::<f64>().ok())
                .unwrap_or(1.0);
            let Some(rate) = movement.get(&source).copied() else {
                continue;
            };
            movement.insert(target, (f64::from(rate) * factor) as u32);
        }
    }
    movement
}

fn save_bonus_components(bonus: &crate::effects::TypedBonus) -> Vec<String> {
    let upper = bonus.target.to_ascii_uppercase();
    if !upper.starts_with("SAVE") {
        return Vec::new();
    }
    let body = bonus
        .target
        .split_once('.')
        .map(|(_, body)| body)
        .unwrap_or("");
    body.split([',', '|'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let name = part.rsplit('.').next().unwrap_or(part).trim();
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => {
                    first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn skill_bonus_components(bonus: &crate::effects::TypedBonus) -> Vec<String> {
    let upper = bonus.target.to_ascii_uppercase();
    if !upper.starts_with("SKILL") {
        return Vec::new();
    }
    let body = bonus
        .target
        .split_once('.')
        .map(|(_, body)| body)
        .unwrap_or("");
    body.split([',', '|'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn attack_bonus_targets(bonus: &crate::effects::TypedBonus) -> Vec<String> {
    let upper = bonus.target.to_ascii_uppercase();
    if upper.contains("TOHIT")
        || upper.starts_with("WEAPON")
        || upper.starts_with("ATTACK")
        || upper.starts_with("COMBAT.ATTACK")
    {
        vec![upper]
    } else {
        Vec::new()
    }
}

fn bonus_stack_group(stack_key: Option<&str>, bonus_type: &str) -> (bool, String) {
    if stack_key.is_some_and(|key| key.eq_ignore_ascii_case("stack")) {
        return (true, format!("stack:{bonus_type}"));
    }
    if stack_key.is_some_and(|key| key.eq_ignore_ascii_case("nonstack")) {
        return (false, format!("nonstack:{bonus_type}"));
    }
    if is_stacking_bonus_type(bonus_type) {
        (true, format!("stack:{bonus_type}"))
    } else {
        (false, format!("nonstack:{bonus_type}"))
    }
}

fn stacked_components<F>(
    bonuses: &[crate::effects::TypedBonus],
    mut components: F,
) -> BTreeMap<String, i32>
where
    F: FnMut(&crate::effects::TypedBonus) -> Vec<String>,
{
    let mut groups: BTreeMap<(String, String), (i32, bool)> = BTreeMap::new();
    for bonus in bonuses {
        let bonus_type = bonus
            .bonus_type
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        let (stack, group_key) = bonus_stack_group(bonus.stack_key.as_deref(), &bonus_type);
        for component in components(bonus) {
            let entry = groups
                .entry((component, group_key.clone()))
                .or_insert((0, stack));
            if stack {
                entry.0 += bonus.value;
            } else {
                entry.0 = choose_non_stacking(entry.0, bonus.value);
            }
        }
    }
    let mut totals: BTreeMap<String, i32> = BTreeMap::new();
    for ((component, _), (value, _)) in groups {
        *totals.entry(component).or_insert(0) += value;
    }
    totals
}

fn ac_bonus_components(bonuses: &[crate::effects::TypedBonus]) -> (i32, i32, i32) {
    let mut groups: BTreeMap<String, (String, i32)> = BTreeMap::new();
    for bonus in bonuses {
        let upper = bonus.target.to_ascii_uppercase();
        if upper != "AC" && upper != "COMBAT.AC" {
            continue;
        }
        let bonus_type = bonus
            .bonus_type
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        let (stack, group_key) = bonus_stack_group(bonus.stack_key.as_deref(), &bonus_type);
        let entry = groups
            .entry(group_key)
            .or_insert_with(|| (bonus_type.clone(), 0));
        if stack {
            entry.1 += bonus.value;
        } else {
            entry.1 = choose_non_stacking(entry.1, bonus.value);
        }
    }

    let mut armor_class = 0;
    let mut touch_ac = 0;
    let mut flat_footed_ac = 0;
    for (bonus_type, value) in groups.into_values() {
        if bonus_type.contains("armor")
            || bonus_type.contains("shield")
            || bonus_type.contains("natural")
        {
            armor_class += value;
            flat_footed_ac += value;
        } else if bonus_type.contains("dodge") {
            armor_class += value;
            touch_ac += value;
        } else {
            armor_class += value;
            touch_ac += value;
            flat_footed_ac += value;
        }
    }
    (armor_class, touch_ac, flat_footed_ac)
}

fn is_stacking_bonus_type(bonus_type: &str) -> bool {
    bonus_type.is_empty()
        || bonus_type == "untyped"
        || bonus_type == "type"
        || bonus_type.contains("stack")
        || bonus_type.contains("dodge")
        || bonus_type.contains("circumstance")
}

fn choose_non_stacking(old: i32, new: i32) -> i32 {
    if old >= 0 && new >= 0 {
        old.max(new)
    } else if old <= 0 && new <= 0 {
        old.min(new)
    } else if new.abs() > old.abs() {
        new
    } else {
        old
    }
}

fn apply_external_modifiers(snapshot: &mut CharacterSnapshot, modifiers: &BTreeMap<String, i32>) {
    for (target, value) in modifiers {
        if *value == 0 {
            continue;
        }
        let upper = target.to_ascii_uppercase();
        if upper == "HP" || upper.starts_with("HP.") {
            // PCGen class-level `BONUS:HP|CURRENTMAX|N` is a hit-point
            // modifier; normalize it to the snapshot HP field.
            snapshot.hit_points += value;
        } else if upper == "BAB" || upper.contains("BASEAB") {
            // Already included by `bab::compute_bab` before snapshot creation.
        } else if upper.contains("SAVE") {
            for component in target.split([',', '|']) {
                let component = component.trim().to_ascii_lowercase();
                if component.contains("fort") {
                    *snapshot.saves.entry("Fortitude".to_string()).or_insert(0) += value;
                } else if component.contains("ref") {
                    *snapshot.saves.entry("Reflex".to_string()).or_insert(0) += value;
                } else if component.contains("will") {
                    *snapshot.saves.entry("Will".to_string()).or_insert(0) += value;
                }
            }
        } else if upper == "AC" || upper == "COMBAT.AC" {
            snapshot.armor_class += value;
            snapshot.touch_ac += value;
            snapshot.flat_footed_ac += value;
        } else if upper.contains("INIT") {
            snapshot.initiative += value;
        } else if upper.starts_with("SKILL") {
            for skill in target.split(['.', ',']) {
                let skill = skill.trim();
                if skill.is_empty() || skill.eq_ignore_ascii_case("SKILL") {
                    continue;
                }
                if let Some(entry) = snapshot
                    .skills
                    .iter_mut()
                    .find(|(name, _)| name.eq_ignore_ascii_case(skill))
                {
                    *entry.1 += value;
                }
            }
        } else if upper.starts_with("SPELLDC") {
            for dc in snapshot.spell_dcs.values_mut() {
                *dc += value;
            }
        }
    }
}

fn collect_divine_tracks(draft: &CharacterDraft) -> Vec<DivineTrack> {
    let mut tracks = Vec::new();
    for frame in &draft.advancement {
        for lane in &frame.lanes {
            if lane.is_divine() {
                tracks.push(DivineTrack {
                    lane_id: LaneId::new(lane.lane_id.to_string()),
                    class_id: lane.class_id.clone(),
                    class_level: lane.class_level,
                    deity: lane.deity.clone(),
                    domains: lane.domains.clone(),
                });
            }
        }
    }
    if tracks.is_empty() && (draft.deity.is_some() || !draft.domains.is_empty()) {
        let class_id = draft
            .class_levels
            .first()
            .map(|entry| entry.class_id.clone())
            .unwrap_or_else(|| "Divine".to_string());
        tracks.push(DivineTrack {
            lane_id: LaneId::new("divine"),
            class_id,
            class_level: draft.total_level(),
            deity: draft.deity.clone(),
            domains: draft.domains.clone(),
        });
    }
    tracks
}

/// Compiled once per process: this function runs per class level per derived
/// value (BAB, saves, progressions), so compiling a regex per call dominated
/// the cost of short formulas.
fn class_formula_regexes() -> &'static (regex::Regex, regex::Regex, regex::Regex, regex::Regex) {
    static REGEXES: std::sync::OnceLock<(regex::Regex, regex::Regex, regex::Regex, regex::Regex)> =
        std::sync::OnceLock::new();
    REGEXES.get_or_init(|| {
        (
            regex::Regex::new(r#"classlevel\s*\(\s*["']?[^"')]*["']?\s*\)"#)
                .expect("classlevel pattern is valid"),
            regex::Regex::new(r"\bTL\b").expect("TL pattern is valid"),
            regex::Regex::new(r"\bCL\b").expect("CL pattern is valid"),
            regex::Regex::new(r#"var\s*\(\s*["']?([^"')]+)["']?\s*\)"#)
                .expect("var() pattern is valid"),
        )
    })
}

pub(crate) fn evaluate_class_formula(
    formula: &str,
    class_level: u32,
    total_level: u32,
    variables: &BTreeMap<String, i32>,
) -> i32 {
    let (classlevel, total, class, var_call) = class_formula_regexes();
    let mut normalized = formula.to_string();
    normalized = classlevel
        .replace_all(&normalized, class_level.to_string())
        .to_string();
    normalized = total
        .replace_all(&normalized, total_level.to_string())
        .to_string();
    normalized = class
        .replace_all(&normalized, class_level.to_string())
        .to_string();
    normalized = var_call
        .replace_all(&normalized, |captures: &regex::Captures| {
            let name = captures[1].trim();
            if let Some(_class_id) = name.strip_prefix("BL=") {
                return class_level.to_string();
            }
            variables.get(name).copied().unwrap_or(0).to_string()
        })
        .to_string();
    pcgen_formula::evaluate_f64(
        &normalized,
        &std::collections::HashMap::<String, i64>::new(),
    )
    .map(|value| value as i32)
    .unwrap_or(0)
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
    fn cached_regexes_keep_identical_results() {
        // Equivalence guard for the `OnceLock` cache: repeated evaluation must
        // return exactly what the first call did, and the patterns must still
        // match after being compiled once per process.
        let variables = variables();
        let cases = [
            ("classlevel()", 5, 9),
            ("classlevel(\"APPLIEDAS=NONEPIC\")/2+2", 5, 9),
            ("TL*2", 5, 9),
            ("CL+1", 5, 9),
            ("var(\"Foo\")+3", 5, 9),
            ("classlevel()/3", 5, 9),
            ("TL + CL", 5, 9),
        ];
        for (formula, class_level, total_level) in cases {
            let first = evaluate_class_formula(formula, class_level, total_level, &variables);
            for _ in 0..50 {
                assert_eq!(
                    evaluate_class_formula(formula, class_level, total_level, &variables),
                    first,
                    "formula `{formula}` changed across calls"
                );
            }
        }
    }

    #[test]
    fn class_formula_rewrites_match_expected_values() {
        let variables = variables();
        assert_eq!(evaluate_class_formula("classlevel()", 5, 9, &variables), 5);
        assert_eq!(
            evaluate_class_formula("classlevel()/2+2", 5, 9, &variables),
            4
        );
        assert_eq!(evaluate_class_formula("TL*2", 5, 9, &variables), 18);
        assert_eq!(evaluate_class_formula("CL+1", 5, 9, &variables), 6);
        assert_eq!(
            evaluate_class_formula("var(\"Foo\")+3", 5, 9, &variables),
            10
        );
        // An unknown variable resolves to 0 rather than failing.
        assert_eq!(
            evaluate_class_formula("var(\"Missing\")", 5, 9, &variables),
            0
        );
    }
}
