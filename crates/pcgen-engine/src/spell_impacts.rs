//! Non-energy touch effects resolve through the ordinary frozen spell event.
use crate::Engine;
use pcgen_core::{class_play::*, combat::*};
pub(crate) fn formula(text: &str, cl: i32) -> Result<i32, String> {
    pcgen_formula::Formula::parse(text)
        .and_then(|f| {
            f.eval_i32(&std::collections::HashMap::from([(
                "CASTERLEVEL".into(),
                cl,
            )]))
        })
        .map_err(|e| format!("Spell effect formula unavailable: {e}"))
}
fn effect(
    s: &mut ClassPlay,
    cast: &CastEffect,
    casting: &str,
    target: &str,
    condition: SpellCondition,
    seconds: u64,
) {
    let key = format!("{casting}:{target}:{}", s.combat_state.effects.len());
    s.combat_state.effects.insert(
        key,
        ActiveSpell {
            casting: casting.into(),
            name: cast.spell.clone(),
            target: target.into(),
            caster_level: cast.caster_level,
            expires: s.encounter.seconds + seconds * u64::from(cast.duration_multiplier),
            condition,
        },
    );
}
pub(crate) fn score_damage(
    s: &mut ClassPlay,
    target: &str,
    key: &str,
    amount: u32,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    let current = crate::necromancy::score(s, target, key)?;
    let n = amount.min(current as u32);
    *s.necromancy
        .actors
        .get_mut(target)
        .ok_or("Record ability scores before ability damage")?
        .damage
        .entry(key.into())
        .or_default() += n;
    if current == n as i32 {
        s.encounter.conditions.push(EncounterCondition {
            source: "Ability Score Loss".into(),
            target: target.into(),
            name: "helpless".into(),
            remaining_seconds: None,
            aura_radius_feet: None,
        });
    }
    facts.push(format!("{target}: {key} ability damage {n}"));
    Ok(())
}
impl Engine {
    pub(crate) fn resolve_touch_impact(
        &self,
        s: &mut ClassPlay,
        cast: &CastEffect,
        casting: &str,
        r: &DefenseRequest,
        saved: Option<bool>,
        damage: Option<i64>,
        victim_hp_before: i64,
        facts: &mut Vec<String>,
    ) -> Result<(), String> {
        let Some(impact) = &cast.impact else {
            return Ok(());
        };
        let target = &r.target;
        let die = |sides: u32| {
            r.effect_roll
                .filter(|n| (1..=sides).contains(n))
                .ok_or_else(|| format!("{} requires an effect d{sides} roll", cast.spell))
        };
        match impact {
            TouchImpact::Vampiric { temporary_seconds } => {
                let n = damage
                    .ok_or("Vampiric damage unavailable")?
                    .min((victim_hp_before + 10).max(0));
                crate::combat::apply_temporary(
                    s,
                    "self",
                    "Vampiric Touch",
                    n,
                    s.encounter.seconds + u64::from(*temporary_seconds),
                    facts,
                )?;
            }
            TouchImpact::Chill { strength_damage } => {
                if s.actors[target].body == TargetBody::Undead {
                    if saved == Some(false) {
                        let seconds = u64::from(die(4)? + cast.caster_level.max(0) as u32) * 6;
                        effect(s, cast, casting, target, SpellCondition::Panicked, seconds);
                        facts.push(format!(
                            "{target}: undead panicked for {} rounds (specific Chill Touch rule)",
                            seconds / 6
                        ));
                    }
                } else if saved == Some(false)
                    && !s.actors[target].dead
                    && !crate::combat::immune(s, target, BodyImmunity::PhysicalAbilityDamage)
                {
                    score_damage(s, target, "STR", *strength_damage, facts)?;
                }
            }
            TouchImpact::Fatigue { seconds } => {
                if saved == Some(false) && !crate::combat::condition(s, target, "fatigued") {
                    for key in ["STR", "DEX"] {
                        crate::necromancy::score(s, target, key)?;
                    }
                    effect(
                        s,
                        cast,
                        casting,
                        target,
                        SpellCondition::Fatigued,
                        u64::try_from(formula(seconds, cast.caster_level)?)
                            .map_err(|_| "Negative fatigue duration")?,
                    );
                    facts.push(format!(
                        "{target}: fatigued; this spell does not cause exhaustion"
                    ));
                }
            }
            TouchImpact::Idiocy { seconds } => {
                for key in ["INT", "WIS", "CHA"] {
                    crate::necromancy::score(s, target, key)?;
                }
                let amount = die(6)?;
                effect(
                    s,
                    cast,
                    casting,
                    target,
                    SpellCondition::AbilityPenalty {
                        abilities: vec!["INT".into(), "WIS".into(), "CHA".into()],
                        amount,
                        floor: 1,
                    },
                    u64::try_from(formula(seconds, cast.caster_level)?)
                        .map_err(|_| "Negative idiocy duration")?,
                );
                facts.push(format!(
                    "{target}: INT/WIS/CHA penalty {amount}, minimum score 1; no save"
                ));
            }
            TouchImpact::Ghoul { radius_feet } => {
                if saved == Some(false)
                    && !crate::combat::immune(s, target, BodyImmunity::Paralysis)
                {
                    crate::necromancy::score(s, target, "DEX")?;
                    crate::necromancy::score(s, target, "STR")?;
                    let seconds = u64::from(die(6)? + 2) * 6;
                    effect(s, cast, casting, target, SpellCondition::Paralyzed, seconds);
                    s.combat_state.stench.insert(
                        format!("{casting}:{target}"),
                        StenchAura {
                            casting: casting.into(),
                            target: target.clone(),
                            caster_level: cast.caster_level,
                            dc: cast.save_dc,
                            expires: s.encounter.seconds
                                + seconds * u64::from(cast.duration_multiplier),
                            radius_feet: *radius_feet,
                        },
                    );
                    crate::combat::refresh_stench(s);
                    facts.push(format!("{target}: paralyzed for {} rounds; stench Fortitude saves queued for exposed living actors",seconds/6));
                }
            }
            TouchImpact::Dispel { maximum_bonus } => {
                let mut candidates: Vec<_> = s
                    .combat_state
                    .effects
                    .values()
                    .filter(|e| e.target == *target && e.expires > s.encounter.seconds)
                    .map(|e| (e.caster_level, e.casting.clone()))
                    .collect();
                candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                candidates.dedup();
                for (cl, id) in candidates {
                    let roll = r
                        .dispel_rolls
                        .get(&id)
                        .copied()
                        .filter(|n| (1..=20).contains(n))
                        .ok_or_else(|| format!("Dispel check d20 required for {id} (CL {cl})"))?;
                    let total = roll as i32 + cast.caster_level.min(*maximum_bonus as i32);
                    let dc = 11 + cl;
                    facts.push(format!(
                        "{id}: dispel {total} vs {dc}; natural 1/20 do not determine this check"
                    ));
                    if total >= dc {
                        s.combat_state
                            .effects
                            .retain(|_, e| !(e.casting == id && e.target == *target));
                        s.combat_state
                            .stench
                            .retain(|_, a| !(a.casting == id && a.target == *target));
                        crate::combat::refresh_stench(s);
                        facts.push(format!(
                            "{id}: one spell on {target} dispelled; carried items unaffected"
                        ));
                        break;
                    }
                }
            }
            TouchImpact::Teleport { feet } => {
                if saved != Some(false) {
                    return Ok(());
                }
                if crate::combat::condition(s, target, "dimensional_anchor") {
                    facts.push("Teleportation blocked by dimensional anchor".into());
                    return Ok(());
                }
                let (x, y) = r.destination.ok_or("Choose a visible destination square")?;
                let old = s.actors.get(target).ok_or("Unknown target")?.clone();
                let mut destination = old.clone();
                destination.x = x;
                destination.y = y;
                destination.path = r
                    .destination_path
                    .ok_or("Record destination visibility before teleportation")?;
                if x.unsigned_abs() > 10000
                    || y.unsigned_abs() > 10000
                    || crate::targeting::distance(&old, &destination)
                        > u32::try_from(formula(feet, cast.caster_level)?)
                            .map_err(|_| "Negative teleport distance")?
                {
                    return Err("Destination exceeds Dimension Hop range".into());
                }
                if !matches!(
                    destination.path,
                    TargetPath::Clear | TargetPath::TransparentSolid
                ) {
                    return Err("Destination line of sight is not established in the scene".into());
                }
                if s.actors.iter().any(|(id, a)| {
                    id != target
                        && !a.dead
                        && (i64::from(x) < i64::from(a.x) + i64::from(a.space)
                            && i64::from(a.x) < i64::from(x) + i64::from(old.space))
                        && (i64::from(y) < i64::from(a.y) + i64::from(a.space)
                            && i64::from(a.y) < i64::from(y) + i64::from(old.space))
                }) {
                    return Err("Teleport destination is occupied".into());
                }
                s.actors.insert(target.clone(), destination);
                crate::feature_resolution::refresh_aura_conditions(s);
                crate::combat::refresh_stench(s);
                facts.push(format!("{target}: teleported to ({x}, {y})"));
            }
        }
        Ok(())
    }
}
