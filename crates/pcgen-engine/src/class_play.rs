//! Explicit, immutable-in / new-state-out class action transitions.
use crate::{CharacterDraft, Engine};
use pcgen_core::class_play::*;
use serde::Serialize;
use std::collections::BTreeSet;
#[derive(Debug, Serialize)]
pub struct PlayResult {
    pub state: ClassPlay,
    pub success: bool,
    pub facts: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct AirPool {
    pub total: u32,
    pub spent: u32,
    pub remaining: u32,
    pub effective_level: Option<u32>,
    pub basis: String,
}
fn revision(state: &mut ClassPlay) -> Result<(), String> {
    state.revision = state
        .revision
        .checked_add(1)
        .ok_or("Encounter revision overflow")?;
    Ok(())
}
fn die(n: u32, sides: u32) -> Result<i64, String> {
    if n == 0 || n > sides {
        Err(format!("Supply a d{sides} roll"))
    } else {
        Ok(n as i64)
    }
}
/// Opposed ties compare modifiers, then require a reroll; neither natural 1 nor 20 is special.
fn opposed(a: i64, b: i64, roll: u32, other: u32) -> Result<Option<bool>, String> {
    let x = a + die(roll, 20)?;
    let y = b + die(other, 20)?;
    Ok(if x == y && a == b {
        None
    } else {
        Some(x > y || (x == y && a > b))
    })
}
fn end_spell(s: &mut ControlledSpell, reason: &str) {
    s.ended = Some(reason.into());
    s.controller = SpellController::None;
    s.awaiting_reassertion = false;
    s.control_mode = None;
}
fn no_reassert(s: &mut ControlledSpell) {
    s.awaiting_reassertion = false;
    s.control_mode = None;
    s.tail_remaining = Some(s.concentration_tail_rounds);
    if s.concentration_tail_rounds == 0 {
        end_spell(s, "Concentration ended");
    }
}
impl Engine {
    pub fn air_pool(&self, draft: &CharacterDraft) -> Result<AirPool, String> {
        let has = draft.domains.iter().any(|d| d.eq_ignore_ascii_case("Air"))
            || crate::engine::effective_class_levels(&self.pack, draft)
                .iter()
                .any(|l| {
                    crate::magic::class_magic(&self.pack, &l.class_id).is_some_and(|m| {
                        m.domains
                            .iter()
                            .any(|d| d.domain == "Air" && l.level >= d.level)
                    })
                });
        if !has {
            return Err("Air domain is not acquired".into());
        }
        let snap = self.evaluate(draft);
        if !snap.is_available() {
            return Err(snap.diagnostics.join("; "));
        }
        let cha = (snap.stats.get("CHA").ok_or("Charisma unavailable")? - 10).div_euclid(2);
        let extra = draft
            .abilities
            .iter()
            .filter(|(_, n)| n.eq_ignore_ascii_case("Extra Turning"))
            .count() as i64;
        let total = u32::try_from((3i64 + cha as i64 + 4 * extra).max(0))
            .map_err(|_| "Air pool overflow")?;
        let cleric = draft.level_in_class("Cleric");
        let (level, basis) = if let Some(r) = draft.domain_level_rulings.get("Air") {
            if r.reason.trim().is_empty() || r.level > 10000 {
                return Err(
                    "An Air effective-level ruling needs a reason and a level from 0 to 10000"
                        .into(),
                );
            }
            (
                Some(r.level),
                format!("Explicit table ruling: {}", r.reason),
            )
        } else if cleric > 0 {
            (Some(cleric),"Actual Cleric class levels; spellcasting advancement does not advance domain powers".into())
        } else {
            (None,"Complete Divine p.20 does not assign nonclerics an effective Cleric level. Supply an explicit table ruling to resolve HD effects; daily uses are known independently.".into())
        };
        Ok(AirPool {
            total,
            spent: draft.class_play.air_spent,
            remaining: total.saturating_sub(draft.class_play.air_spent),
            effective_level: level,
            basis,
        })
    }
    pub fn air_attempt(
        &self,
        draft: &CharacterDraft,
        r: &AirRequest,
    ) -> Result<PlayResult, String> {
        let pool = self.air_pool(draft)?;
        let level = pool.effective_level.ok_or(
            "Choose an explicit Air-domain effective Cleric level before rolling HD effects",
        )?;
        if pool.remaining == 0 {
            return Err("No Air-domain uses remain today".into());
        }
        let snap = self.evaluate(draft);
        let cha = (snap.stats.get("CHA").ok_or("Charisma unavailable")? - 10).div_euclid(2) as i64;
        let check = die(r.roll, 20)? + cha;
        let offset = if check <= 0 {
            -4
        } else if check >= 22 {
            4
        } else {
            (check - 1) / 3 - 3
        };
        let max_hd = (level as i64 + offset).max(0) as u32;
        let damage =
            (die(r.damage_die_1, 6)? + die(r.damage_die_2, 6)? + level as i64 + cha).max(0) as u32;
        let mut state = draft.class_play.clone();
        let mut facts = vec![
            pool.basis,
            format!(
                "Air pool: {} of {} uses remain; undead turning is untouched",
                pool.remaining - 1,
                pool.total
            ),
            format!("Turning check {check}; maximum HD {max_hd}; turning damage {damage}"),
            "Standard action, no attack of opportunity; present the domain's divine focus".into(),
        ];
        if r.mode == AirMode::DispelTurning && r.original_turning_check.is_none() {
            return Err("Dispelling turning requires the original turning check total".into());
        }
        let mut targets = r.targets.clone();
        targets.sort_by_key(|t| t.distance_feet);
        let mut ids = BTreeSet::new();
        let mut budget = damage;
        let mut success = false;
        for t in targets {
            if t.id.trim().is_empty() || !ids.insert(t.id.clone()) || t.hit_dice == 0 {
                return Err("Targets need unique IDs and positive Hit Dice".into());
            }
            let boosted = state
                .air_effects
                .get(&t.id)
                .and_then(|e| e.effective_hit_dice)
                .unwrap_or(t.hit_dice)
                .max(t.hit_dice);
            let effective = boosted
                .checked_add(t.turning_resistance)
                .ok_or("Target HD overflow")?;
            let eligible = if r.mode == AirMode::TurnEarth {
                t.earth
            } else {
                t.air
            };
            if !eligible || t.distance_feet > 60 || t.total_cover {
                facts.push(format!(
                    "{}: outside the eligible type, range or line of effect",
                    t.id
                ));
                continue;
            }
            if r.mode == AirMode::BolsterAir {
                if max_hd > t.hit_dice {
                    state.air_effects.insert(
                        t.id.clone(),
                        AirEffect {
                            outcome: "Bolstered".into(),
                            rounds: 10,
                            effective_hit_dice: Some(max_hd),
                        },
                    );
                    facts.push(format!(
                        "{}: bolstered to {max_hd} effective HD for 10 rounds",
                        t.id
                    ));
                    success = true;
                }
                continue;
            }
            if r.mode == AirMode::DispelTurning {
                if check < i64::from(r.original_turning_check.unwrap()) {
                    continue;
                }
            } else if effective > max_hd {
                continue;
            }
            if effective > budget {
                continue;
            }
            budget -= effective;
            let outcome = match r.mode {
                AirMode::TurnEarth => {
                    if level as u64 >= 2 * effective as u64 {
                        "Destroyed"
                    } else {
                        "Turned"
                    }
                }
                AirMode::RebukeAir => {
                    if level as u64 >= 2 * effective as u64 {
                        let commanded: u64 = state
                            .air_commands
                            .iter()
                            .filter(|(id, _)| *id != &t.id)
                            .map(|(_, hd)| *hd as u64)
                            .sum();
                        if commanded + t.hit_dice as u64 > level as u64 {
                            facts.push(format!("{}: command capacity exceeded; relinquish a commanded creature first",t.id));
                            continue;
                        }
                        state.air_commands.insert(t.id.clone(), t.hit_dice);
                        "Commanded"
                    } else {
                        "Rebuked"
                    }
                }
                AirMode::DispelTurning => "Turning dispelled",
                AirMode::BolsterAir => unreachable!(),
            };
            success = true;
            facts.push(format!("{}: {outcome}", t.id));
            if ["Turned", "Rebuked"].contains(&outcome) {
                state.air_effects.insert(
                    t.id,
                    AirEffect {
                        outcome: outcome.into(),
                        rounds: 10,
                        effective_hit_dice: None,
                    },
                );
            } else {
                state.air_effects.remove(&t.id);
            }
        }
        facts.push("Turned creatures flee or cower for 10 rounds; approaching within 10 feet breaks turning. Rebuked creatures cower for 10 rounds. Commands persist until relinquished; giving mental orders takes a standard action.".into());
        state.air_spent += 1;
        revision(&mut state)?;
        Ok(PlayResult {
            state,
            success,
            facts,
        })
    }
    pub fn air_manage(
        &self,
        draft: &CharacterDraft,
        operation: &str,
        target: &str,
        rounds: u32,
    ) -> Result<PlayResult, String> {
        self.air_pool(draft)?;
        let mut state = draft.class_play.clone();
        let fact = match operation {
            "new_day" => {
                state.air_spent = 0;
                "Air daily uses renewed; commands and effects are not removed"
            }
            "release" => {
                if state.air_commands.remove(target).is_none() {
                    return Err("That creature is not commanded".into());
                }
                "Command relinquished"
            }
            "order" => {
                if !state.air_commands.contains_key(target) {
                    return Err("That creature is not commanded".into());
                }
                "Mental order issued as a standard action; no turning use spent"
            }
            "approach" => {
                if !state
                    .air_effects
                    .get(target)
                    .is_some_and(|e| e.outcome == "Turned")
                {
                    return Err("That creature is not turned".into());
                }
                state.air_effects.remove(target);
                "Turning broken by approaching within 10 feet"
            }
            "advance" => {
                if rounds == 0 {
                    return Err("Advance a positive number of rounds".into());
                }
                state.air_effects.retain(|_, e| {
                    e.rounds = e.rounds.saturating_sub(rounds);
                    e.rounds > 0
                });
                "Air effect durations advanced"
            }
            _ => return Err("Unknown Air management operation".into()),
        };
        revision(&mut state)?;
        Ok(PlayResult {
            state,
            success: true,
            facts: vec![fact.into()],
        })
    }
    pub fn register_spell_effect(
        &self,
        draft: &CharacterDraft,
        id: &str,
        spell: ControlledSpell,
    ) -> Result<PlayResult, String> {
        if spell.original_caster.eq_ignore_ascii_case(&draft.id)
            || spell.original_caster.eq_ignore_ascii_case(&draft.name)
        {
            return Err("Spell control applies to another caster's spell".into());
        }
        if id.trim().is_empty() || draft.class_play.spells.contains_key(id) {
            return Err("Use a new nonempty effect ID".into());
        }
        if spell.spell.trim().is_empty()
            || spell.original_caster.trim().is_empty()
            || spell.original_caster_level == 0
            || spell.spell_level > 9
            || spell.remaining_rounds == Some(0)
            || spell.controller != SpellController::Original
            || spell.control_mode.is_some()
            || spell.awaiting_reassertion
            || spell.ended.is_some()
            || spell.tail_remaining.is_some()
        {
            return Err(
                "Register an active original spell with its original casting variables".into(),
            );
        }
        let mut state = draft.class_play.clone();
        state.spells.insert(id.into(), spell);
        revision(&mut state)?;
        Ok(PlayResult{state,success:true,facts:vec!["Original spell recorded. Its casting variables will not be recomputed when control changes.".into()]})
    }
    pub fn spell_control(
        &self,
        draft: &CharacterDraft,
        id: &str,
        event: &ControlEvent,
    ) -> Result<PlayResult, String> {
        let snap = self.evaluate(draft);
        if !snap.is_available() {
            return Err(snap.diagnostics.join("; "));
        }
        let mut state = draft.class_play.clone();
        let spell = state.spells.get_mut(id).ok_or("Unknown spell effect")?;
        if spell.ended.is_some() {
            return Err("This spell has ended; it cannot be controlled or restored".into());
        }
        let mut success = true;
        let mut facts = vec![];
        let caster_bonus = |track: &str| -> Result<i64, String> {
            let level = *snap
                .caster_levels
                .get(track)
                .ok_or("Select an acquired casting track")?;
            let class = self
                .pack
                .classes
                .get(track)
                .ok_or("Unknown casting track")?;
            Ok(level as i64
                + if class.spell_power == pcgen_core::SpellPowerType::Divine {
                    2
                } else {
                    0
                })
        };
        let other = spell.original_caster_level as i64 + if spell.original_divine { 2 } else { 0 };
        match event {
            ControlEvent::Acquire {
                mode,
                track,
                distance_feet,
                willing,
                roll,
                original_roll,
            } => {
                if spell.controller != SpellController::Original || spell.awaiting_reassertion {
                    return Err(
                        "Acquisition requires an active effect controlled by its original caster"
                            .into(),
                    );
                }
                let acquired = crate::engine::effective_class_levels(&self.pack, draft)
                    .iter()
                    .any(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id).is_some_and(|m| {
                            match mode {
                                ControlMode::Seize => m.seize_from_level,
                                ControlMode::Snatch => m.snatch_from_level,
                            }
                            .is_some_and(|n| l.level >= n)
                        })
                    });
                if !acquired {
                    return Err("Required Incantatrix class feature not acquired".into());
                }
                if *distance_feet > 30
                    || (*mode == ControlMode::Seize && !spell.concentration)
                    || (*mode == ControlMode::Snatch
                        && (spell.concentration || !spell.controllable))
                {
                    return Err("Seize requires concentration and the caster within 30 feet; Snatch requires a non-concentration controllable effect within 30 feet".into());
                }
                let won = if *mode == ControlMode::Seize && *willing {
                    Some(true)
                } else {
                    opposed(caster_bonus(track)?, other, *roll, *original_roll)?
                };
                match won {
                    Some(true) => {
                        spell.controller = SpellController::Incantatrix;
                        spell.control_mode = Some(*mode);
                        facts.push("Control transferred; original casting variables and remaining duration preserved".into());
                    }
                    Some(false) => {
                        success = false;
                        facts.push("Opposed check failed; original caster retains control".into());
                    }
                    None => {
                        success = false;
                        facts.push(
                            "Equal totals and modifiers: reroll both checks; control unchanged"
                                .into(),
                        );
                    }
                }
            }
            ControlEvent::Maintain => {
                if state
                    .concentration_checks
                    .values()
                    .any(|c| c.spell == id && c.success.is_none())
                {
                    return Err(
                        "Resolve pending damage Concentration checks before maintaining".into(),
                    );
                }
                if spell.controller != SpellController::Incantatrix
                    || spell.control_mode != Some(ControlMode::Seize)
                {
                    return Err("Only a seized concentration spell can be maintained".into());
                }
                facts.push(
                    "Concentration maintained with a standard action; duration is not refreshed"
                        .into(),
                );
            }
            ControlEvent::Lapse => {
                if spell.controller != SpellController::Incantatrix
                    || spell.control_mode != Some(ControlMode::Seize)
                {
                    return Err("No seized concentration to relinquish".into());
                }
                spell.controller = SpellController::None;
                spell.awaiting_reassertion = true;
                for c in state
                    .concentration_checks
                    .values_mut()
                    .filter(|c| c.spell == id && c.success.is_none())
                {
                    c.success = Some(false);
                    c.fact.push_str("; cancelled because concentration lapsed");
                }
                facts.push("Concentration lapsed; original caster may immediately attempt to reassert control".into());
            }
            ControlEvent::Reassert { roll } => {
                if !spell.awaiting_reassertion {
                    return Err("No pending reassertion attempt".into());
                }
                let total = die(*roll, 20)? + spell.original_caster_level as i64;
                let dc = 15 + spell.spell_level;
                success = total >= dc as i64;
                spell.awaiting_reassertion = false;
                spell.control_mode = None;
                if success {
                    spell.controller = SpellController::Original;
                    spell.tail_remaining = None;
                } else {
                    no_reassert(spell);
                }
                facts.push(format!("Original caster check {total} vs DC {dc}; the opposed-check divine bonus does not apply to reassertion"));
            }
            ControlEvent::DeclineReassertion => {
                if !spell.awaiting_reassertion {
                    return Err("No pending reassertion attempt".into());
                }
                no_reassert(spell);
                facts.push("Original caster declined; no one controls the spell, and its concentration tail begins".into());
            }
            ControlEvent::Dismiss {
                track,
                roll,
                original_roll,
            } => {
                if spell.controller != SpellController::Incantatrix || !spell.dismissible {
                    return Err(
                        "Only an Incantatrix-controlled dismissible spell can be dismissed".into(),
                    );
                }
                if spell.control_mode == Some(ControlMode::Snatch) {
                    match opposed(caster_bonus(track)?, other, *roll, *original_roll)? {
                        Some(true) => end_spell(spell, "Dismissed"),
                        Some(false) => {
                            success = false;
                            spell.controller = SpellController::Original;
                            spell.control_mode = None;
                            facts.push(
                                "Dismissal failed; the active spell returns to its original caster"
                                    .into(),
                            );
                        }
                        None => {
                            success = false;
                            facts.push(
                                "Opposed tie: reroll; spell remains under Incantatrix control"
                                    .into(),
                            );
                        }
                    }
                } else {
                    end_spell(spell, "Dismissed");
                }
            }
            ControlEvent::AdvanceTime { rounds } => {
                if *rounds == 0 || spell.awaiting_reassertion {
                    return Err(
                        "Advance positive rounds after resolving the pending reassertion decision"
                            .into(),
                    );
                }
                if let Some(n) = &mut spell.remaining_rounds {
                    *n = n.saturating_sub(*rounds);
                }
                if let Some(n) = &mut spell.tail_remaining {
                    *n = n.saturating_sub(*rounds);
                }
                if spell.remaining_rounds == Some(0) || spell.tail_remaining == Some(0) {
                    end_spell(spell, "Duration expired");
                }
                facts.push(format!(
                    "Advanced {rounds} rounds; no casting variables changed"
                ));
            }
            ControlEvent::Dispel => {
                end_spell(spell, "Dispelled");
                facts.push("Effect ended by a resolved external dispel".into());
            }
        }
        if spell.controller == SpellController::Incantatrix {
            facts.push("Original caster has +2 circumstance on saves against this spell; other targets do not".into());
        }
        facts.push(format!(
            "Controller: {:?}; ended: {}",
            spell.controller,
            spell.ended.as_deref().unwrap_or("no")
        ));
        revision(&mut state)?;
        Ok(PlayResult {
            state,
            success,
            facts,
        })
    }
}
