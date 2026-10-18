//! Shared casting progression, access and spell transformations.
use crate::{CharacterDraft, Engine};
use pcgen_core::magic::*;
use pcgen_core::{CastingType, ClassDefinition, CompiledRulePack, SpellPowerType};
use serde::Serialize;
use std::collections::BTreeMap;
fn find_class<'a>(pack: &'a CompiledRulePack, id: &str) -> Option<&'a ClassDefinition> {
    pack.classes
        .values()
        .find(|c| c.id.eq_ignore_ascii_case(id))
}
pub(crate) fn casting_levels(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    diagnostics: &mut Vec<String>,
) -> BTreeMap<String, u32> {
    let class_levels = crate::engine::effective_class_levels(pack, draft);
    // Each advancing level belongs to one selected casting track. Never
    // broadcast an Arcane advance to every arcane class on the character.
    let mut advanced: BTreeMap<String, u32> = BTreeMap::new();
    for entry in &class_levels {
        let Some(class) = find_class(pack, &entry.class_id) else {
            continue;
        };
        for (level, kinds) in &class.level_spellcaster_advance {
            if *level > entry.level {
                continue;
            }
            let acquired_at = draft
                .advancement
                .iter()
                .find(|frame| {
                    frame.lanes.iter().any(|lane| {
                        lane.class_id.eq_ignore_ascii_case(&entry.class_id)
                            && lane.class_level == *level
                    })
                })
                .map(|frame| frame.character_level);
            let candidates: Vec<_> = class_levels
                .iter()
                .filter_map(|track| {
                    if let Some(acquired_at) = acquired_at {
                        if !draft
                            .advancement
                            .iter()
                            .filter(|frame| frame.character_level < acquired_at)
                            .flat_map(|frame| &frame.lanes)
                            .any(|lane| lane.class_id.eq_ignore_ascii_case(&track.class_id))
                        {
                            return None;
                        }
                    }
                    let target = find_class(pack, &track.class_id)?;
                    if target.spellcasting_ability.is_none()
                        || target.is_psionic
                        || !target.level_spellcaster_advance.is_empty()
                    {
                        return None;
                    }
                    let matches = kinds.iter().any(|kind| {
                        let kind = kind.to_ascii_uppercase();
                        kind == "ANY"
                            || kind == "SPELLCASTER"
                            || (kind == "ARCANE"
                                && target.spell_power == pcgen_core::SpellPowerType::Arcane)
                            || (kind == "DIVINE"
                                && target.spell_power == pcgen_core::SpellPowerType::Divine)
                    });
                    matches.then_some(target.id.clone())
                })
                .collect();
            if kinds
                .iter()
                .all(|kind| kind.to_ascii_uppercase().contains("PSIONIC"))
            {
                continue;
            }
            let chosen: Vec<_> = draft
                .advancement
                .iter()
                .flat_map(|frame| &frame.lanes)
                .filter(|lane| {
                    lane.class_id.eq_ignore_ascii_case(&entry.class_id)
                        && lane.class_level == *level
                })
                .flat_map(|lane| lane.advances.iter().cloned())
                .collect();
            let target = match chosen.as_slice() {
                [target] if candidates.iter().any(|id| id.eq_ignore_ascii_case(target)) => {
                    candidates
                        .iter()
                        .find(|id| id.eq_ignore_ascii_case(target))
                        .cloned()
                }
                [] if candidates.len() == 1 => candidates.first().cloned(),
                _ => None,
            };
            if let Some(target) = target {
                *advanced.entry(target).or_default() += 1;
            } else {
                diagnostics.push(format!("{} level {}: casting advancement needs one eligible target; chosen {:?}, eligible {:?}", entry.class_id, level, chosen, candidates));
            }
        }
    }

    class_levels
        .iter()
        .filter_map(|entry| {
            let c = find_class(pack, &entry.class_id)?;
            if c.spellcasting_ability.is_none()
                || c.is_psionic
                || (c.is_prestige && !c.level_spellcaster_advance.is_empty())
            {
                return None;
            }
            let variant = draft
                .advancement
                .iter()
                .flat_map(|f| &f.lanes)
                .filter(|l| l.class_id.eq_ignore_ascii_case(&c.id))
                .any(|l| {
                    l.archetype
                        .iter()
                        .chain(l.substitution_level.iter())
                        .any(|id| {
                            pack.variants.values().any(|v| {
                                v.id.eq_ignore_ascii_case(id)
                                    && v.applies_to
                                        .as_deref()
                                        .is_none_or(|id| id.eq_ignore_ascii_case(&c.id))
                                    && v.spellcaster_advance
                            })
                        })
                });
            Some((
                c.id.clone(),
                entry.level + advanced.get(&c.id).copied().unwrap_or(0) + u32::from(variant),
            ))
        })
        .collect()
}

pub(crate) fn class_magic<'a>(pack: &'a CompiledRulePack, id: &str) -> Option<&'a ClassMagic> {
    pack.entities
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(id) && e.metadata.class_magic.is_some())
        .and_then(|e| e.metadata.class_magic.as_ref())
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccessibleSpell {
    pub spell: String,
    pub level: u32,
    pub power: SpellPowerType,
    pub known: bool,
    pub prohibited: bool,
    pub source_list: String,
}
#[derive(Debug, Serialize)]
pub struct CastResolution {
    pub action_cost: Option<pcgen_core::actions::ActionCost>,
    pub feature_costs: BTreeMap<String, u32>,
    pub penetration_bonus: i32,
    pub bypass_sr: bool,
    pub damage: Option<crate::class_actions::DamageResolution>,
    pub area_multiplier: u32,
    pub instant_uses_remaining: u32,
    pub warmage_edge: Option<i32>,
    pub arcane_failure_percent: Option<u32>,
    pub material_cost_gp: Option<u32>,
    pub xp_cost: Option<u32>,
    pub effect_timing: String,
    pub applied_metamagic: Vec<SpellRule>,
    pub spell: String,
    pub class_id: String,
    pub power: SpellPowerType,
    pub base_level: u32,
    pub effective_level: i32,
    pub required_slot_level: u32,
    pub paid_slot_level: u32,
    pub paid_slots: u32,
    pub caster_level: i32,
    pub save_dc: i32,
    pub turning_cost: u32,
    pub turning_remaining: u32,
    pub duration: Option<String>,
    pub range: SpellRange,
    pub range_multiplier: u32,
    pub range_feet: Option<u32>,
    pub tapestry: bool,
    pub quiescent_capacity: Option<u32>,
    pub casting_time_note: String,
    pub adjudications: Vec<String>,
}
impl Engine {
    /// List access is independent of spell knowledge, slots and casting power.
    pub fn spell_catalog(
        &self,
        draft: &CharacterDraft,
        track: &str,
    ) -> Result<Vec<AccessibleSpell>, String> {
        if draft.build_history.is_some() {
            return self.spell_catalog(&draft.materialized()?, track);
        }
        let mut errors = Vec::new();
        let tracks = casting_levels(&self.pack, draft, &mut errors);
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        let c = find_class(&self.pack, track).ok_or("Unknown casting class")?;
        let cl = tracks
            .get(&c.id)
            .ok_or("Character does not have this casting track")?;
        let Some(table) = c.spell_slots.get(cl) else {
            return Ok(Vec::new());
        };
        let max_level = table
            .len()
            .checked_sub(1)
            .ok_or("Empty spell-slot progression")? as u32;
        let policy = class_magic(&self.pack, &c.id);
        let minimum = policy.map(|m| m.first_spell_level).unwrap_or(0);
        let all_known = policy.is_some_and(|m| m.knows_entire_list || m.prepared_entire_list);
        let levels = crate::engine::effective_class_levels(&self.pack, draft);
        let access: Vec<_> = levels
            .iter()
            .filter_map(|l| class_magic(&self.pack, &l.class_id).map(|m| (l, m)))
            .flat_map(|(l, m)| m.list_access.iter().filter(move |a| a.at_level <= l.level))
            .filter(|a| {
                a.target_class
                    .as_ref()
                    .is_none_or(|id| id.eq_ignore_ascii_case(&c.id))
                    && a.power.is_none_or(|p| p == c.spell_power)
            })
            .collect();
        let learned = crate::class_features::learned_spells(&self.pack, draft, &c.id)?;
        let domain_access: Vec<_> = levels
            .iter()
            .filter_map(|l| class_magic(&self.pack, &l.class_id).map(|m| (l, m)))
            .flat_map(|(l, m)| m.domains.iter().filter(move |d| d.level <= l.level))
            .collect();
        let prohibited = native_prohibited_schools(&self.pack, draft)?;
        let mut result = Vec::new();
        for spell in self.pack.spells.values() {
            let own = spell
                .class_levels
                .iter()
                .find(|(id, _)| id.eq_ignore_ascii_case(&c.id))
                .map(|(_, l)| *l);
            let extra = access.iter().find_map(|a| {
                spell
                    .class_levels
                    .iter()
                    .find(|(id, level)| {
                        id.eq_ignore_ascii_case(&a.list)
                            && a.max_spell_level.is_none_or(|max| **level <= max)
                    })
                    .map(|(_, l)| (*l, *a))
            });
            let learned_level = learned
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&spell.name))
                .map(|(_, l)| *l);
            let domain = domain_access.iter().find_map(|d| {
                d.spells
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&spell.name))
                    .map(|(_, l)| (*l, d.domain.clone()))
            });
            let Some(level) = own
                .or(learned_level)
                .or_else(|| extra.map(|(l, _)| l))
                .or_else(|| domain.as_ref().map(|(l, _)| *l))
            else {
                continue;
            };
            // Even a spell already on the warmage list can become divine under
            // the Cleric Spell Access rule; Wizard/Sorcerer/Bard overlap decides.
            let power = if let Some((_, a)) = extra {
                if !a.divine_unless_on.is_empty()
                    && !a.divine_unless_on.iter().any(|id| {
                        spell
                            .class_levels
                            .keys()
                            .any(|k| k.eq_ignore_ascii_case(id))
                    })
                {
                    SpellPowerType::Divine
                } else {
                    c.spell_power
                }
            } else {
                c.spell_power
            };
            let known = level >= minimum
                && level <= max_level
                && ((all_known && (own.is_some() || learned_level.is_some() || extra.is_some()))
                    || draft.spells.iter().any(|s| {
                        s.name.eq_ignore_ascii_case(&spell.name)
                            && s.class_id
                                .as_deref()
                                .is_some_and(|id| id.eq_ignore_ascii_case(&c.id))
                    }));
            result.push(AccessibleSpell {
                spell: spell.name.clone(),
                level,
                power,
                known,
                prohibited: prohibited
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(&spell.school)),
                source_list: if own.is_some() {
                    c.id.clone()
                } else {
                    extra
                        .map(|(_, a)| a.list.clone())
                        .or_else(|| domain.map(|(_, n)| format!("Domain: {n}")))
                        .unwrap_or_else(|| "Advanced Learning".into())
                },
            });
        }
        Ok(result)
    }

    pub fn cast_preview(
        &self,
        draft: &CharacterDraft,
        request: &CastRequest,
    ) -> Result<CastResolution, String> {
        if draft.build_history.is_some() {
            return self.cast_preview(&draft.materialized()?, request);
        }
        if draft.class_play.encounter.rage.is_some() {
            return Err("Cannot cast spells while raging".into());
        }
        let catalog = self.spell_catalog(draft, &request.class_id)?;
        let access = catalog
            .iter()
            .find(|s| s.spell.eq_ignore_ascii_case(&request.spell))
            .ok_or("Spell is not on an accessible list")?;
        if access.prohibited {
            return Err("Focused Studies prohibits this spell school".into());
        }
        if !access.known {
            return Err("Spell is not known at the current progression; Versatile Spellcaster does not grant knowledge".into());
        }
        let spell = self
            .pack
            .spells
            .values()
            .find(|s| s.name.eq_ignore_ascii_case(&access.spell))
            .ok_or("Missing spell definition")?;
        let c = find_class(&self.pack, &request.class_id).ok_or("Unknown casting class")?;
        let state = crate::apply_effects(&self.pack, draft);
        if !state.diagnostics.is_empty() {
            return Err(state.diagnostics.join("; "));
        }
        let require = |rule: SpellRule| -> Result<(), String> {
            let entity = self
                .pack
                .entities
                .iter()
                .find(|e| {
                    e.metadata.spell_rule == Some(rule)
                        && (crate::effects::ability_matches_draft(draft, e)
                            || state.active_abilities.contains(&e.id.key))
                })
                .ok_or_else(|| format!("Required feat is not selected: {rule:?}"))?;
            for pre in &entity.requirements {
                if crate::effects::predicate_result(Some(pre), draft, &state) != Some(true) {
                    return Err(format!(
                        "Unmet or unavailable prerequisite for {}",
                        entity.name
                    ));
                }
            }
            Ok(())
        };
        let snapshot = self.evaluate(draft);
        if !snapshot.diagnostics.is_empty() {
            return Err(snapshot.diagnostics.join("; "));
        }
        let ability = c
            .spellcasting_ability
            .as_ref()
            .ok_or("Missing casting ability")?;
        let score = *snapshot
            .stats
            .get(ability)
            .ok_or("Casting ability score is unavailable")?;
        let modifier = (score - 10).div_euclid(2);
        let dc_ability = policy_dc_ability(&self.pack, &c.id).unwrap_or(ability);
        let dc_modifier = (snapshot
            .stats
            .get(dc_ability)
            .ok_or("Save DC ability score is unavailable")?
            - 10)
            .div_euclid(2);
        let mut caster_level = *snapshot
            .caster_levels
            .get(&c.id)
            .ok_or("Caster level unavailable")?;
        for l in crate::engine::effective_class_levels(&self.pack, draft) {
            if let Some(m) = class_magic(&self.pack, &l.class_id) {
                for bonus in &m.descriptor_bonuses {
                    if l.level >= bonus.at_level
                        && spell
                            .descriptors
                            .iter()
                            .any(|d| d.eq_ignore_ascii_case(&bonus.descriptor))
                    {
                        caster_level += bonus.caster_level;
                    }
                }
            }
        }
        let mut effective = access.level as i32;
        let mut slot = access.level;
        let mut heightening = 0;
        if let Some(level) = request.heighten_to {
            require(SpellRule::Heighten)?;
            if level <= access.level || level > 9 {
                return Err("Heighten must raise the spell level, at most to ninth".into());
            }
            heightening = level - access.level;
            effective = level as i32;
            slot = level;
        }
        if request.earth {
            require(SpellRule::Earth)?;
            if request.versatile && !request.earth_uses_virtual_slot {
                return Err("Earth Spell with Versatile Spellcaster requires the explicit virtual-slot adjudication".into());
            }
            if heightening == 0 || !request.on_stone_or_unworked_earth {
                return Err(
                    "Earth Spell requires Heighten and contact with stone or unworked earth".into(),
                );
            }
            if spell.descriptors.iter().any(|d| {
                ["air", "fire", "water"]
                    .iter()
                    .any(|x| d.eq_ignore_ascii_case(x))
            }) {
                return Err("Earth Spell excludes air, fire and water spells".into());
            }
            effective += 1;
            caster_level += heightening as i32;
        }
        let mut duration = spell.profile.duration.clone();
        let mut range = spell.profile.range;
        if request.spellguard_touch {
            let level = crate::engine::effective_class_levels(&self.pack, draft)
                .iter()
                .find(|c| c.class_id == "Spellguard of Silverymoon")
                .map(|c| c.level)
                .unwrap_or(0);
            range = crate::spell_features::spellguard_range(level, access.power, &spell.profile)?;
        }
        let mut range_multiplier = 1;
        let mut turning_cost = 0;
        let mut applied = Vec::new();
        for rule in &request.metamagic {
            if applied.contains(rule) {
                return Err("A metamagic feat cannot be applied twice".into());
            }
            applied.push(*rule);
            let sudden = request.sudden.contains(rule);
            if sudden {
                if !crate::class_features::sudden_granted(&self.pack, draft, *rule) {
                    return Err("Sudden metamagic is not granted by an acquired class level".into());
                }
                let key = serde_rule_name(*rule);
                if request.sudden_spent.get(key).copied().unwrap_or(0) >= 1 {
                    return Err("This Sudden metamagic daily use is exhausted".into());
                }
            } else {
                require(*rule)?;
            }
            let instant = request.instant_metamagic == Some(*rule);
            if *rule == SpellRule::Quicken
                && c.casting_type == CastingType::Spontaneous
                && !instant
                && !sudden
            {
                return Err("Normal spontaneous metamagic cannot use Quicken Spell".into());
            }
            let adjustment = crate::metamagic::transform(
                &self.pack,
                spell,
                *rule,
                request.inside_sanctum,
                &mut effective,
                &mut duration,
                range,
                &mut range_multiplier,
            )?;
            let adjustment = crate::class_features::improved_cost(&self.pack, draft, adjustment);
            if (sudden || instant) && request.divine_metamagic == Some(*rule) {
                return Err("Choose only one payment method for each metamagic application".into());
            }
            if request.divine_metamagic == Some(*rule) {
                require(SpellRule::DivineMetamagic)?;
                if access.power != SpellPowerType::Divine {
                    return Err("Divine Metamagic requires a divine spell; cleric-list overlap does not suffice".into());
                }
                let selected = self
                    .pack
                    .entities
                    .iter()
                    .find(|e| e.metadata.spell_rule == Some(*rule))
                    .ok_or("Missing metamagic definition")?;
                let dmm = self
                    .pack
                    .entities
                    .iter()
                    .find(|e| e.metadata.spell_rule == Some(SpellRule::DivineMetamagic))
                    .ok_or("Missing Divine Metamagic definition")?;
                if !draft.choices.iter().any(|(k, v)| {
                    (k.eq_ignore_ascii_case(&dmm.name)
                        || k.eq_ignore_ascii_case(&dmm.id.key)
                        || k == &dmm.id.as_string())
                        && (v.eq_ignore_ascii_case(&selected.name)
                            || v.eq_ignore_ascii_case(&selected.id.key))
                }) {
                    return Err(
                        "Divine Metamagic must be bound to this specific metamagic feat".into(),
                    );
                }
                turning_cost = 1 + adjustment;
            } else if !sudden && !instant {
                slot += adjustment;
            }
        }
        if request
            .divine_metamagic
            .is_some_and(|r| !applied.contains(&r))
        {
            return Err("Divine Metamagic target is not applied to this cast".into());
        }
        if score < 10 + effective.max(0) {
            return Err("Casting ability is too low for the effective spell level".into());
        }
        let levels = crate::engine::effective_class_levels(&self.pack, draft);
        let turning_total = snapshot
            .turning
            .as_ref()
            .map(|t| t.times_per_day.max(0) as u32)
            .unwrap_or(0);
        if request.turning_spent > turning_total
            || turning_cost > turning_total - request.turning_spent
        {
            return Err(format!("Insufficient turning attempts: need {turning_cost}, total {turning_total}, already spent {}",request.turning_spent));
        }
        let (paid_slot_level, paid_slots) = if request.versatile {
            require(SpellRule::Versatile)?;
            if c.casting_type != CastingType::Spontaneous || slot == 0 {
                return Err("Versatile Spellcaster requires spontaneous casting and a nonzero resulting slot level".into());
            }
            (slot - 1, 2)
        } else {
            (slot, 1)
        };
        if request
            .sudden
            .iter()
            .any(|r| !request.metamagic.contains(r))
            || request
                .sudden
                .iter()
                .enumerate()
                .any(|(i, r)| request.sudden[..i].contains(r))
        {
            return Err("Sudden metamagic must identify unique applied feats".into());
        }
        if let Some(rule) = request.instant_metamagic {
            if !request.metamagic.contains(&rule) || request.sudden.contains(&rule) {
                return Err("Instant Metamagic must identify one applied feat".into());
            }
            let uses = crate::class_features::instant_uses(&self.pack, draft);
            if request.instant_spent >= uses {
                return Err("Instant Metamagic is unavailable or exhausted".into());
            }
            let adjusted = slot
                + crate::class_features::improved_cost(
                    &self.pack,
                    draft,
                    crate::metamagic::cost(&self.pack, rule)?.slot_increase,
                );
            let highest = snapshot
                .spell_slots
                .iter()
                .filter(|(_, count)| **count > 0)
                .filter_map(|(key, _)| {
                    key.strip_prefix(&format!("{}|", c.id))
                        .and_then(|n| n.parse::<u32>().ok())
                })
                .filter(|n| *n as i32 <= score - 10)
                .max()
                .ok_or("Maximum slot level unavailable")?;
            if adjusted > highest {
                return Err("Instant Metamagic exceeds the highest castable slot".into());
            }
        }
        let progression = casting_levels(&self.pack, draft, &mut Vec::new());
        let table = c
            .spell_slots
            .get(&progression[&c.id])
            .ok_or("Missing slot table")?;
        let base = *table
            .get(paid_slot_level as usize)
            .ok_or("Requested slot level has not been acquired")?;
        if paid_slot_level
            < class_magic(&self.pack, &c.id)
                .map(|m| m.first_spell_level)
                .unwrap_or(0)
        {
            return Err("This class has no slots of that spell level".into());
        }
        let bonus = if paid_slot_level > 0 && modifier >= paid_slot_level as i32 {
            1 + (modifier - paid_slot_level as i32) as u32 / 4
        } else {
            0
        };
        let spent = request
            .slots_spent
            .get(&paid_slot_level)
            .copied()
            .unwrap_or(0);
        if spent > base + bonus || paid_slots > base + bonus - spent {
            return Err("Insufficient spell slots".into());
        }
        if c.casting_type == CastingType::Prepared {
            if !request.metamagic.is_empty() || request.heighten_to.is_some() || request.versatile {
                return Err("Prepared metamagic needs a recorded prepared variant; spontaneous payment is not applicable".into());
            }
            let copies = draft.prepared_copies(&c.id, &spell.name)?;
            let key = format!(
                "{}|{}",
                c.id.to_ascii_lowercase(),
                spell.name.to_ascii_lowercase()
            );
            let used = draft
                .class_play
                .resources
                .prepared_spent
                .get(&key)
                .copied()
                .unwrap_or(0);
            if copies <= used {
                return Err("No unspent prepared copy of this spell".into());
            }
        }
        let track_magic = class_magic(&self.pack, &c.id);
        let warmage_edge = match (
            track_magic.and_then(|m| m.edge_ability.as_ref()),
            spell.profile.hit_point_damage,
        ) {
            (Some(key), Some(true)) => Some(
                (snapshot.stats.get(key).ok_or("Edge ability unavailable")? - 10)
                    .div_euclid(2)
                    .max(0),
            ),
            (Some(_), None) => None,
            _ => Some(0),
        };
        let arcane_failure_percent = request
            .armor
            .as_ref()
            .map(|armor| -> Result<u32, String> {
                if armor.armor_failure > 100 || armor.shield_failure > 100 {
                    return Err("Armor failure percentages must be 0 to 100".into());
                }
                if access.power == SpellPowerType::Divine
                    || request.metamagic.contains(&SpellRule::Still)
                    || spell.profile.somatic == Some(false)
                {
                    return Ok(0);
                }
                if spell.profile.somatic.is_none() {
                    return Err("Somatic component data is unavailable for this spell".into());
                }
                let level = levels
                    .iter()
                    .find(|l| l.class_id.eq_ignore_ascii_case(&c.id))
                    .map(|l| l.level)
                    .ok_or("Casting class level unavailable")?;
                let feature = track_magic.and_then(|m| m.armored_casting.as_ref());
                let ignores_armor = match armor.armor {
                    ArmorWeight::None => true,
                    ArmorWeight::Light => feature.is_some_and(|f| level >= f.light_from_level),
                    ArmorWeight::Medium => feature.is_some_and(|f| level >= f.medium_from_level),
                    ArmorWeight::Heavy => false,
                };
                let ignores_shield = armor.shield == ShieldWeight::None
                    || (armor.shield == ShieldWeight::Heavy
                        && feature.is_some_and(|f| {
                            f.heavy_shield_from_level.is_some_and(|at| level >= at)
                        }))
                    || (armor.shield == ShieldWeight::Light
                        && feature.is_some_and(|f| f.light_shield && level >= f.light_from_level));
                Ok((if ignores_armor {
                    0
                } else {
                    armor.armor_failure
                } + if ignores_shield {
                    0
                } else {
                    armor.shield_failure
                })
                .min(100))
            })
            .transpose()?;
        let mut material_cost_gp = spell.profile.material_cost_gp;
        let mut xp_cost = spell.profile.xp_cost;
        let mut capacity = None;
        if request.quiescent && !request.tapestry {
            return Err("Quiescent weaving requires an eldritch tapestry".into());
        }
        if request.tapestry {
            let targets = request
                .tapestry_targets
                .as_ref()
                .ok_or("Tapestry target conditions must be supplied")?;
            let count = u32::try_from(targets.recipients.len()).map_err(|_| "Too many targets")?;
            if count == 0 {
                return Err("Select at least one tapestry target".into());
            }
            material_cost_gp = Some(
                spell
                    .profile
                    .material_cost_gp
                    .ok_or("Material cost data unavailable for tapestry spell")?
                    .checked_mul(count)
                    .ok_or("Material cost overflow")?,
            );
            xp_cost = Some(
                spell
                    .profile
                    .xp_cost
                    .ok_or("XP cost data unavailable for tapestry spell")?
                    .checked_mul(count)
                    .ok_or("XP cost overflow")?,
            );
            let (level, magic) = levels
                .iter()
                .find_map(|l| {
                    class_magic(&self.pack, &l.class_id)
                        .filter(|m| m.tapestry_from_level.is_some_and(|n| l.level >= n))
                        .map(|m| (l.level, m))
                })
                .ok_or("Eldritch tapestry is not acquired")?;
            if effective > level as i32
                || spell.profile.harmless_or_willing != Some(true)
                || spell.profile.targets_creatures != Some(true)
                || matches!(range, SpellRange::Personal | SpellRange::Unknown)
            {
                return Err("Tapestry requires a verified harmless/willing creature spell, nonpersonal range, and effective level within War Weaver level".into());
            }
            if magic.enlarged_tapestry_level.is_some_and(|n| level >= n) {
                range = match range {
                    SpellRange::Touch => SpellRange::Close,
                    SpellRange::Close => SpellRange::Medium,
                    SpellRange::Medium => SpellRange::Long,
                    r => r,
                };
            }
            if request.quiescent {
                let limit = level.saturating_sub(1);
                if request.stored_spells >= limit {
                    return Err("Quiescent weaving storage is full or not acquired".into());
                }
                capacity = Some(limit);
            }
        }
        let damage = spell
            .profile
            .damage
            .as_ref()
            .map(|p| {
                crate::class_actions::damage_range(
                    p,
                    caster_level,
                    warmage_edge.ok_or("Warmage Edge applicability unavailable")?,
                    &request.metamagic,
                )
            })
            .transpose()?;
        let range_feet = crate::targeting::range_feet(
            range,
            caster_level,
            range_multiplier,
            spell.profile.fixed_range_feet,
        )?;
        if request.tapestry {
            let resolved = self.tapestry_targets(
                draft,
                &spell.name,
                request
                    .tapestry_targets
                    .as_ref()
                    .ok_or("Missing recipients")?,
                range,
                range_feet,
                request.quiescent,
            )?;
            if !resolved.rejected.is_empty() {
                return Err(resolved.rejected.join("; "));
            }
        }
        let cloaked = (request.target_denied_dex
            || request
                .target
                .as_ref()
                .is_some_and(|t| crate::feature_resolution::feint_applies(draft, t)))
            && class_magic(&self.pack, &c.id).is_some_and(|m| m.cloaked_casting);
        let own_level = draft.level_in_class(&c.id);
        let cloaked_dc = if cloaked && own_level >= 2 {
            if own_level >= 14 {
                2
            } else {
                1
            }
        } else {
            0
        };
        let spontaneous_delay = c.casting_type == CastingType::Spontaneous
            && (heightening > 0
                || applied.iter().any(|r| {
                    Some(*r) != request.divine_metamagic
                        && Some(*r) != request.instant_metamagic
                        && !request.sudden.contains(r)
                }));
        let mut feature_costs = BTreeMap::new();
        if request.quick_cast {
            let quick =
                crate::class_features::resolve_feature_values(&self.pack, draft, &snapshot.stats)?
                    .into_iter()
                    .find(|f| {
                        f.class_id == c.id && f.resolution == Some(FeatureResolution::QuickCast)
                    })
                    .ok_or("Quick Cast is not acquired on this casting track")?;
            let key = format!("{}: {}", quick.class_id, quick.name);
            let capacity = quick.daily_uses.ok_or("Quick Cast allowance unavailable")?;
            if draft
                .class_play
                .resources
                .daily
                .get(&key)
                .copied()
                .unwrap_or(0)
                >= capacity
            {
                return Err("No Quick Cast uses remain".into());
            }
            if !matches!(
                spell.profile.casting_time,
                Some(
                    SpellCastingTime::Standard
                        | SpellCastingTime::Swift
                        | SpellCastingTime::Immediate
                )
            ) {
                return Err(
                    "Quick Cast requires a known casting time of one standard action or less"
                        .into(),
                );
            }
            if spontaneous_delay {
                return Err("Quick Cast cannot shorten spontaneous metamagic that increases the casting time".into());
            }
            feature_costs.insert(key, 1);
        }
        Ok(CastResolution {
            action_cost: if request.quick_cast || applied.contains(&SpellRule::Quicken) {Some(pcgen_core::actions::ActionCost::Swift)} else {match spell.profile.casting_time {
                Some(SpellCastingTime::Standard) if spontaneous_delay => Some(pcgen_core::actions::ActionCost::FullRound),
                Some(SpellCastingTime::Standard) => Some(pcgen_core::actions::ActionCost::Standard),
                Some(SpellCastingTime::FullRound) if !spontaneous_delay => Some(pcgen_core::actions::ActionCost::FullRound),
                Some(SpellCastingTime::Swift) if !spontaneous_delay => Some(pcgen_core::actions::ActionCost::Swift),
                _ => None,
            }},
            feature_costs,
            penetration_bonus:if cloaked && own_level>=8 {2}else{0},
            bypass_sr:cloaked && own_level>=20,
            damage,
            area_multiplier:if request.metamagic.contains(&SpellRule::Widen){2}else{1},
            instant_uses_remaining:crate::class_features::instant_uses(&self.pack,draft).saturating_sub(request.instant_spent.saturating_add(u32::from(request.instant_metamagic.is_some()))),
            warmage_edge, arcane_failure_percent, material_cost_gp, xp_cost,
            effect_timing: if request.quiescent {"Dormant until release; duration starts on release. A move action releases every stored spell in casting order; targets must be legal and in range at release."}else{"Effect starts when casting completes"}.into(),
            applied_metamagic: request.metamagic.clone(),
            spell: spell.name.clone(),
            class_id: c.id.clone(),
            power: access.power,
            base_level: access.level,
            effective_level: effective,
            required_slot_level: slot,
            paid_slot_level,
            paid_slots,
            caster_level,
            save_dc: 10 + effective + dc_modifier + cloaked_dc + crate::item_casting::dc_bonus(&state, &spell.school, &spell.name, &c.id),
            turning_cost,
            turning_remaining: turning_total - request.turning_spent - turning_cost,
            duration,
            range,
            range_multiplier,
            range_feet,
            tapestry: request.tapestry,
            quiescent_capacity: capacity,
            adjudications: if request.earth && request.versatile {
                vec!["Earth Spell uses the replaced higher-level slot; qualifying by this cast is conditional on that interpretation".into()]
            } else {
                Vec::new()
            },
            casting_time_note: if request.quick_cast { "Swift action (Quick Cast)".into() } else if applied.contains(&SpellRule::Quicken) { "Swift action (Quicken Spell)".into() } else if spontaneous_delay
            {
                "Spontaneous metamagic: standard-action spells become full-round; longer spells add a full-round action".into()
            } else {
                "Use the spell's normal casting time".into()
            },
        })
    }
}

#[derive(Debug, Serialize)]
pub struct EntryRequirement {
    pub requirement: String,
    pub satisfied: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct EntryReport {
    pub class_id: String,
    pub qualified: bool,
    pub requirements_met: bool,
    pub conditional: bool,
    pub checks: Vec<EntryRequirement>,
    pub casting_proof: Option<CastResolution>,
}
impl Engine {
    /// Evaluate against the character BEFORE acquiring the requested class.
    /// A cast proof can satisfy casting capability; it never creates spell slots.
    pub fn class_entry(
        &self,
        draft: &CharacterDraft,
        target: &str,
        proof: Option<&CastRequest>,
    ) -> Result<EntryReport, String> {
        if draft
            .advancement
            .iter()
            .flat_map(|f| &f.lanes)
            .any(|l| l.class_id.eq_ignore_ascii_case(target))
        {
            return Err(
                "Entry checks require the character before acquiring the target class".into(),
            );
        }
        let entity = self
            .pack
            .entities
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(target) && e.metadata.class_magic.is_some())
            .ok_or("Class has no native entry model")?;
        let magic = entity.metadata.class_magic.as_ref().unwrap();
        let state = crate::apply_effects(&self.pack, draft);
        if !state.diagnostics.is_empty() {
            return Err(state.diagnostics.join("; "));
        }
        let cast = proof.map(|r| self.cast_preview(draft, r)).transpose()?;
        let mut checks = Vec::new();
        for pre in &entity.requirements {
            let satisfied = if let (pcgen_core::Predicate::SpellType { specs, count }, Some(cast)) =
                (pre, cast.as_ref())
            {
                if *count <= 1
                    && specs.iter().any(|s| {
                        cast.effective_level >= s.min_level as i32
                            && s.spell_type.as_deref().is_none_or(|p| {
                                p.eq_ignore_ascii_case("any")
                                    || p.eq_ignore_ascii_case(&format!("{:?}", cast.power))
                            })
                    })
                {
                    Some(true)
                } else {
                    crate::effects::predicate_result(Some(pre), draft, &state)
                }
            } else {
                crate::effects::predicate_result(Some(pre), draft, &state)
            };
            checks.push(EntryRequirement {
                requirement: format!("{pre:?}"),
                satisfied,
            });
            if let pcgen_core::Predicate::SkillRanks { specs, .. } = pre {
                for spec in specs {
                    let ranks = draft
                        .skills
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(&spec.name))
                        .map(|(_, n)| *n)
                        .unwrap_or(0.0);
                    let class_skill = state.class_skills.contains(&spec.name.to_ascii_lowercase());
                    let cap = pcgen_core::skill_rules::max_rank(draft.total_level(), class_skill);
                    checks.push(EntryRequirement {
                        requirement: format!(
                            "{} ranks within the pre-entry maximum {cap}",
                            spec.name
                        ),
                        satisfied: Some(ranks.is_finite() && ranks >= 0.0 && ranks <= cap),
                    });
                }
            }
        }
        let snapshot = self.evaluate(draft);
        if !snapshot.is_available() {
            return Err(snapshot.diagnostics.join("; "));
        }
        for school in &magic.entry_forbidden_schools {
            checks.push(EntryRequirement {requirement:format!("{school} must not be a prohibited school"),satisfied:Some(!snapshot.prohibited_spells.iter().any(|p|matches!(&p.target,pcgen_core::ProhibitionTarget::School(s) if s.eq_ignore_ascii_case(school))))});
        }
        if let Some(incarnum) = &magic.incarnum {
            if incarnum.entry_essentia > 0 {
                let pool =
                    self.incarnum_snapshot(draft, &crate::incarnum::IncarnumLoadout::default());
                checks.push(EntryRequirement {
                    requirement: format!(
                        "Essentia pool at least {} before class entry",
                        incarnum.entry_essentia
                    ),
                    satisfied: Some(pool.errors.is_empty() && pool.pool >= incarnum.entry_essentia),
                });
            }
        }
        for name in &magic.entry_states {
            checks.push(EntryRequirement {
                requirement: name.clone(),
                satisfied: Some(draft.active_states.iter().any(|s| s == name)),
            });
        }
        if !magic.entry_spells_any.is_empty() {
            let tracks = casting_levels(&self.pack, draft, &mut Vec::new());
            let mut known = false;
            for track in tracks.keys() {
                for spell in self.spell_catalog(draft, track)? {
                    if spell.known
                        && magic
                            .entry_spells_any
                            .iter()
                            .any(|s| s.eq_ignore_ascii_case(&spell.spell))
                        && self
                            .cast_preview(
                                draft,
                                &CastRequest {
                                    class_id: track.clone(),
                                    spell: spell.spell,
                                    ..Default::default()
                                },
                            )
                            .is_ok()
                    {
                        known = true;
                    }
                }
            }
            checks.push(EntryRequirement {
                requirement: format!("Can cast any of: {}", magic.entry_spells_any.join(", ")),
                satisfied: Some(known),
            });
        }
        let requirements_met = checks.iter().all(|c| c.satisfied == Some(true));
        let conditional = cast.as_ref().is_some_and(|c| !c.adjudications.is_empty());
        Ok(EntryReport {
            class_id: entity.name.clone(),
            qualified: requirements_met && !conditional,
            requirements_met,
            conditional,
            checks,
            casting_proof: cast,
        })
    }
}

/// Native school choices are canonical restrictions, independent of Java tags.
pub(crate) fn native_prohibited_schools(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for level in crate::engine::effective_class_levels(pack, draft) {
        if let Some(choice) = class_magic(pack, &level.class_id)
            .and_then(|m| m.school_choice.as_ref())
            .filter(|c| level.level >= c.at_level)
        {
            let selected = draft.choices.get(&choice.choice_key).ok_or_else(|| {
                format!("Required school choice is missing: {}", choice.choice_key)
            })?;
            let canonical = choice
                .allowed
                .iter()
                .find(|s| s.eq_ignore_ascii_case(selected))
                .ok_or_else(|| {
                    format!(
                        "Invalid prohibited school for {}: {selected}",
                        choice.choice_key
                    )
                })?;
            if result.contains(canonical) {
                return Err("An additional prohibited school must be a new school".into());
            }
            result.push(canonical.clone());
        }
    }
    Ok(result)
}

pub(crate) fn serde_rule_name(rule: SpellRule) -> &'static str {
    match rule {
        SpellRule::Empower => "empower",
        SpellRule::Enlarge => "enlarge",
        SpellRule::Widen => "widen",
        SpellRule::Maximize => "maximize",
        SpellRule::Extend => "extend",
        SpellRule::Silent => "silent",
        SpellRule::Still => "still",
        SpellRule::Quicken => "quicken",
        _ => "unsupported",
    }
}

pub(crate) fn policy_dc_ability<'a>(pack: &'a CompiledRulePack, class: &str) -> Option<&'a String> {
    class_magic(pack, class)?.save_dc_ability.as_ref()
}
