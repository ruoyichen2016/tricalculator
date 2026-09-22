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
use std::collections::{BTreeMap, BTreeSet};

type SpellcastingMaps = (
    BTreeMap<String, i32>,
    BTreeMap<String, i32>,
    BTreeMap<String, u32>,
    BTreeMap<String, u32>,
    BTreeMap<String, u32>,
);

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

    pub fn evaluate(&self, draft: &CharacterDraft) -> CharacterSnapshot {
        let mut diagnostics = Vec::new();
        let effect_state = apply_effects(&self.pack, draft);
        diagnostics.extend(effect_state.diagnostics.clone());
        let class_levels = effective_class_levels(draft);
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
        let size_modifier = self
            .race(&draft.race_id)
            .map(|race| size_modifier(&race.size))
            .unwrap_or(0);
        let total_level = draft.total_level();
        let con_mod = stat_modifier(stats.get("CON").copied().unwrap_or(10));
        let dex_mod = stat_modifier(stats.get("DEX").copied().unwrap_or(10));
        let equipment = self.resolve_equipment(draft, &mut diagnostics);
        let (armor_bonus, shield_bonus, max_dex, armor_check_penalty) =
            equipment_totals(&equipment);
        let dex_bonus = match max_dex {
            Some(cap) => dex_mod.min(cap),
            None => dex_mod,
        };
        let retain_dex_flat_footed = effect_state.active_abilities.iter().any(|ability| {
            let ability = ability.to_ascii_lowercase();
            ability.contains("retain_dex") || ability.contains("uncanny_dodge")
        });
        let (effect_ac, effect_touch, effect_flat) = ac_bonus_components(&effect_state.ac_bonuses);
        let armor_class = 10 + armor_bonus + shield_bonus + dex_bonus + size_modifier + effect_ac;
        let touch_ac = 10 + dex_mod + size_modifier + effect_touch;
        // Flat-footed loses a positive Dexterity bonus but still applies a
        // Dexterity penalty (PHB 3.5, flat-footed).
        let flat_footed_dex = if retain_dex_flat_footed {
            dex_bonus
        } else {
            dex_bonus.min(0)
        };
        let flat_footed_ac =
            10 + armor_bonus + shield_bonus + flat_footed_dex + size_modifier + effect_flat;

        let gestalt = if draft.is_gestalt() {
            Some(plan_gestalt(
                &draft.advancement,
                self,
                stat_modifier(stats.get("INT").copied().unwrap_or(10)),
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
            spells
        };
        let variant_effects = {
            let mut effects = BTreeMap::new();
            for frame in &draft.advancement {
                for lane in &frame.lanes {
                    if let Some(variant) = self.variant_for(lane, &lane.class_id) {
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
                .map(|entry| hit_points_for_entry(entry, con_mod))
                .sum()
        };

        let saves = if let Some(plan) = &gestalt {
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
                    + save_ability_modifier(&stats, kind);
                saves.insert(key.to_string(), total);
            }
            saves
        } else {
            let mut fort = save_ability_modifier(&stats, SaveKind::Fortitude);
            let mut reflex = save_ability_modifier(&stats, SaveKind::Reflex);
            let mut will = save_ability_modifier(&stats, SaveKind::Will);
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

        let base_attack_bonus = crate::bab::compute_bab(
            &self.pack,
            draft,
            &effect_state.variables,
            &effect_state.external_modifiers,
        );

        let skills = self.skills(draft, &stats, armor_check_penalty);
        let (caster_levels, spell_dcs, spell_slots, known_limits, prepared_spells) =
            self.spellcasting(draft, &class_levels, &stats);
        let (power_points, manifester_levels) = self.psionics(&class_levels, &stats);

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

        let mut snapshot = CharacterSnapshot {
            draft_id: draft.id.clone(),
            name: draft.name.clone(),
            race_id: draft.race_id.clone(),
            total_level,
            stats,
            hit_points,
            base_attack_bonus,
            initiative: dex_mod,
            armor_class,
            armor_bonus,
            shield_bonus,
            touch_ac,
            flat_footed_ac,
            saves,
            skills,
            caster_levels,
            spell_dcs,
            spell_slots,
            known_limits,
            prepared_spells,
            spell_power_types: effect_state.spell_powers.clone(),
            max_spell_level: effect_state.max_spell_level.clone(),
            has_spellbook: effect_state.has_spellbook,
            power_points,
            manifester_levels,
            gestalt,
            faith_decisions,
            domain_spells,
            variant_effects,
            explanations,
            diagnostics,
            variables: effect_state.variables,
            external_modifiers: effect_state.external_modifiers,
            active_abilities: effect_state.active_abilities.into_iter().collect(),
            choices: effect_state.choices,
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

    /// Validate structural references and Gestalt policy constraints.
    pub fn validate(&self, draft: &CharacterDraft) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for entry in effective_class_levels(draft) {
            if self.class(&entry.class_id).is_none() {
                diagnostics.push(format!("unknown class `{}`", entry.class_id));
            }
        }
        if draft.is_gestalt() {
            let plan = plan_gestalt(
                &draft.advancement,
                self,
                stat_modifier(draft.stats.get("INT").copied().unwrap_or(10)),
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
        diagnostics
    }

    fn faith_checks(&self, draft: &CharacterDraft) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for track in &collect_divine_tracks(draft) {
            if let Some(deity) = &track.deity {
                match self.deity(deity) {
                    Some(definition) => {
                        for domain in &track.domains {
                            if !definition.domains.is_empty()
                                && !definition
                                    .domains
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
        diagnostics
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

    fn domain(&self, domain: &str) -> Option<&pcgen_core::DomainDefinition> {
        self.pack.domains.get(domain).or_else(|| {
            self.pack
                .domains
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(domain))
                .map(|(_, domain)| domain)
        })
    }

    fn variant_for(&self, lane: &AdvancementLane, class_id: &str) -> Option<&VariantDefinition> {
        let id = lane
            .archetype
            .as_deref()
            .or(lane.substitution_level.as_deref())?;
        self.pack
            .variants
            .get(id)
            .or_else(|| {
                self.pack
                    .variants
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(id))
                    .map(|(_, variant)| variant)
            })
            .filter(|variant| {
                variant
                    .applies_to
                    .as_deref()
                    .is_none_or(|applies| applies.eq_ignore_ascii_case(class_id))
            })
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
        draft
            .equipment
            .iter()
            .filter_map(|item| {
                let found = self
                    .pack
                    .equipment
                    .get(item)
                    .or_else(|| {
                        self.pack
                            .equipment
                            .iter()
                            .find(|(key, definition)| {
                                key.eq_ignore_ascii_case(item)
                                    || definition.name.eq_ignore_ascii_case(item)
                            })
                            .map(|(_, definition)| definition)
                    })
                    .cloned();
                if found.is_none() {
                    diagnostics.push(format!("missing equipment definition for `{item}`"));
                }
                found
            })
            .collect()
    }

    fn skills(
        &self,
        draft: &CharacterDraft,
        stats: &BTreeMap<String, i32>,
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
                .map(|key_stat| stat_modifier(stats.get(key_stat).copied().unwrap_or(10)))
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

    fn spellcasting(
        &self,
        draft: &CharacterDraft,
        class_levels: &[DraftClassLevel],
        stats: &BTreeMap<String, i32>,
    ) -> SpellcastingMaps {
        let mut caster_levels = BTreeMap::new();
        let mut spell_slots = BTreeMap::new();
        let mut known_limits = BTreeMap::new();
        for entry in class_levels {
            let Some(class) = self.class(&entry.class_id) else {
                continue;
            };
            let Some(stat) = class.spellcasting_ability.as_deref() else {
                continue;
            };
            let _ = stat;
            let variant_advance = draft.advancement.iter().any(|frame| {
                frame.lanes.iter().any(|lane| {
                    lane.class_id.eq_ignore_ascii_case(&class.id)
                        && self
                            .variant_for(lane, &class.id)
                            .is_some_and(|variant| variant.spellcaster_advance)
                })
            });
            caster_levels.insert(
                class.id.clone(),
                entry.level as i32 + i32::from(variant_advance),
            );
            if let Some(slots) = class.spell_slots.get(&entry.level) {
                for (level, count) in slots.iter().enumerate() {
                    spell_slots.insert(format!("{}|{}", class.id, level), *count);
                }
            }
            if let Some(known) = class.spells_known.get(&entry.level) {
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
            let dc = 10 + level as i32 + stat_modifier(stats.get(&stat).copied().unwrap_or(10));
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
        stats: &BTreeMap<String, i32>,
    ) -> (BTreeMap<String, i32>, BTreeMap<String, i32>) {
        let mut power_points = BTreeMap::new();
        let mut manifester_levels = BTreeMap::new();
        for entry in class_levels {
            let Some(class) = self.class(&entry.class_id) else {
                continue;
            };
            if !class.is_psionic {
                continue;
            }
            let stat = class.spellcasting_ability.as_deref().unwrap_or("INT");
            let ability_mod = stat_modifier(stats.get(stat).copied().unwrap_or(10));
            let total = pcgen_core::total_power_points(&class.id, entry.level, ability_mod, 0);
            power_points.insert(class.id.clone(), total);
            manifester_levels.insert(class.id.clone(), entry.level as i32);
        }
        (power_points, manifester_levels)
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
        match self.variant_for(lane, class_id) {
            Some(variant) => Some(variant.apply_to_features(base)),
            None => Some(base),
        }
    }

    fn class_skills_for_lane(&self, lane: &AdvancementLane, class_id: &str) -> Option<Vec<String>> {
        let mut skills = self.class_skills(class_id)?;
        if let Some(variant) = self.variant_for(lane, class_id) {
            skills.extend(variant.class_skills.iter().cloned());
        }
        Some(skills)
    }
}

pub fn validate(_pack: &CompiledRulePack, _draft: &CharacterDraft) -> Vec<String> {
    Vec::new()
}

fn hit_points_for_entry(entry: &DraftClassLevel, con_mod: i32) -> i32 {
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
    let raw = entry.hit_points_gained + con_mod * entry.level as i32;
    if entry.hit_points_gained == 0 {
        raw.max(0)
    } else {
        raw.max(entry.level as i32)
    }
}

pub(crate) fn effective_class_levels(draft: &CharacterDraft) -> Vec<DraftClassLevel> {
    if !draft.class_levels.is_empty() {
        return draft.class_levels.clone();
    }
    let mut levels: BTreeMap<String, DraftClassLevel> = BTreeMap::new();
    for frame in &draft.advancement {
        for lane in &frame.lanes {
            let entry = levels
                .entry(lane.class_id.clone())
                .or_insert_with(|| DraftClassLevel {
                    class_id: lane.class_id.clone(),
                    level: 0,
                    hit_points_gained: 0,
                    hit_points_by_level: Vec::new(),
                    experience_gained: 0,
                });
            entry.level = entry.level.max(lane.class_level);
            entry.hit_points_gained += lane.hit_points_gained;
            entry.experience_gained += lane.experience_gained;
        }
    }
    levels.into_values().collect()
}

pub(crate) fn stat_modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

fn save_ability_modifier(stats: &BTreeMap<String, i32>, kind: SaveKind) -> i32 {
    let stat = match kind {
        SaveKind::Fortitude => "CON",
        SaveKind::Reflex => "DEX",
        SaveKind::Will => "WIS",
    };
    stat_modifier(stats.get(stat).copied().unwrap_or(10))
}

fn size_modifier(size: &str) -> i32 {
    match size.to_ascii_lowercase().as_str() {
        "fine" => 8,
        "diminutive" => 4,
        "tiny" => 2,
        "small" => 1,
        "large" => -1,
        "huge" => -2,
        "gargantuan" => -4,
        "colossal" => -8,
        _ => 0,
    }
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
        if tags.contains("shield") {
            shield += item.armor_bonus;
        } else if tags.contains("armor") {
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

fn ac_bonus_components(bonuses: &[crate::effects::AcBonus]) -> (i32, i32, i32) {
    let mut armor_class = 0;
    let mut touch_ac = 0;
    let mut flat_footed_ac = 0;
    for bonus in bonuses {
        let bonus_type = bonus
            .bonus_type
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        if bonus_type.contains("armor")
            || bonus_type.contains("shield")
            || bonus_type.contains("natural")
        {
            // Armor, shield and natural armor apply to normal/flat-footed AC,
            // but not touch AC.
            armor_class += bonus.value;
            flat_footed_ac += bonus.value;
        } else if bonus_type.contains("dodge") {
            // Dodge applies to normal/touch AC, but not flat-footed AC.
            armor_class += bonus.value;
            touch_ac += bonus.value;
        } else {
            // Deflection, insight, luck, untyped, etc. apply to all three.
            armor_class += bonus.value;
            touch_ac += bonus.value;
            flat_footed_ac += bonus.value;
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
    tracks
}

pub(crate) fn evaluate_class_formula(
    formula: &str,
    class_level: u32,
    total_level: u32,
    variables: &BTreeMap<String, i32>,
) -> i32 {
    let mut normalized = formula.to_string();
    let classlevel = regex::Regex::new(r#"classlevel\s*\(\s*["']?[^"')]*["']?\s*\)"#).unwrap();
    normalized = classlevel
        .replace_all(&normalized, class_level.to_string())
        .to_string();
    let total = regex::Regex::new(r"\bTL\b").unwrap();
    normalized = total
        .replace_all(&normalized, total_level.to_string())
        .to_string();
    let class = regex::Regex::new(r"\bCL\b").unwrap();
    normalized = class
        .replace_all(&normalized, class_level.to_string())
        .to_string();
    let var_call = regex::Regex::new(r#"var\s*\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap();
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
