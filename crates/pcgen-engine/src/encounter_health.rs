//! Encounter HP transitions. No spell lookup, metamagic or HTML dependencies.
use crate::{class_play::PlayResult, CharacterDraft, Engine};
use pcgen_core::class_play::*;
pub(crate) fn apply_healing(
    state: &mut ClassPlay,
    target: &str,
    amount: i64,
    cap_half: bool,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    crate::necromancy::require_actor(&state, target)?;
    let actor = state.actors.get_mut(target).ok_or("Unknown target")?;
    if actor.dead {
        return Err("Healing cannot restore a dead or destroyed creature".into());
    }
    let hp = state
        .health
        .get_mut(target)
        .ok_or("Target HP unavailable")?;
    let was_dying = hp.current < 0;
    let cap = if cap_half { hp.maximum / 2 } else { hp.maximum };
    let healed = amount.min((cap - hp.current).max(0));
    hp.current = hp.current.checked_add(healed).ok_or("HP overflow")?;
    if healed > 0 {
        hp.stable = true;
        if was_dying && hp.current >= 0 {
            actor.unconscious = false;
        }
    }
    facts.push(format!(
        "{target}: healed {healed}; HP {}/{}",
        hp.current, hp.maximum
    ));
    Ok(())
}
pub(crate) fn apply_damage(
    state: &mut ClassPlay,
    target: &str,
    damage: i64,
    massive: Option<u32>,
    fort: i32,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    crate::necromancy::require_actor(&state, target)?;
    let actor = state.actors.get_mut(target).ok_or("Unknown target")?;
    if matches!(actor.body, TargetBody::LivingConstruct | TargetBody::Object) {
        return Err("HP rules for living constructs and objects are not yet structured".into());
    }
    let hp = state
        .health
        .get_mut(target)
        .ok_or("Target HP unavailable; initialize encounter HP")?;
    if actor.dead {
        return Err("Target is dead or destroyed".into());
    }
    let absorbed = hp.temporary.min(damage);
    hp.temporary -= absorbed;
    hp.current = hp
        .current
        .checked_sub(damage - absorbed)
        .ok_or("HP overflow")?;
    if actor.body == TargetBody::Living {
        if hp.current <= -10 {
            actor.dead = true;
        } else if damage >= 50 {
            let die = massive.ok_or("Massive damage: Fortitude d20 against DC 15 required")?;
            if die == 1 || (die != 20 && i64::from(die) + i64::from(fort) < 15) {
                actor.dead = true;
            }
            facts.push(format!(
                "Massive damage Fortitude d20 {die} + {fort} against DC 15"
            ));
        }
        if hp.current < 0 && damage > absorbed {
            hp.stable = false;
        }
        if hp.current < 0 || actor.dead {
            actor.unconscious = true;
        }
    } else if hp.current <= 0 {
        actor.dead = true;
        actor.unconscious = true;
    }
    facts.push(format!(
        "HP {}/{}; temporary {} (absorbed {}); {}",
        hp.current,
        hp.maximum,
        hp.temporary,
        absorbed,
        if actor.dead {
            "dead/destroyed"
        } else if hp.current < 0 {
            if hp.stable {
                "stable"
            } else {
                "dying"
            }
        } else if hp.current == 0 {
            "disabled"
        } else {
            "alive"
        }
    ));
    crate::combat::consume_temporary(state, target, absorbed);
    crate::feature_resolution::refresh_aura_conditions(state);
    Ok(())
}
impl Engine {
    pub fn save_encounter_health(
        &self,
        d: &CharacterDraft,
        target: &str,
        hp: HitPoints,
        protections: Protections,
    ) -> Result<PlayResult, String> {
        let actor = d.class_play.actors.get(target).ok_or("Unknown target")?;
        if matches!(actor.body, TargetBody::LivingConstruct | TargetBody::Object) {
            return Err("HP rules for this body type are unavailable".into());
        }
        if hp.maximum < 1
            || hp.maximum > 1000000
            || hp.current > hp.maximum
            || hp.current < -1000000
            || !(0..=1000000).contains(&hp.temporary)
            || protections.resistance.values().any(|n| *n > 1000000)
        {
            return Err("Invalid encounter HP or resistance".into());
        }
        let mut state = d.class_play.clone();
        let a = state.actors.get_mut(target).unwrap();
        a.dead = hp.current <= if a.body == TargetBody::Living { -10 } else { 0 };
        if hp.current < 0 || a.dead {
            a.unconscious = true;
        }
        state.health.insert(target.into(), hp);
        state.protections.insert(target.into(), protections);
        state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        Ok(PlayResult {
            state,
            success: true,
            facts: vec![format!(
                "Encounter HP and protections recorded for {target}"
            )],
        })
    }
}

impl Engine {
    /// Advance only this dying creature's turn; spell clocks are separate.
    pub fn dying_turn(
        &self,
        d: &CharacterDraft,
        target: &str,
        percentile: u32,
    ) -> Result<PlayResult, String> {
        if !(1..=100).contains(&percentile) {
            return Err("Stabilization roll must be 1-100".into());
        }
        let mut state = d.class_play.clone();
        let actor = state.actors.get(target).ok_or("Unknown target")?;
        let hp = state.health.get_mut(target).ok_or("HP unavailable")?;
        if actor.body != TargetBody::Living || actor.dead || hp.current >= 0 || hp.stable {
            return Err("Only a living, dying creature makes this stabilization roll".into());
        }
        let mut facts = vec![];
        if percentile <= 10 {
            hp.stable = true;
            facts.push("Stabilized; no HP lost this round".into());
        } else {
            hp.current -= 1;
            let actor = state.actors.get_mut(target).unwrap();
            actor.dead = hp.current <= -10;
            facts.push(format!(
                "Stabilization failed: HP {}; {}",
                hp.current,
                if actor.dead { "dead" } else { "dying" }
            ));
        }
        state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        Ok(PlayResult {
            state,
            success: percentile <= 10,
            facts,
        })
    }
}

pub(crate) fn evasion_applies(actor: &SceneActor, p: &Protections) -> bool {
    !p.armor_blocks_evasion
        && !p.helpless
        && !p.bound_or_squeezing
        && !actor.unconscious
        && !actor.dead
}
pub(crate) fn energy_after_resistance(
    p: &Protections,
    energy: pcgen_core::magic::Energy,
    amount: i64,
    facts: &mut Vec<String>,
) -> i64 {
    if p.immunity.contains(&energy) {
        facts.push(format!("{energy:?} immunity: no damage"));
        return 0;
    }
    let resistance = i64::from(p.resistance.get(&energy).copied().unwrap_or(0));
    let n = (amount - resistance).max(0);
    facts.push(format!(
        "{energy:?} resistance {resistance} per attack; final damage {n}"
    ));
    n
}
