//! Deterministic runtime engine.

use crate::bonuses::{stacked_components, BonusGroup, BonusPolicy};

use crate::draft::{CharacterDraft, ClassLevelSummary};
use crate::effects::apply_effects;
use crate::progression::{find_progression, ClassProgress, ProgressSource};
use crate::snapshot::CharacterSnapshot;
use pcgen_core::domain::{ClassDefinition, EquipmentDefinition, RaceDefinition};
use pcgen_core::CompiledRulePack;
use pcgen_core::{
    evaluate_faith, plan_gestalt, AdvancementLane, DivineTrack, FaithPolicy, GestaltPolicy, LaneId,
    SaveKind, VariantDefinition,
};
use std::collections::{BTreeMap, BTreeSet};

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
    natural_attacks: Vec<pcgen_core::NaturalAttack>,
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

    /// Ability-score modifiers for all core stats, honouring typed `STATMOD`
    /// formulas where present.
    fn ability_modifiers(
        &self,
        stats: &BTreeMap<String, i32>,
        diagnostics: &mut Vec<String>,
    ) -> BTreeMap<String, i32> {
        let mut modifiers = BTreeMap::new();
        for stat in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
            let score = stats.get(stat).copied().unwrap_or(10);
            modifiers.insert(
                stat.to_string(),
                crate::derived::report(
                    stat_modifier_for_pack(&self.pack, stat, score),
                    diagnostics,
                )
                .unwrap_or_default(),
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
        if draft.build_history.is_some() {
            return match draft.materialized() {
                Ok(d) => self.evaluate(&d),
                Err(error) => CharacterSnapshot {
                    diagnostics: vec![error],
                    ..Default::default()
                },
            };
        }
        let mut diagnostics = Vec::new();
        diagnostics.extend(crate::class_features::validate_class_choices(
            &self.pack, draft,
        ));
        let effect_state = apply_effects(&self.pack, draft);
        diagnostics.extend(effect_state.diagnostics.clone());
        if let Err(error) = crate::class_features::validate_replacements(&self.pack, draft) {
            diagnostics.push(error);
        }
        if let Err(error) = crate::class_features::validate_sudden_feats(&self.pack, draft) {
            diagnostics.push(error);
        }
        for class in effective_class_levels(&self.pack, draft) {
            if let Err(error) =
                crate::class_features::learned_spells(&self.pack, draft, &class.class_id)
            {
                diagnostics.push(error);
            }
        }
        let class_levels = effective_class_levels(&self.pack, draft);
        let mut stats = crate::feature_resolution::encounter_base_stats(draft);
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
        for (key, n) in &mut stats {
            *n = crate::combat::modified_score(&draft.class_play, "self", key, *n);
        }
        let size_rank = self.size_rank(&draft.race_id);
        let size_modifier = size_modifier_from_rank(size_rank);
        let total_level = if draft.is_gestalt() {
            draft.total_level()
        } else {
            class_levels.iter().map(|entry| entry.level).sum()
        };
        let mut stat_mods = self.ability_modifiers(&stats, &mut diagnostics);
        if draft.class_play.necromancy.lich {
            stat_mods.insert("CON".into(), 0);
        }
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
            let mut frames = draft.advancement.clone();
            for lane in frames.iter_mut().flat_map(|frame| &mut frame.lanes) {
                if lane.hit_points_gained.is_none() {
                    lane.hit_points_gained = Some(self.hit_points_for_entry(
                        draft,
                        &ClassLevelSummary {
                            class_id: lane.class_id.clone(),
                            level: 1,
                            hit_points_by_level: vec![None],
                            experience_gained: 0,
                        },
                        0,
                    ));
                }
            }
            let source = ProgressSource {
                pack: &self.pack,
                race_id: &draft.race_id,
                variables: &effect_state.variables,
                total_level,
                diagnostics: Default::default(),
            };
            let plan = plan_gestalt(
                &frames,
                &source,
                stat_mods.get("INT").copied().unwrap_or(0),
                &GestaltPolicy::default(),
            );
            diagnostics.extend(source.diagnostics.into_inner());
            Some(plan)
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
                for domain in self.class_added_domains(&entry.class_id, draft, &effect_state) {
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
        if let Some(rage) = &draft.class_play.encounter.rage {
            all_typed_bonuses.push(crate::effects::TypedBonus {
                target: "SAVE.Will".into(),
                value: rage.will_bonus,
                bonus_type: Some("Morale".into()),
                stack_key: None,
            });
        }
        if let Some(c) = draft
            .class_play
            .tactics
            .fighting
            .as_ref()
            .filter(|c| c.expires > draft.class_play.encounter.seconds)
        {
            all_typed_bonuses.push(crate::effects::TypedBonus {
                target: "SAVE.Will".into(),
                value: c.bonus,
                bonus_type: Some("Morale".into()),
                stack_key: None,
            });
        }
        let (mut effect_ac, mut effect_touch, mut effect_flat) =
            ac_bonus_components(&all_typed_bonuses);
        if draft.class_play.encounter.rage.is_some() {
            effect_ac -= 2;
            effect_touch -= 2;
            effect_flat -= 2;
        }
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

        let negative_levels = crate::necromancy::negative_levels(&draft.class_play, "self");
        let hit_points = hit_points - negative_levels * 5;
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
            if let Some(base) = crate::derived::report(
                crate::bab::derive_base_saves(&self.pack, draft, &effect_state.variables),
                &mut diagnostics,
            ) {
                fort += base.get(&SaveKind::Fortitude).copied().unwrap_or_default();
                reflex += base.get(&SaveKind::Reflex).copied().unwrap_or_default();
                will += base.get(&SaveKind::Will).copied().unwrap_or_default();
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

        let base_attack_bonus = crate::derived::report(
            crate::bab::compute_bab(
                &self.pack,
                draft,
                &effect_state.variables,
                &effect_state.external_modifiers,
            ),
            &mut diagnostics,
        )
        .unwrap_or_default();

        // Recompute ability modifiers for skill math using post-spell stats:
        // `stat_mods` was computed before spell STAT bonuses were folded.
        let skill_stat_mods = self.ability_modifiers(&stats, &mut diagnostics);
        let mut skills = self.skills(
            draft,
            &skill_stat_mods,
            armor_check_penalty,
            &mut diagnostics,
        );
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
        let condition_penalty =
            if crate::combat::condition(&draft.class_play, "self", "sickened") {
                2
            } else {
                0
            } + if crate::combat::condition(&draft.class_play, "self", "panicked") {
                2
            } else {
                0
            };
        for n in saves.values_mut() {
            *n -= negative_levels + condition_penalty;
        }
        for n in skills.values_mut() {
            *n -= negative_levels + condition_penalty;
        }
        let mut attack_bonuses = stacked_components(&all_typed_bonuses, attack_bonus_targets);
        if let Some(c) = draft
            .class_play
            .tactics
            .fighting
            .as_ref()
            .filter(|c| c.expires > draft.class_play.encounter.seconds)
        {
            attack_bonuses.insert(format!("TARGET.{}.TOHIT.MORALE", c.target), c.bonus);
            attack_bonuses.insert(format!("TARGET.{}.DAMAGE.MORALE", c.target), c.bonus);
        }
        if negative_levels > 0 {
            *attack_bonuses.entry("ALL".into()).or_default() -= negative_levels;
        }
        let (mut caster_levels, spell_dcs, mut spell_slots, known_limits, prepared_spells) =
            self.spellcasting(draft, &class_levels, &stat_mods, &mut diagnostics);
        for loss in draft
            .class_play
            .necromancy
            .negative_levels
            .iter()
            .filter(|n| n.target == "self")
        {
            if let Some(key) = &loss.lost_slot {
                if let Some(cap) = spell_slots.get_mut(key) {
                    *cap = cap.saturating_sub(1);
                }
            }
        }
        for n in caster_levels.values_mut() {
            *n = (*n - negative_levels).max(0);
        }
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
        let feat_slots = self.feat_slots(total_level, &class_levels);
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
        let ability_pools = crate::pools::ability_pools(
            &self.pack,
            &effect_state,
            draft,
            feat_slots,
            &mut diagnostics,
        );
        let ability_pool_usage =
            crate::pools::ability_pool_usage(&self.pack, draft, &ability_pools);
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
        } = self.creature_traits(draft, &effect_state, &mut diagnostics);
        let movement = resolve_movement(&practiced_movement(draft, &effect_state), &move_clone);
        let mut master_variables = effect_state.variables.clone();
        if !effect_state.unresolved_derived.contains("BAB") {
            master_variables.insert("BAB".into(), base_attack_bonus);
        }
        let companions = self.companions(draft, &master_variables, &mut diagnostics);
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
        let turning = self.turning(draft, &effect_state.variables, &stat_mods);
        let uncanny_dodge =
            self.uncanny_dodge(&effect_state.active_abilities, &effect_state.variables);
        let mut prohibited_spells = self.prohibited_spells(draft, &effect_state);
        match crate::magic::native_prohibited_schools(&self.pack, draft) {
            Ok(schools) => {
                for school in schools {
                    if prohibited_spells.iter().any(|p|matches!(&p.target,pcgen_core::ProhibitionTarget::School(s) if s.eq_ignore_ascii_case(&school))) { diagnostics.push(format!("Focused Studies must add a new prohibited school: {school}")); }
                else { prohibited_spells.push(pcgen_core::ProhibitedSpell{target:pcgen_core::ProhibitionTarget::School(school),condition:None}); }
                }
            }
            Err(error) => diagnostics.push(error),
        }
        let mut spell_access = BTreeMap::new();
        for track in caster_levels
            .keys()
            .filter(|id| crate::magic::class_magic(&self.pack, id).is_some())
        {
            match self.spell_catalog(draft, track) {
                Ok(catalog) => {
                    spell_access.insert(track.clone(), catalog);
                }
                Err(error) => diagnostics.push(format!("{track} spell access: {error}")),
            }
        }
        let rule_gaps = self
            .pack
            .entities
            .iter()
            .filter_map(|entity| {
                let pending = entity.facts.get("NativeRulePending")?;
                let selected = match entity.category {
                    pcgen_core::EntityCategory::Class => class_levels
                        .iter()
                        .any(|level| level.class_id.eq_ignore_ascii_case(&entity.name)),
                    pcgen_core::EntityCategory::Feat | pcgen_core::EntityCategory::Ability => {
                        draft.abilities.iter().any(|(_, key)| {
                            key.eq_ignore_ascii_case(&entity.name)
                                || key.eq_ignore_ascii_case(&entity.id.key)
                        })
                    }
                    _ => false,
                };
                selected.then(|| (entity.id.as_string(), pending.clone()))
            })
            .collect();
        let mut snapshot = CharacterSnapshot {
            creature_type: effect_state.race_creature_type.clone(),
            body_immunities: if effect_state.race_creature_type.as_deref() == Some("undead") {
                pcgen_core::class_play::TargetBody::Undead
                    .immunities()
                    .to_vec()
            } else {
                vec![]
            },
            rule_gaps,
            spell_access,
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
            prohibited_spells: prohibited_spells.clone(),
            power_points,
            manifester_levels,
            powers_known,
            vision,
            damage_reduction,
            spell_resistance,
            // Rendered here: the sheet shows an attack routine, not PCGen's
            // `Name,Weapon.Natural...,*1,1d6` encoding.
            natural_attacks: natural_attacks
                .iter()
                .map(pcgen_core::NaturalAttack::render)
                .collect(),
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
                if draft.class_play.encounter.rage.is_some() {
                    states.push("Raging".into());
                }
                if draft.class_play.encounter.fatigued {
                    states.push("Fatigued".into());
                }
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
            turning,
            uncanny_dodge,
        };
        let external_modifiers = snapshot.external_modifiers.clone();
        apply_external_modifiers(&mut snapshot, &external_modifiers);
        snapshot
    }

    fn creature_traits(
        &self,
        draft: &CharacterDraft,
        effect_state: &crate::effects::EffectState,
        diagnostics: &mut Vec<String>,
    ) -> CreatureTraitAccumulator {
        let mut merged = CreatureTraitAccumulator::default();
        let class_levels = effective_class_levels(&self.pack, draft);

        for entity in &self.pack.entities {
            if !entity_is_active_for_traits(entity, draft, effect_state, &class_levels) {
                continue;
            }
            merge_creature_traits(
                &entity.traits,
                &mut merged,
                draft,
                effect_state,
                &entity.id.key,
                diagnostics,
            );
        }
        for traits in self.equipmod_traits(draft) {
            merge_creature_traits(
                &traits,
                &mut merged,
                draft,
                effect_state,
                "equipment modifier",
                diagnostics,
            );
        }

        if draft.class_play.necromancy.lich {
            merged.vision.push("Darkvision (60 feet)".into());
        }
        merged.vision.sort();
        merged.vision.dedup();
        // Same-type reductions do not stack: keep the highest per bypass type.
        merged.damage_reduction = pcgen_core::collapse_damage_reduction(&merged.damage_reduction);
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
            let state = apply_effects(&self.pack, draft);
            diagnostics.extend(state.diagnostics.clone());
            let source = ProgressSource {
                pack: &self.pack,
                race_id: &draft.race_id,
                variables: &state.variables,
                total_level: draft.total_level(),
                diagnostics: Default::default(),
            };
            let plan = plan_gestalt(
                &draft.advancement,
                &source,
                self.ability_modifiers(&draft.stats, &mut diagnostics)
                    .get("INT")
                    .copied()
                    .unwrap_or(0),
                &GestaltPolicy::default(),
            );
            diagnostics.extend(source.diagnostics.into_inner());
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

    /// Schools and alignment descriptors this character may not cast.
    ///
    /// The prohibitions sit on the class and carry their own gate, e.g.
    /// `SCHOOL.Abjuration|PREVARGTEQ:ProhibitAbjuration,1`, where the variable
    /// is set by a `TYPE:WizardProhibitSchool` ability.
    fn prohibited_spells(
        &self,
        draft: &CharacterDraft,
        state: &crate::effects::EffectState,
    ) -> Vec<pcgen_core::ProhibitedSpell> {
        let mut prohibitions = Vec::new();
        for entry in effective_class_levels(&self.pack, draft) {
            let Some(class) = self.class(&entry.class_id) else {
                continue;
            };
            for prohibition in &class.prohibited_spells {
                if !crate::effects::predicate_holds(prohibition.condition.as_ref(), draft, state) {
                    continue;
                }
                if !prohibitions.contains(prohibition) {
                    prohibitions.push(prohibition.clone());
                }
            }
        }
        prohibitions
    }

    fn class_added_domains(
        &self,
        class_id: &str,
        draft: &CharacterDraft,
        state: &crate::effects::EffectState,
    ) -> Vec<String> {
        let entries = self
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
        // The condition is a `Predicate` now, evaluated by the same code that
        // evaluates every other condition.  This used to parse PCGen's
        // `[PRETEMPLATE:...]` syntax here.
        entries
            .into_iter()
            .filter(|entry| crate::effects::predicate_holds(entry.condition.as_ref(), draft, state))
            .map(|entry| entry.domain)
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

    fn variants_for(&self, lane: &AdvancementLane, class_id: &str) -> Vec<&VariantDefinition> {
        crate::progression::variants_for(&self.pack, lane, class_id)
    }

    fn race(&self, race_id: &str) -> Option<&RaceDefinition> {
        crate::progression::find_race(&self.pack, race_id)
    }

    pub fn progression(&self, class_id: &str) -> Option<&ClassProgress> {
        crate::progression::find_progression(&self.pack, class_id)
    }

    pub(crate) fn class(&self, class_id: &str) -> Option<&ClassDefinition> {
        crate::progression::find_class(&self.pack, class_id)
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
        diagnostics: &mut Vec<String>,
    ) -> BTreeMap<String, i32> {
        let mut skills = BTreeMap::new();
        for (name, ranks) in &draft.skills {
            if !ranks.is_finite() || *ranks < 0.0 || *ranks > i32::MAX as f64 - 10000.0 {
                diagnostics.push(format!("Invalid skill ranks: {name}"));
                continue;
            }
            let definition = self.pack.skills.get(name).or_else(|| {
                self.pack
                    .skills
                    .iter()
                    .find(|(key, skill)| {
                        key.eq_ignore_ascii_case(name) || skill.name.eq_ignore_ascii_case(name)
                    })
                    .map(|(_, skill)| skill)
            });
            let (ability_mod, armor_penalty) = if let Some(definition) = definition {
                let mod_val = definition
                    .key_stat
                    .as_deref()
                    .and_then(|key| stat_mods.get(&key.to_ascii_uppercase()).copied())
                    .unwrap_or_else(|| {
                        diagnostics.push(format!("Skill key ability unavailable: {name}"));
                        0
                    });
                let mod_val = if draft.class_play.necromancy.lich
                    && name.eq_ignore_ascii_case("Concentration")
                {
                    stat_mods.get("CHA").copied().unwrap_or(0)
                } else {
                    mod_val
                };
                let penalty = if definition.armor_check {
                    armor_check_penalty
                } else {
                    0
                };
                (mod_val, penalty)
            } else {
                diagnostics.push(format!("Skill definition unavailable: {name}"));
                (0, 0)
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
    pub(crate) fn untrained_skill_base(
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
            .and_then(|key_stat| stat_mods.get(&key_stat.to_ascii_uppercase()).copied())?;
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

    pub(crate) fn skill(&self, name: &str) -> Option<&pcgen_core::SkillDefinition> {
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

    /// True when `class_id` grants divine domain spell slots.
    ///
    /// SRD, Cleric: a cleric "gains one domain spell slot for each spell level
    /// she can cast", which is the `+1` in the class table's Spells per Day
    /// column.  The base `CAST:` progression does not include it, so it is
    /// added on top.  Druids and similar divine casters have no domain slots.
    fn has_domain_slots(&self, draft: &CharacterDraft, class_id: &str) -> bool {
        let has_domains = collect_divine_tracks(draft).iter().any(|track| {
            track.class_id.eq_ignore_ascii_case(class_id) && !track.domains.is_empty()
        });
        if has_domains {
            return true;
        }
        // Domains can also come from the draft directly (deity_domains), which
        // is how the Web wizard writes them.
        !draft.deity_domains.is_empty()
            && self
                .class(class_id)
                .is_some_and(|class| class.spellcasting_ability.is_some())
            && draft
                .class_levels()
                .iter()
                .any(|entry| entry.class_id.eq_ignore_ascii_case(class_id))
    }

    fn spellcasting(
        &self,
        draft: &CharacterDraft,
        class_levels: &[ClassLevelSummary],
        stat_mods: &BTreeMap<String, i32>,
        diagnostics: &mut Vec<String>,
    ) -> SpellcastingMaps {
        let mut caster_levels = BTreeMap::new();
        let mut spell_slots = BTreeMap::new();
        let mut known_limits = BTreeMap::new();

        let tracks = crate::magic::casting_levels(&self.pack, draft, diagnostics);

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
            let Some(&effective_level) = tracks.get(&class.id) else {
                continue;
            };
            let cl = if let Some(formula) = crate::magic::class_magic(&self.pack, &class.id)
                .and_then(|m| m.caster_level_formula.as_ref())
            {
                let vars = std::collections::HashMap::from([(
                    "CASTINGLEVEL".into(),
                    effective_level as i32,
                )]);
                match pcgen_formula::Formula::parse(formula).and_then(|f| f.eval_i32(&vars)) {
                    Ok(n) if n >= 0 => n,
                    Ok(_) => {
                        diagnostics.push(format!("{}: negative caster level", class.id));
                        continue;
                    }
                    Err(e) => {
                        diagnostics.push(format!("{} caster level: {e}", class.id));
                        continue;
                    }
                }
            } else {
                effective_level as i32
            };
            caster_levels.insert(class.id.clone(), cl);
            if let Some(slots) = class.spell_slots.get(&effective_level) {
                for (level, count) in slots.iter().enumerate() {
                    spell_slots.insert(format!("{}|{}", class.id, level), *count);
                }
            }
            // SRD, Cleric: "A cleric [...] gains one domain spell slot for each
            // spell level she can cast" — the `+1` in the Cleric table's
            // "Spells per Day" column.  It is a slot the cleric must fill from
            // one of her domains, and it is absent from the base `CAST:` table,
            // so it has to be added here.
            //
            // https://www.d20srd.org/srd/classes/cleric.htm
            if self.has_domain_slots(draft, &class.id) {
                if let Some(slots) = class.spell_slots.get(&effective_level) {
                    for (level, count) in slots.iter().enumerate() {
                        if level == 0 || *count == 0 {
                            continue;
                        }
                        let key = format!("{}|{}", class.id, level);
                        let entry = spell_slots.entry(key).or_insert(*count);
                        *entry = (*entry).max(*count + 1);
                    }
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
        for spell in draft.spells.iter().chain(&draft.preparations) {
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
            let stat = crate::magic::policy_dc_ability(&self.pack, &class_id)
                .cloned()
                .or_else(|| class.and_then(|class| class.spellcasting_ability.clone()))
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
            let copies = draft.prepared_copies(&class_id, &spell.name).unwrap_or(0);
            if copies > 0 {
                prepared_spells.insert(format!("{}|{}", class_id, spell.name), copies);
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
        class_levels: &[ClassLevelSummary],
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
                        (modifier.target.clone(), modifier.value)
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
                entry.value += value.floor() as i32;
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
        entry: &ClassLevelSummary,
        con_mod: i32,
    ) -> i32 {
        let class_die = self
            .class(&entry.class_id)
            .and_then(|class| parse_hit_die(&class.hit_die));
        let die = match self.hit_die_override(draft, &entry.class_id) {
            Some(rule) => rule.apply(class_die),
            None => self
                .class(&entry.class_id)
                .filter(|class| class.is_monster)
                .and_then(|class| parse_hit_die(&class.hit_die)),
        };
        (0..entry.level as usize)
            .map(|level| {
                let roll = entry.hit_points_by_level.get(level).copied().flatten();
                let base = roll.unwrap_or_else(|| die.map(|value| value / 2 + 1).unwrap_or(0));
                if base == 0 {
                    con_mod.max(0)
                } else {
                    (base + con_mod).max(1)
                }
            })
            .sum()
    }

    fn hit_die_override(&self, draft: &CharacterDraft, class_id: &str) -> Option<HitDieRule> {
        if let Some(class) = self.class(class_id) {
            if let Some(rule) = class
                .hit_die_override
                .as_deref()
                .and_then(parse_hit_die_rule)
            {
                return Some(rule);
            }
        }
        for template_name in &draft.templates {
            if let Some(template) = find_template(&self.pack, template_name) {
                if let Some(rule) = template
                    .hit_die_override
                    .as_deref()
                    .and_then(parse_hit_die_rule)
                {
                    return Some(rule);
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
        class_levels: &[ClassLevelSummary],
        stat_mods: &BTreeMap<String, i32>,
    ) -> i32 {
        let int_mod = stat_mods.get("INT").copied().unwrap_or(0);
        let first_class = draft
            .advancement
            .first()
            .and_then(|frame| frame.lanes.first())
            .map(|lane| normalized_name(&lane.class_id));
        let mut total = 0;
        for entry in class_levels {
            let normal = self
                .skill_points_for_level(&draft.race_id, &entry.class_id, int_mod, false)
                .unwrap_or(0) as i32;
            total += normal * entry.level as i32;
            if entry.level > 0
                && first_class.as_deref() == Some(normalized_name(&entry.class_id).as_str())
            {
                total += self
                    .skill_points_for_level(&draft.race_id, &entry.class_id, int_mod, true)
                    .unwrap_or(0) as i32
                    - normal;
            }
        }
        total
    }

    /// RSRD: apply the minimum before racial points, then multiply the whole
    /// award at character level one (never again on entering another class).
    fn skill_points_for_level(
        &self,
        race_id: &str,
        class_id: &str,
        intelligence_modifier: i32,
        first_level: bool,
    ) -> Option<u32> {
        let progression = self.progression(class_id)?;
        Some(crate::progression::skill_points_for_level(
            progression,
            self.race(race_id),
            intelligence_modifier,
            first_level,
        ))
    }

    /// Uncanny dodge and improved uncanny dodge.
    ///
    /// The classes grant the abilities themselves, and the data sums the
    /// granting class levels into `UncannyDodgeFlankingLevel` (the RSRD gives
    /// that variable `TYPE=EachClass.REPLACE` from both the barbarian and the
    /// rogue records, so the values accumulate).  What the data does not state
    /// is the SRD's conversion rule:
    ///
    /// > When a barbarian/rogue would gain uncanny dodge a second time (for her
    /// > second class), she instead gains improved uncanny dodge, if she does
    /// > not already have it.
    fn uncanny_dodge(
        &self,
        active_abilities: &BTreeSet<String>,
        variables: &BTreeMap<String, i32>,
    ) -> Option<crate::snapshot::UncannyDodgeSnapshot> {
        let active: Vec<String> = active_abilities.iter().cloned().collect();
        let (uncanny_classes, improved_classes) = crate::snapshot::uncanny_dodge_classes(&active);
        if uncanny_classes.is_empty() && improved_classes.is_empty() {
            return None;
        }
        let uncanny_dodge = !uncanny_classes.is_empty();
        // A second granting class turns its own uncanny dodge into improved
        // uncanny dodge, unless improved uncanny dodge is already held.
        let improved_from_second_class = improved_classes.is_empty() && uncanny_classes.len() >= 2;
        let improved = !improved_classes.is_empty() || improved_from_second_class;
        let raw_flanking = variables
            .get("UncannyDodgeFlankingLevel")
            .copied()
            .unwrap_or(0)
            .max(0) as u32;
        let flanking_level = if improved {
            raw_flanking.saturating_sub(4)
        } else {
            raw_flanking
        };
        Some(crate::snapshot::UncannyDodgeSnapshot {
            uncanny_dodge,
            improved,
            flanking_level,
            improved_from_second_class,
            uncanny_dodge_classes: uncanny_classes,
            improved_classes,
        })
    }

    /// Turn or rebuke undead state.
    ///
    /// The SRD's paladin and multiclass rules are already encoded in the data:
    /// the Cleric record sets `TurnLevelBase` to `ClericLVL`, and the Paladin
    /// record (granted from 4th level) sets it to `PaladinLVL - 3`.  Because
    /// those bonuses accumulate, a Cleric 3 / Paladin 6 reaches 6, which is
    /// exactly "her cleric level plus her paladin level minus 3".
    ///
    /// What the data cannot express is which energy the character channels.
    /// The SRD ties that to alignment and never to a specific deity, so no
    /// deity data is needed:
    ///
    /// > A good cleric (or a neutral cleric who worships a good deity) can turn
    /// > or destroy undead creatures. An evil cleric (or a neutral cleric who
    /// > worships an evil deity) instead rebukes or commands such creatures. A
    /// > neutral cleric of a neutral deity must choose [...]
    fn turning(
        &self,
        draft: &CharacterDraft,
        variables: &BTreeMap<String, i32>,
        stat_mods: &BTreeMap<String, i32>,
    ) -> Option<crate::snapshot::TurningSnapshot> {
        let native_level: u32 = effective_class_levels(&self.pack, draft)
            .iter()
            .filter(|l| {
                crate::magic::class_magic(&self.pack, &l.class_id)
                    .is_some_and(|m| m.turning_from_level.is_some_and(|n| l.level >= n))
            })
            .map(|l| l.level)
            .sum();
        let level = variables.get("TurnLevelBase").copied().unwrap_or(0) + native_level as i32;
        if level <= 0 {
            return None;
        }
        let forced = effective_class_levels(&self.pack, draft)
            .iter()
            .filter_map(|l| {
                crate::magic::class_magic(&self.pack, &l.class_id)
                    .filter(|m| m.turning_from_level.is_some_and(|n| l.level >= n))
                    .and_then(|m| m.turning_mode)
            })
            .next();
        let (mode, note) = match (forced, variables.get("TurnLevelBase").copied().unwrap_or(0)) {
            (Some(pcgen_core::magic::TurningModeRule::Rebuke), 0) => {
                (crate::snapshot::TurningMode::Rebuke, None)
            }
            (Some(pcgen_core::magic::TurningModeRule::Turn), 0) => {
                (crate::snapshot::TurningMode::Turn, None)
            }
            _ => match crate::snapshot::alignment_morality(&draft.alignment) {
                Some(true) => (crate::snapshot::TurningMode::Turn, None),
                Some(false) => (crate::snapshot::TurningMode::Rebuke, None),
                None => {
                    // A neutral character follows her deity, but the RSRD omits
                    // deities as closed content, so the answer is not in the pack.
                    // Report the choice rather than inventing an alignment.
                    let note = if draft.alignment.trim().is_empty() {
                        "character alignment is not set, so turn or rebuke cannot be decided"
                            .to_string()
                    } else {
                        "the SRD decides a neutral cleric's turning by her deity's alignment, and \
                     this pack carries no deities (the RSRD omits them as closed content)"
                            .to_string()
                    };
                    (crate::snapshot::TurningMode::Choice, Some(note))
                }
            },
        };
        Some(crate::snapshot::TurningSnapshot {
            level: level.max(0) as u32,
            mode,
            times_per_day: if native_level > 0 {
                variables
                    .get("TurnTimesBase")
                    .copied()
                    .unwrap_or(3 + stat_mods.get("CHA").copied()?)
            } else {
                variables.get("TurnTimesBase").copied()?
            },
            damage_bonus: if native_level > 0 {
                level + stat_mods.get("CHA").copied()?
            } else {
                variables.get("TurnDamagePlusBase").copied()?
            },
            note,
        })
    }

    /// General feat slots, which the SRD grants by **character** level.
    ///
    /// SRD, Multiclass Characters:
    ///
    /// > A multiclass character gains feats based on character levels,
    /// > regardless of individual class level.
    ///
    /// <https://www.d20srd.org/srd/classes/multiclass.htm>
    ///
    /// Class bonus feats are a separate pool and are not counted here.  Summing
    /// the per-class progressions instead — as this previously did — over-
    /// grants: a Fighter 1 / Wizard 1 has a character level of 2 and therefore
    /// one general feat, not one per class.
    fn feat_slots(&self, character_level: u32, class_levels: &[ClassLevelSummary]) -> u32 {
        if character_level == 0 {
            return 0;
        }
        // `LEVELSPERFEAT` is a per-class tag in PCGen, but the SRD's feat
        // progression belongs to the character, so take the step from the first
        // class that declares one and default to 3.
        let step = class_levels
            .iter()
            .find_map(|entry| {
                self.progression(&entry.class_id)
                    .map(|progression| progression.levels_per_feat)
            })
            .unwrap_or(3)
            .max(1);
        1 + character_level / step
    }

    /// Derive companion states from the master's companions and the typed
    /// `FOLLOWER:` companion modifiers.
    /// The variable that supplies a companion type's effective master level.
    ///
    /// `FOLLOWER:FamiliarLVL=3` means "this row applies once the familiar's
    /// master level reaches 3", so the variable names the level, and the
    /// numbers are thresholds rather than multipliers.
    fn companion_master_variable(&self, companion_type: &str) -> Option<String> {
        self.pack
            .companion_mods
            .values()
            .flatten()
            .find(|definition| {
                definition
                    .companion_type
                    .eq_ignore_ascii_case(companion_type)
                    && !definition.master_variable.is_empty()
            })
            .map(|definition| definition.master_variable.clone())
    }

    fn companions(
        &self,
        draft: &CharacterDraft,
        variables: &BTreeMap<String, i32>,
        diagnostics: &mut Vec<String>,
    ) -> Vec<crate::snapshot::CompanionSnapshot> {
        let mut models = Vec::new();
        for companion in &draft.companions {
            // SRD, Multiclass Characters: "A sorcerer/wizard stacks his
            // sorcerer and wizard levels to determine the familiar's natural
            // armor, Intelligence score, and special abilities."
            //
            // The data names that stacked value: the RSRD's familiar rows are
            // `FOLLOWER:FamiliarLVL=<threshold>`, so the companion's effective
            // master level is the variable, not whatever the caller passed.
            // `FamiliarLVL` is set from `SorcererLVL` and `WizardLVL`, and
            // those accumulate, which is what makes the two classes stack.
            let effective_master_level = match self
                .companion_master_variable(&companion.companion_type)
            {
                Some(name) => match variables.get(&name).copied() {
                    Some(level) if level >= 0 => level as u32,
                    _ => {
                        diagnostics.push(format!("companion `{}` master-level variable `{name}` unavailable; companion omitted", companion.companion_type));
                        continue;
                    }
                },
                None => companion.master_level,
            };
            let mut model = crate::snapshot::CompanionSnapshot {
                companion_type: companion.companion_type.clone(),
                master_level: effective_master_level,
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
                    if definition.master_level > effective_master_level {
                        continue;
                    }
                    if let Some(raw) = &definition.spell_resistance {
                        if let Some(value) = crate::derived::report(
                            evaluate_companion_formula(raw, effective_master_level, variables)
                                .map_err(|error| {
                                    crate::derived::context(
                                        error,
                                        &format!(
                                            "companion `{}` row `{}` SR",
                                            companion.companion_type, definition.id
                                        ),
                                        raw,
                                    )
                                }),
                            diagnostics,
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
                        let Some(value) = crate::derived::report(
                            evaluate_companion_formula(
                                &bonus.formula,
                                effective_master_level,
                                variables,
                            )
                            .map_err(|error| {
                                crate::derived::context(
                                    error,
                                    &format!(
                                        "companion `{}` row `{}` target `{}`",
                                        companion.companion_type, definition.id, bonus.target
                                    ),
                                    &bonus.formula,
                                )
                            }),
                            diagnostics,
                        ) else {
                            continue;
                        };
                        let target = bonus.target.to_ascii_uppercase();
                        if target == "COMBAT.AC" || target == "AC" {
                            model.armor_class_bonus += value;
                        } else if let Some(stat) = target.strip_prefix("STAT.") {
                            *model.stat_adjustments.entry(stat.to_string()).or_insert(0) += value;
                        }
                    }
                }
            }
            model.abilities.sort();
            model.abilities.dedup();
            model.traits.vision.sort();
            model.traits.vision.dedup();
            // Sort on the value;  is not .
            model
                .traits
                .damage_reduction
                .sort_by(|a, b| a.value.cmp(&b.value));
            model
                .traits
                .damage_reduction
                .dedup_by(|a, b| a.value == b.value);
            model.traits.natural_attacks.sort();
            model.traits.natural_attacks.dedup();
            model.traits.unarmed_damage.sort();
            model.traits.unarmed_damage.dedup();
            models.push(model);
        }
        models
    }
}

fn evaluate_companion_formula(
    formula: &str,
    master_level: u32,
    variables: &BTreeMap<String, i32>,
) -> Result<i32, pcgen_formula::FormulaError> {
    pcgen_formula::evaluate_scoped_floor_i32(
        formula,
        variables,
        &[(
            pcgen_core::formula_vocab::MASTER_LEVEL.to_string(),
            master_level as i32,
        )],
    )
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

fn parse_hit_die(value: &str) -> Option<i32> {
    let raw = value.trim().trim_start_matches(['d', 'D']);
    raw.parse::<i32>().ok().filter(|value| *value > 0)
}

/// A PCGen `HITDIE:` value.
///
/// From PCGen's own tag reference (`docs/listfilepages/datafilestagpages/
/// datafilesclasses.html`):
///
/// ```text
/// HITDIE:x
///   x: Number        Hit Die size
///   x: %+Number      added to the Hit Die
///   x: %-Number      subtracted from the Hit Die
///   x: %*Number      Hit Die is multiplied by
///   x: %/Number      Hit Die is divided by
///   x: %upNumber     stepped up by
///   x: %downNumber   stepped down by
///   x: %HupNumber    stepped up by
///   x: %HdownNumber  stepped down by
/// ```
///
/// The raw data uses three of these and appends a `|CLASS.TYPE=Monster` scope.
/// Neither the scope nor the `%` was handled, so four values silently returned
/// `None` and the override did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitDieRule {
    /// `HITDIE:8` — the die is this.
    Fixed(i32),
    /// `%+N`, `%-N`, `%*N`, `%/N` — arithmetic on the class's own die.
    Adjust(Adjustment, i32),
    /// `%upN` / `%downN` (and the `%H` spellings) — steps of two: 4, 6, 8, 10, 12.
    Steps(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Adjustment {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl HitDieRule {
    fn apply(self, class_die: Option<i32>) -> Option<i32> {
        match self {
            HitDieRule::Fixed(die) => Some(die),
            HitDieRule::Adjust(how, amount) => class_die.map(|die| match how {
                Adjustment::Add => die + amount,
                Adjustment::Subtract => die - amount,
                Adjustment::Multiply => die * amount,
                // Integer division on purpose: PCGen's fractional Hit Die
                // mechanic is `%/8` on a d8, which is a die of 1.
                Adjustment::Divide => die / amount.max(1),
            }),
            HitDieRule::Steps(steps) => class_die.map(|die| (die + steps * 2).clamp(4, 12)),
        }
        .map(|die| die.max(1))
    }
}

/// Parse a `HITDIE:` value, ignoring its `|CLASS.TYPE=...` scope suffix.
///
/// Order matters: lowering stores these with a `d` in front (`d%/12`,
/// `d%up1`), so the `d` has to come off before the `%` is recognisable.
fn parse_hit_die_rule(value: &str) -> Option<HitDieRule> {
    let head = value
        .split('|')
        .next()?
        .trim()
        .trim_start_matches(['d', 'D']);
    let Some(body) = head.strip_prefix('%') else {
        return parse_hit_die(head).map(HitDieRule::Fixed);
    };
    // `%Hup1` / `%Hdown1` are alternate spellings of `%up1` / `%down1`.
    let body = body.strip_prefix(['H', 'h']).unwrap_or(body);
    for (prefix, adjustment) in [
        ("+", Adjustment::Add),
        ("-", Adjustment::Subtract),
        ("*", Adjustment::Multiply),
        ("/", Adjustment::Divide),
    ] {
        if let Some(amount) = body.strip_prefix(prefix) {
            return amount
                .trim()
                .parse::<i32>()
                .ok()
                .map(|amount| HitDieRule::Adjust(adjustment, amount));
        }
    }
    let lower = body.to_ascii_lowercase();
    if let Some(steps) = lower.strip_prefix("up") {
        return steps.trim().parse::<i32>().ok().map(HitDieRule::Steps);
    }
    if let Some(steps) = lower.strip_prefix("down") {
        return steps
            .trim()
            .parse::<i32>()
            .ok()
            .map(|n| HitDieRule::Steps(-n));
    }
    None
}

pub(crate) fn effective_class_levels(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> Vec<ClassLevelSummary> {
    let mut levels: BTreeMap<String, ClassLevelSummary> = BTreeMap::new();
    for entry in draft.class_levels() {
        levels.insert(normalized_name(&entry.class_id), entry);
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
        let entry = levels.entry(key).or_insert_with(|| ClassLevelSummary {
            class_id: target.clone(),
            level: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        });
        entry.level += count;
    }

    levels.into_values().collect()
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

pub(crate) fn stat_modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// Resolve one ability-score modifier, honouring a typed `STATMOD` formula
/// when the compiled pack defines one for `stat`.
pub(crate) fn stat_modifier_for_pack(
    pack: &CompiledRulePack,
    stat: &str,
    score: i32,
) -> Result<i32, pcgen_formula::FormulaError> {
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
        let variables =
            BTreeMap::from([("SCORE".to_string(), score), ("score".to_string(), score)]);
        return pcgen_formula::evaluate_scoped_floor_i32(formula, &variables, &[]).map_err(
            |error| crate::derived::context(error, &format!("stat `{stat}` STATMOD"), formula),
        );
    }
    Ok(stat_modifier(score))
}

pub(crate) fn save_ability_modifier(stat_mods: &BTreeMap<String, i32>, kind: SaveKind) -> i32 {
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
    draft: &CharacterDraft,
    state: &crate::effects::EffectState,
    owner: &str,
    diagnostics: &mut Vec<String>,
) {
    merged.vision.extend(traits.vision.iter().cloned());
    // Each entry carries its own gate (`5/Magic|PRELEVEL:MIN=4`).  Passing the
    // raw string through meant the snapshot showed both entries of a Celestial
    // at level 1, conditions and all.
    merged.damage_reduction.extend(
        traits
            .damage_reduction
            .iter()
            .filter(|entry| crate::effects::predicate_holds(entry.condition.as_ref(), draft, state))
            .map(|entry| entry.value.clone()),
    );
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
        let value = crate::derived::report(
            pcgen_formula::Formula::parse(raw)
                .and_then(|formula| formula.eval_i32(state))
                .map_err(|error| {
                    crate::derived::context(error, &format!("creature `{owner}` SR"), raw)
                }),
            diagnostics,
        );
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
    class_levels: &[ClassLevelSummary],
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

fn ac_bonus_components(bonuses: &[crate::effects::TypedBonus]) -> (i32, i32, i32) {
    let mut groups: BTreeMap<String, (String, BonusGroup)> = BTreeMap::new();
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
        let policy = BonusPolicy::of(bonus.stack_key.as_deref(), &bonus_type);
        let group_key = policy.group(&bonus_type);
        groups
            .entry(group_key)
            .or_insert_with(|| (bonus_type.clone(), BonusGroup::default()))
            .1
            .add(policy, bonus.value);
    }

    let mut armor_class = 0;
    let mut touch_ac = 0;
    let mut flat_footed_ac = 0;
    for (bonus_type, group) in groups.into_values() {
        let value = group.total();
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
            .class_levels()
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
