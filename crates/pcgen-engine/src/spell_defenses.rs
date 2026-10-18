//! Atomic per-event spell defenses and encounter HP; casting variables are frozen.
use crate::{class_play::PlayResult, encounter_health::apply_damage, CharacterDraft, Engine};
use pcgen_core::{class_play::*, magic::*};
use std::collections::BTreeMap;
impl Engine {
    pub fn record_spell_effect(
        &self,
        d: &CharacterDraft,
        state: &mut ClassPlay,
        spell: &str,
        cl: i32,
        dc: i32,
        damage: Option<&crate::class_actions::DamageResolution>,
        use_feats: bool,
        metamagic: &[SpellRule],
    ) -> Result<String, String> {
        let def = self
            .pack
            .spells
            .values()
            .find(|s| s.name.eq_ignore_ascii_case(spell) || s.id.eq_ignore_ascii_case(spell))
            .ok_or("Unknown spell")?;
        let has = |name: &str| {
            self.pack.entities.iter().any(|e| {
                e.name.eq_ignore_ascii_case(name) && crate::effects::ability_matches_draft(d, e)
            })
        };
        let penetration_bonus = if use_feats {
            2 * i32::from(has("Spell Penetration"))
                + 2 * i32::from(has("Greater Spell Penetration"))
        } else {
            0
        };
        let rounds = if let Some(formula) = &def.profile.recurring_rounds {
            let bindings = std::collections::HashMap::from([("CASTERLEVEL".into(), cl)]);
            let base = pcgen_formula::Formula::parse(formula)
                .and_then(|f| f.eval_i32(&bindings))
                .map_err(|e| format!("Recurring duration unavailable: {e}"))?;
            let n = if metamagic.contains(&SpellRule::Persistent) {
                14400
            } else {
                base.checked_mul(if metamagic.contains(&SpellRule::Extend) {
                    2
                } else {
                    1
                })
                .ok_or("Duration overflow")?
            };
            u32::try_from(n)
                .ok()
                .filter(|n| *n > 0)
                .ok_or("Invalid recurring duration")?
        } else {
            1
        };
        let key = format!("cast-{}-{}", state.revision, state.cast_effects.len());
        state.cast_effects.insert(
            key.clone(),
            CastEffect {
                source_class: None,
                requires_touch: false,
                contact_sequence: 0,
                duration_multiplier: if metamagic.contains(&SpellRule::Extend) {
                    2
                } else {
                    1
                },
                impact: def.profile.impact.clone(),
                damage_kind: def.profile.damage_kind.or(def
                    .profile
                    .energy
                    .map(pcgen_core::combat::DamageKind::Energy)),
                targeting: def.profile.targeting.clone(),
                hit_point_damage: def.profile.hit_point_damage,
                undead_immunity: if def.school.eq_ignore_ascii_case("Enchantment")
                    || def.descriptors.iter().any(|v| {
                        ["Mind-Affecting", "Death"]
                            .iter()
                            .any(|k| v.eq_ignore_ascii_case(k))
                    }) {
                    Some(true)
                } else if def
                    .profile
                    .defense
                    .as_ref()
                    .is_some_and(|r| r.save == SaveKind::Fortitude && !r.harmless)
                {
                    def.profile.affects_objects.map(|yes| !yes)
                } else {
                    Some(false)
                },
                spell: def.name.clone(),
                bypass_sr: false,
                energy: def.profile.energy,
                round: 0,
                rounds,
                ended: false,
                bound_target: None,
                sr_results: BTreeMap::new(),
                caster_level: cl,
                save_dc: dc,
                penetration_bonus,
                defense: def.profile.defense.clone(),
                shared_damage: damage.is_some_and(|d| d.area),
                shared_damage_roll: None,
                damage_bounds: damage
                    .filter(|d| d.packets.len() == 1)
                    .map(|d| (d.packets[0].minimum, d.packets[0].maximum)),
                resolved: BTreeMap::new(),
            },
        );
        Ok(key)
    }
    pub fn save_spell_defenses(
        &self,
        d: &CharacterDraft,
        id: &str,
        defense: SpellDefenses,
    ) -> Result<PlayResult, String> {
        if !d.class_play.actors.contains_key(id) {
            return Err("Register this scene actor first".into());
        }
        if [defense.fortitude, defense.reflex, defense.will]
            .iter()
            .any(|v| v.unsigned_abs() > 10000)
            || defense.spell_resistance > 10000
        {
            return Err("Defense values outside supported range".into());
        }
        let mut state = d.class_play.clone();
        state.defenses.insert(id.into(), defense);
        state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        Ok(PlayResult {
            state,
            success: true,
            facts: vec![format!("Saved defenses for {id}")],
        })
    }
    pub fn resolve_spell_defenses(
        &self,
        d: &CharacterDraft,
        casting: &str,
        r: &DefenseRequest,
    ) -> Result<PlayResult, String> {
        if d.class_play
            .cast_effects
            .get(casting)
            .is_some_and(|c| c.requires_touch)
        {
            return Err("Touch delivery requires the held-charge attack form; it cannot bypass contact or discharge".into());
        }
        self.resolve_delivered_spell(d, casting, r, false)
    }
    pub(crate) fn resolve_delivered_spell(
        &self,
        d: &CharacterDraft,
        casting: &str,
        r: &DefenseRequest,
        critical: bool,
    ) -> Result<PlayResult, String> {
        let mut cast = d
            .class_play
            .cast_effects
            .get(casting)
            .ok_or("Unknown casting")?
            .clone();
        if critical {
            cast.damage_bounds = cast.damage_bounds.map(|(a, b)| (a * 2, b * 2));
            if let Some(pcgen_core::combat::TouchImpact::Chill { strength_damage }) =
                &mut cast.impact
            {
                *strength_damage *= 2;
            }
        }
        if cast.ended || cast.round >= cast.rounds {
            return Err("This casting has ended".into());
        }
        if cast.bound_target.as_ref().is_some_and(|t| t != &r.target) {
            return Err("Recurring damage remains attached to the original hit target".into());
        }
        let event_key = if cast.requires_touch {
            format!("touch-{}:{}", cast.contact_sequence, r.target)
        } else {
            format!("{}:{}", cast.round, r.target)
        };
        if cast.resolved.contains_key(&event_key) {
            return Err("This target already resolved this round; advance the effect before its next damage tick".into());
        }
        if !d.class_play.actors.contains_key(&r.target) {
            return Err("Unknown scene actor".into());
        }
        let actor = &d.class_play.actors[&r.target];
        let chill_undead = matches!(
            cast.impact,
            Some(pcgen_core::combat::TouchImpact::Chill { .. })
        ) && actor.body == TargetBody::Undead;
        if chill_undead {
            cast.defense = Some(SpellDefenseRule {
                save: SaveKind::Will,
                on_save: SaveEffect::Negates,
                harmless: false,
                spell_resistance: true,
            });
            cast.damage_bounds = None;
            cast.hit_point_damage = Some(false);
            cast.undead_immunity = Some(false);
        }
        let ineligible = cast.targeting.as_ref().is_some_and(|t| {
            t.living_only && !actor.living()
                || !t.creature_types.is_empty()
                    && !t
                        .creature_types
                        .iter()
                        .any(|kind| kind.eq_ignore_ascii_case(&actor.creature_type))
        });
        let rule = cast
            .defense
            .as_ref()
            .ok_or("Structured saving throw / SR rules unavailable for this spell")?;
        let defense = d
            .class_play
            .defenses
            .get(&r.target)
            .ok_or("Target defenses unavailable; enter known values (0 means no SR)")?;
        for roll in [r.sr_roll, r.save_roll, r.massive_roll]
            .into_iter()
            .flatten()
        {
            if !(1..=20).contains(&roll) {
                return Err("D20 must be 1–20".into());
            }
        }
        let mut facts = vec![format!(
            "{} / {} / {}: spell defenses and HP",
            casting, cast.spell, r.target
        )];
        let protections = d.class_play.protections.get(&r.target).ok_or(
            "Target protections unavailable; enter immunities, resistance and evasion explicitly",
        )?;
        crate::necromancy::require_actor(&d.class_play, &r.target)?;
        let actor = &d.class_play.actors[&r.target];
        let undead_immune = if actor.body == TargetBody::Undead {
            cast.undead_immunity.ok_or("Undead immunity unavailable: classify whether this Fortitude effect affects objects before resolving it")?
        } else {
            false
        };
        let mut sr_checked = None;
        let mut resisted = !cast.bypass_sr
            && rule.spell_resistance
            && protections.spell_immunity
            && r.target != "self";
        if resisted {
            facts.push("SR-style spell immunity prevents this spell".into());
        }
        if ineligible
            || (matches!(
                cast.impact,
                Some(pcgen_core::combat::TouchImpact::Idiocy { .. })
            ) && (crate::combat::immune(&d.class_play, &r.target, BodyImmunity::MindAffecting)
                || d.class_play
                    .encounter
                    .combat
                    .get(&r.target)
                    .is_some_and(|c| c.intelligence == 0)))
            || (matches!(
                cast.impact,
                Some(pcgen_core::combat::TouchImpact::Fatigue { .. })
            ) && (crate::combat::immune(&d.class_play, &r.target, BodyImmunity::Fatigue)
                || crate::combat::condition(&d.class_play, &r.target, "fatigued")))
        {
            resisted = true;
            facts.push("Target type or condition immunity prevents this effect".into());
        }
        if undead_immune {
            resisted = true;
            facts.push("Undead type immunity prevents this effect".into());
        }
        if !cast.bypass_sr
            && !resisted
            && !cast
                .energy
                .is_some_and(|e| protections.immunity.contains(&e))
            && rule.spell_resistance
            && defense.spell_resistance > 0
            && r.target != "self"
            && !r.sr_lowered
        {
            if rule.harmless {
                resisted = true;
                facts.push("Harmless spell: SR must be voluntarily lowered using the required standard action".into());
            } else if let Some(result) = cast.sr_results.get(&r.target) {
                resisted = *result;
                facts.push("Reused this casting's original SR check".into());
            } else {
                let die = r.sr_roll.ok_or("SR check d20 required")?;
                let power = if cast.source_class.is_some()
                    && d.class_play.tactics.wounded_in_melee.contains(&r.target)
                {
                    self.class_feature_values(d)?
                        .into_iter()
                        .find(|f| f.class_id == "Duskblade" && f.name == "Spell Power")
                        .map(|f| {
                            f.values
                                .get("penetration_bonus")
                                .copied()
                                .ok_or("Spell Power bonus unavailable")
                        })
                        .transpose()?
                        .unwrap_or(0)
                } else {
                    0
                };
                let total = i64::from(die)
                    + i64::from(cast.caster_level)
                    + i64::from(cast.penetration_bonus + power);
                if power > 0 {
                    facts.push(format!(
                        "Spell Power +{power}: target injured by your melee weapon this encounter"
                    ));
                }
                resisted = total < i64::from(defense.spell_resistance);
                sr_checked = Some(resisted);
                facts.push(format!("SR: {die} + CL {} + penetration {} = {total} against {}; natural 1/20 have no automatic result",cast.caster_level,cast.penetration_bonus,defense.spell_resistance));
            }
        }
        let saved = if resisted
            || cast
                .energy
                .is_some_and(|e| protections.immunity.contains(&e))
            || rule.save == SaveKind::None
        {
            None
        } else if r.waive_save {
            facts.push("Saving throw voluntarily waived".into());
            Some(false)
        } else {
            let die = r
                .save_roll
                .ok_or("Saving throw d20 required (or explicitly waive)")?;
            let bonus = match rule.save {
                SaveKind::Fortitude => defense.fortitude,
                SaveKind::Reflex => defense.reflex,
                SaveKind::Will => defense.will,
                SaveKind::None => unreachable!(),
            } - if crate::combat::condition(&d.class_play, &r.target, "sickened") {
                2
            } else {
                0
            } + crate::necromancy::save_delta(
                &d.class_play,
                &r.target,
                match rule.save {
                    SaveKind::Fortitude => "CON",
                    SaveKind::Reflex => "DEX",
                    _ => "WIS",
                },
            );
            let success = die == 20
                || (die != 1 && i64::from(die) + i64::from(bonus) >= i64::from(cast.save_dc));
            facts.push(format!(
                "{:?}: d20 {die} + {bonus} against DC {}: {}",
                rule.save,
                cast.save_dc,
                if success { "saved" } else { "failed" }
            ));
            if die == 1 {
                facts.push("Natural 1: check exposed carried equipment separately if the spell can damage objects".into());
            }
            Some(success)
        };
        let evasion = rule.save == SaveKind::Reflex
            && rule.on_save == SaveEffect::Half
            && crate::encounter_health::evasion_applies(actor, protections);
        let immune = cast
            .energy
            .is_some_and(|e| protections.immunity.contains(&e));
        let effect = if immune {
            "immune"
        } else if resisted {
            "resisted"
        } else if saved == Some(true) {
            match rule.on_save {
                SaveEffect::Negates => "negated",
                SaveEffect::Half if evasion && protections.evasion != Evasion::None => "evaded",
                SaveEffect::Half => "half",
                SaveEffect::None => "full",
            }
        } else if evasion && protections.evasion == Evasion::Improved && !r.waive_save {
            "half"
        } else {
            "full"
        };
        if cast.shared_damage
            && r.damage_roll
                .zip(cast.shared_damage_roll)
                .is_some_and(|(a, b)| a != b)
        {
            return Err(
                "Area damage is already rolled for this casting; all targets use the same roll"
                    .into(),
            );
        }
        let rolled = r
            .damage_roll
            .or(cast.shared_damage_roll)
            .or_else(|| cast.damage_bounds.filter(|(a, b)| a == b).map(|(a, _)| a));
        let damage = if let Some(rolled) = rolled {
            let (min, max) = cast
                .damage_bounds
                .ok_or("A single damage packet is not available; do not enter a damage total")?;
            if rolled < min || rolled > max {
                return Err(format!(
                    "Damage total must be within the resolved {min}–{max} range"
                ));
            }
            Some(match effect {
                "resisted" | "negated" | "immune" | "evaded" => 0,
                "half" => rolled / 2,
                _ => rolled,
            })
        } else {
            None
        };
        let damage = if cast.damage_bounds.is_some() {
            let kind = cast
                .damage_kind
                .ok_or("Damage classification unavailable; HP cannot be resolved")?;
            let n = if matches!(effect, "resisted" | "negated" | "immune" | "evaded") {
                0
            } else {
                damage.ok_or("Damage roll required before committing this event")?
            };
            let n = match kind {
                pcgen_core::combat::DamageKind::Energy(energy) => {
                    crate::encounter_health::energy_after_resistance(
                        protections,
                        energy,
                        n,
                        &mut facts,
                    )
                }
                pcgen_core::combat::DamageKind::Untyped
                | pcgen_core::combat::DamageKind::NegativeEnergy => n,
            };
            Some(n)
        } else {
            if cast.hit_point_damage != Some(false) && !chill_undead {
                return Err(
                    "Damage packet unavailable; this event cannot be partially committed".into(),
                );
            }
            None
        };
        let mut state = d.class_play.clone();
        let victim_hp_before = if matches!(
            cast.impact,
            Some(pcgen_core::combat::TouchImpact::Vampiric { .. })
        ) {
            state
                .health
                .get(&r.target)
                .ok_or("Target HP unavailable for Vampiric Touch")?
                .current
        } else {
            0
        };
        if let Some(n) = damage.filter(|n| *n > 0) {
            apply_damage(
                &mut state,
                &r.target,
                n,
                r.massive_roll,
                defense.fortitude,
                &mut facts,
            )?;
            if r.target == "self" && n > 0 {
                for (id, spell) in &mut state.spells {
                    if spell.ended.is_none()
                        && spell.concentration
                        && spell.controller == SpellController::Incantatrix
                    {
                        if state.actors["self"].unconscious || state.actors["self"].dead {
                            spell.controller = SpellController::None;
                            spell.awaiting_reassertion = true;
                            facts.push(format!("{id}: incapacitated; concentration lapsed, original caster may reassert"));
                        } else {
                            let damage_dc = if cast.rounds > 1 { n / 2 } else { n };
                            let dc = 10 + damage_dc + i64::from(spell.spell_level);
                            let key = format!("{casting}:{}:{id}", cast.round);
                            state.concentration_checks.insert(
                                key.clone(),
                                ConcentrationCheck {
                                    spell: id.clone(),
                                    dc,
                                    success: None,
                                    fact: format!(
                                        "{id}: Concentration DC {dc}; {} damage {n}",
                                        if cast.rounds > 1 && !cast.requires_touch {
                                            "continuous"
                                        } else {
                                            "interrupting"
                                        }
                                    ),
                                },
                            );
                            facts.push(format!("Concentration check pending: {key}, DC {dc}"));
                        }
                    }
                }
            }
        }
        if !matches!(effect, "resisted" | "negated" | "immune" | "evaded") {
            self.resolve_touch_impact(
                &mut state,
                &cast,
                casting,
                r,
                saved,
                damage,
                victim_hp_before,
                &mut facts,
            )?;
        }
        facts.push(format!("Result: {effect}"));
        let outcome = DefenseOutcome {
            resisted,
            saved,
            damage,
            effect: effect.into(),
            facts: facts.clone(),
        };
        let changed = state
            .cast_effects
            .get_mut(casting)
            .ok_or("Casting disappeared")?;
        if let Some(result) = sr_checked {
            changed.sr_results.insert(r.target.clone(), result);
        }
        if cast.rounds > 1 && !cast.requires_touch {
            changed.bound_target = Some(r.target.clone());
        }
        if cast.shared_damage {
            state
                .cast_effects
                .get_mut(casting)
                .ok_or("Casting disappeared")?
                .shared_damage_roll = rolled;
        }
        state
            .cast_effects
            .get_mut(casting)
            .ok_or("Casting disappeared")?
            .resolved
            .insert(event_key, outcome);
        state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        Ok(PlayResult {
            state,
            success: true,
            facts,
        })
    }
}

impl Engine {
    pub fn advance_damage_effect(
        &self,
        d: &CharacterDraft,
        casting: &str,
        end: bool,
    ) -> Result<PlayResult, String> {
        let mut state = d.class_play.clone();
        let c = state
            .cast_effects
            .get_mut(casting)
            .ok_or("Unknown casting")?;
        if c.ended || c.rounds <= 1 {
            return Err("No active recurring effect".into());
        }
        if !end
            && !c
                .resolved
                .keys()
                .any(|key| key.starts_with(&format!("{}:", c.round)))
        {
            return Err("Resolve this round before advancing the damage effect".into());
        }
        if end {
            c.ended = true;
        } else {
            c.round += 1;
            c.shared_damage_roll = None;
            c.ended = c.round >= c.rounds;
        }
        let facts = vec![format!(
            "{}: round {}/{}; {}",
            c.spell,
            c.round + 1,
            c.rounds,
            if c.ended { "ended" } else { "active" }
        )];
        state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        Ok(PlayResult {
            state,
            success: true,
            facts,
        })
    }
}

impl Engine {
    pub fn resolve_damage_concentration(
        &self,
        d: &CharacterDraft,
        id: &str,
        roll: u32,
    ) -> Result<PlayResult, String> {
        if !(1..=20).contains(&roll) {
            return Err("Concentration d20 must be 1-20".into());
        }
        let check = d
            .class_play
            .concentration_checks
            .get(id)
            .ok_or("Unknown concentration check")?;
        if check.success.is_some() {
            return Err("Concentration check already resolved".into());
        }
        let spell = d
            .class_play
            .spells
            .get(&check.spell)
            .ok_or("Controlled spell unavailable")?;
        if spell.ended.is_some() || spell.controller != SpellController::Incantatrix {
            return Err("Concentration already ended".into());
        }
        let definition = self
            .skill("Concentration")
            .ok_or("Concentration definition unavailable")?;
        let mut input = d.clone();
        if !input
            .skills
            .keys()
            .any(|k| k.eq_ignore_ascii_case("Concentration"))
        {
            if definition.trained_only {
                return Err("Concentration cannot be used untrained".into());
            }
            // Materialize the RAW zero ranks, then use the normal stat/typed-bonus evaluator.
            input.skills.insert("Concentration".into(), 0.0);
        }
        let snapshot = self.evaluate(&input);
        if !snapshot.is_available() {
            return Err(snapshot.diagnostics.join("; "));
        }
        let bonus = snapshot
            .skills
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Concentration"))
            .map(|(_, v)| *v)
            .ok_or("Concentration modifier unavailable")?;
        let success = i64::from(roll) + i64::from(bonus) >= check.dc;
        let mut state = if success {
            d.class_play.clone()
        } else {
            self.spell_control(d, &check.spell, &ControlEvent::Lapse)?
                .state
        };
        let fact = format!(
            "{}; d20 {roll} + {bonus}: {} (no automatic natural 1/20 result)",
            check.fact,
            if success {
                "maintained"
            } else {
                "concentration lapsed"
            }
        );
        let c = state.concentration_checks.get_mut(id).unwrap();
        c.success = Some(success);
        c.fact = fact.clone();
        if success {
            state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        }
        Ok(PlayResult {
            state,
            success,
            facts: vec![fact],
        })
    }
}
