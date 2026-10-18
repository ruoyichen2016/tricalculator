//! Data-driven class actions commit atomically to the shared encounter model.
use crate::{class_features::ResolvedClassFeature, class_play::PlayResult, CharacterDraft, Engine};
use pcgen_core::{class_play::*, magic::*};
use std::collections::BTreeMap;

pub(crate) fn finish(
    mut state: ClassPlay,
    success: bool,
    facts: Vec<String>,
) -> Result<PlayResult, String> {
    state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
    Ok(PlayResult {
        state,
        success,
        facts,
    })
}
fn die(value: Option<u32>) -> Result<u32, String> {
    value
        .filter(|n| (1..=20).contains(n))
        .ok_or("A d20 roll from 1 to 20 is required".into())
}
fn val(f: &ResolvedClassFeature, name: &str) -> Result<i32, String> {
    f.values
        .get(name)
        .copied()
        .ok_or_else(|| format!("{}: missing rule value {name}", f.name))
}
pub(crate) fn feint_applies(d: &CharacterDraft, target: &str) -> bool {
    d.class_play
        .encounter
        .feints
        .get(target)
        .is_some_and(|until| *until >= d.class_play.encounter.seconds)
        && d.class_play
            .actors
            .get("self")
            .zip(d.class_play.actors.get(target))
            .is_some_and(|(a, b)| {
                crate::targeting::distance(a, b) <= a.touch_reach_feet
                    && b.path == TargetPath::Clear
            })
}
pub(crate) fn refresh_aura_conditions(state: &mut ClassPlay) {
    crate::combat::refresh_stench(state);
    let active = |target: &String| {
        state
            .actors
            .get(target)
            .is_none_or(|a| !a.dead && a.body == TargetBody::Undead)
    };
    state.necromancy.commands.retain(|target, _| active(target));
    state.necromancy.animated.retain(|target, _| active(target));
    state
        .necromancy
        .spell_bonds
        .retain(|target, _| active(target));
    state.encounter.feints.retain(|target, _| {
        state
            .actors
            .get("self")
            .zip(state.actors.get(target))
            .is_some_and(|(a, b)| {
                !a.dead
                    && !a.unconscious
                    && crate::targeting::distance(a, b) <= a.touch_reach_feet
                    && b.path == TargetPath::Clear
            })
    });
    state.encounter.conditions.retain(|c| {
        c.aura_radius_feet.is_none_or(|radius| {
            state
                .actors
                .get("self")
                .zip(state.actors.get(&c.target))
                .is_some_and(|(origin, target)| {
                    !origin.dead
                        && !origin.unconscious
                        && !target.dead
                        && crate::targeting::distance(origin, target) <= radius
                        && matches!(target.path, TargetPath::Clear | TargetPath::Obscured)
                })
        })
    });
}
fn spend(state: &mut ClassPlay, f: &ResolvedClassFeature, amount: u32) -> Result<(), String> {
    if let Some(cap) = f.daily_uses {
        let key = format!("{}: {}", f.class_id, f.name);
        let spent = state.resources.daily.entry(key).or_default();
        *spent = spent
            .checked_add(amount)
            .filter(|n| *n <= cap)
            .ok_or("Insufficient daily resource")?;
    }
    Ok(())
}
/// One ability-score input path for formula bindings and derived snapshots.
pub(crate) fn encounter_stats(d: &CharacterDraft) -> BTreeMap<String, i32> {
    let mut stats = encounter_base_stats(d);
    for (key, n) in &mut stats {
        *n = crate::combat::modified_score(&d.class_play, "self", key, *n);
    }
    stats
}
pub(crate) fn encounter_base_stats(d: &CharacterDraft) -> BTreeMap<String, i32> {
    let mut stats = d.stats.clone();
    if let Some(rage) = &d.class_play.encounter.rage {
        for key in ["STR", "CON"] {
            if let Some(n) = stats.get_mut(key) {
                *n += rage.ability_bonus;
            }
        }
    }
    if let Some(a) = d.class_play.necromancy.actors.get("self") {
        for (key, n) in &mut stats {
            *n = (*n
                - a.damage.get(key).copied().unwrap_or(0) as i32
                - a.drain.get(key).copied().unwrap_or(0) as i32)
                .max(0);
        }
    }
    stats
}
fn in_range(state: &ClassPlay, target: &str, feet: u32, sight: bool) -> Result<(), String> {
    let a = state
        .actors
        .get("self")
        .ok_or("Register self in the scene first")?;
    let b = state.actors.get(target).ok_or("Unknown target")?;
    if crate::targeting::distance(a, b) > feet
        || !matches!(b.path, TargetPath::Clear | TargetPath::Obscured)
        || sight && b.path != TargetPath::Clear
    {
        return Err(format!("{target}: out of range or path blocked"));
    }
    Ok(())
}
pub(crate) fn save_bonus(d: &CharacterDraft, target: &str, kind: SaveKind) -> Result<i32, String> {
    let s = d
        .class_play
        .defenses
        .get(target)
        .ok_or_else(|| format!("{target}: record saving throws first"))?;
    let mut n = match kind {
        SaveKind::Fortitude => s.fortitude,
        SaveKind::Reflex => s.reflex,
        SaveKind::Will => s.will,
        SaveKind::None => 0,
    };
    if target == "self" && kind == SaveKind::Will {
        n += d
            .class_play
            .encounter
            .rage
            .as_ref()
            .map_or(0, |r| r.will_bonus);
    }
    if d.class_play
        .encounter
        .conditions
        .iter()
        .any(|c| c.target == target && c.name == "shaken")
    {
        n -= 2;
    }
    n += crate::necromancy::save_delta(
        &d.class_play,
        target,
        match kind {
            SaveKind::Fortitude => "CON",
            SaveKind::Reflex => "DEX",
            _ => "WIS",
        },
    );
    if target == "self" && kind == SaveKind::Will {
        if let Some(c) = d
            .class_play
            .tactics
            .fighting
            .as_ref()
            .filter(|c| c.expires > d.class_play.encounter.seconds)
        {
            n += c.bonus;
        }
    }
    if crate::combat::condition(&d.class_play, target, "sickened") {
        n -= 2;
    }
    Ok(n)
}
pub(crate) fn saved(
    d: &CharacterDraft,
    target: &str,
    kind: SaveKind,
    dc: i32,
    roll: Option<u32>,
    facts: &mut Vec<String>,
) -> Result<bool, String> {
    let roll = die(roll)?;
    let bonus = save_bonus(d, target, kind)?;
    let yes = roll == 20 || roll != 1 && i64::from(roll) + i64::from(bonus) >= i64::from(dc);
    facts.push(format!(
        "{target}: {kind:?} d20 {roll} + {bonus} vs DC {dc}: {}",
        if yes { "saved" } else { "failed" }
    ));
    Ok(yes)
}
impl Engine {
    pub fn save_combat_facts(
        &self,
        d: &CharacterDraft,
        target: &str,
        c: CombatFacts,
    ) -> Result<PlayResult, String> {
        if !d.class_play.actors.contains_key(target)
            || c.hit_dice == 0
            || c.hit_dice > 1000
            || c.intelligence > 100
            || c.touch_ac.unsigned_abs() > 1000
            || c.sense_motive.unsigned_abs() > 1000
            || c.base_attack_bonus.unsigned_abs() > 1000
        {
            return Err(
                "Register an actor; HD 1-1000 and bounded combat values are required".into(),
            );
        }
        let mut state = d.class_play.clone();
        state.encounter.combat.insert(target.into(), c);
        finish(
            state,
            true,
            vec![format!("Recorded combat facts for {target}")],
        )
    }
    pub fn resolve_class_feature(
        &self,
        d: &CharacterDraft,
        r: &FeatureRequest,
    ) -> Result<PlayResult, String> {
        let f = self
            .class_feature_values(d)?
            .into_iter()
            .find(|f| {
                f.class_id.eq_ignore_ascii_case(&r.class_id)
                    && f.name.eq_ignore_ascii_case(&r.feature)
            })
            .ok_or("Feature not acquired")?;
        let resolution = f
            .resolution
            .clone()
            .ok_or("This feature does not yet have an executable encounter rule")?;
        let mut state = d.class_play.clone();
        let mut facts = vec![format!("{}: {}", f.class_id, f.name)];
        let origin = state
            .actors
            .get("self")
            .ok_or("Register self in the scene")?
            .clone();
        if origin.dead || origin.unconscious {
            return Err("An unconscious or dead actor cannot use this ability".into());
        }
        match resolution {
            FeatureResolution::KnightChallenge { mode } => {
                return self.resolve_knight_challenge(d, r, &f, mode)
            }
            FeatureResolution::BreathWeapon => return self.resolve_breath(d, r, &f),
            FeatureResolution::QuickCast => {
                return Err("Apply Quick Cast in the Cast & record form".into())
            }
            FeatureResolution::Vitality {
                energy,
                self_only,
                touch,
                radius_feet,
                dice_sides,
                save,
                points,
                cap_half,
            } => {
                let targets = if let Some(radius) = radius_feet {
                    state
                        .actors
                        .iter()
                        .filter(|(_, a)| {
                            !a.dead
                                && crate::targeting::distance(&origin, a) <= radius
                                && matches!(a.path, TargetPath::Clear | TargetPath::Obscured)
                        })
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>()
                } else {
                    vec![r.target.clone()]
                };
                if targets.is_empty() {
                    return Err("No eligible targets in range".into());
                }
                let amount = if touch
                    && energy == VitalityEnergy::Negative
                    && state
                        .actors
                        .get(&r.target)
                        .is_some_and(|a| a.body == TargetBody::Undead)
                {
                    i64::from(val(&f, "undead_healing")?)
                } else if points {
                    if r.amount == 0 {
                        return Err("Choose a positive number of healing points".into());
                    }
                    i64::from(r.amount)
                } else if let Some(sides) = dice_sides {
                    let dice = i64::from(val(&f, "dice")?);
                    let flat = i64::from(val(&f, "flat")?);
                    let low = dice + flat;
                    let high = dice * i64::from(sides) + flat;
                    r.damage_roll
                        .filter(|n| *n >= low && *n <= high)
                        .ok_or_else(|| format!("Supply one shared roll between {low} and {high}"))?
                } else {
                    i64::from(val(&f, "healing")?)
                };
                if amount < 0 {
                    return Err("Negative vitality amount".into());
                }
                let key = format!("{}: {}", f.class_id, f.name);
                if touch
                    && energy == VitalityEnergy::Negative
                    && state
                        .encounter
                        .last_used
                        .get(&key)
                        .is_some_and(|t| state.encounter.seconds.saturating_sub(*t) < 6)
                {
                    return Err(
                        "This touch can be used at most once per round; advance encounter time"
                            .into(),
                    );
                }
                spend(&mut state, &f, if points { r.amount } else { 1 })?;
                for target in targets {
                    if self_only && target != "self" {
                        return Err("This ability heals only the user".into());
                    }
                    if touch {
                        in_range(&state, &target, origin.touch_reach_feet, false)?;
                    }
                    crate::necromancy::require_actor(&state, &target)?;
                    let actor = state.actors.get(&target).ok_or("Unknown target")?;
                    if actor.dead {
                        return Err("Target is dead or destroyed".into());
                    }
                    if actor.body == TargetBody::LivingConstruct {
                        return Err(
                            "Vitality rules for living constructs are not structured".into()
                        );
                    }
                    let heals = match energy {
                        VitalityEnergy::Negative => actor.body == TargetBody::Undead,
                        VitalityEnergy::Positive | VitalityEnergy::LivingHealing => {
                            actor.body == TargetBody::Living
                        }
                        VitalityEnergy::Healing => {
                            matches!(actor.body, TargetBody::Living | TargetBody::Undead)
                        }
                    };
                    let damages = matches!(
                        (energy, actor.body),
                        (VitalityEnergy::Negative, TargetBody::Living)
                            | (VitalityEnergy::Positive, TargetBody::Undead)
                    );
                    if !heals && !damages {
                        facts.push(format!("{target}: unaffected body type"));
                        continue;
                    }
                    if touch && (damages || target != "self" && !actor.willing()) {
                        let roll = die(r.roll)?;
                        let defense = state
                            .encounter
                            .combat
                            .get(&target)
                            .ok_or("Record target touch AC first")?;
                        let s = self.evaluate(d);
                        let strength = s.stats.get("STR").ok_or("STR unavailable")?;
                        let bonus = s.base_attack_bonus + (strength - 10).div_euclid(2)
                            - crate::necromancy::negative_levels(&state, "self");
                        let hit =
                            roll == 20 || roll != 1 && roll as i32 + bonus >= defense.touch_ac;
                        facts.push(format!(
                            "{target}: melee touch d20 {roll} + {bonus} vs AC {}",
                            defense.touch_ac
                        ));
                        if !hit {
                            if energy == VitalityEnergy::Positive && points {
                                return finish(d.class_play.clone(),false,vec!["Touch missed; no healing points spent. Choose the expenditure only after a hit.".into()]);
                            }
                            state
                                .encounter
                                .last_used
                                .insert(key.clone(), state.encounter.seconds);
                            return finish(state, false, facts);
                        }
                    }
                    if heals {
                        let n = if energy == VitalityEnergy::Negative && touch {
                            i64::from(val(&f, "undead_healing")?)
                        } else {
                            amount
                        };
                        crate::encounter_health::apply_healing(
                            &mut state, &target, n, cap_half, &mut facts,
                        )?;
                    } else {
                        let n = if save != SaveKind::None
                            && saved(
                                d,
                                &target,
                                save,
                                val(&f, "save_dc")?,
                                r.saves.get(&target).copied(),
                                &mut facts,
                            )? {
                            amount / 2
                        } else {
                            amount
                        };
                        let fort = save_bonus(d, &target, SaveKind::Fortitude)?;
                        crate::encounter_health::apply_damage(
                            &mut state,
                            &target,
                            n,
                            r.massive_rolls.get(&target).copied(),
                            fort,
                            &mut facts,
                        )?;
                    }
                    if touch && energy == VitalityEnergy::Negative {
                        crate::necromancy::touch_riders(
                            self,
                            d,
                            &mut state,
                            &target,
                            &r.affliction,
                            &mut facts,
                        )?;
                    }
                }
                if touch && energy == VitalityEnergy::Negative {
                    state
                        .encounter
                        .last_used
                        .insert(key, state.encounter.seconds);
                }
            }
            FeatureResolution::FearAura { radius_feet } => {
                let targets = state
                    .actors
                    .iter()
                    .filter(|(id, a)| {
                        id.as_str() != "self"
                            && !a.dead
                            && !a.allied
                            && crate::targeting::distance(&origin, a) <= radius_feet
                            && matches!(a.path, TargetPath::Clear | TargetPath::Obscured)
                    })
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>();
                for target in targets {
                    let a = &state.actors[&target];
                    if !matches!(a.body, TargetBody::Undead | TargetBody::Construct)
                        && !state.encounter.combat.contains_key(&target)
                    {
                        return Err(format!(
                            "{target}: record Intelligence and fear immunity first"
                        ));
                    }
                    if a.body == TargetBody::Undead
                        || a.body == TargetBody::Construct
                        || state.encounter.combat.get(&target).is_some_and(|c| {
                            c.intelligence == 0
                                || c.fear_immune
                                || state
                                    .actors
                                    .get(&target)
                                    .is_some_and(|a| a.body == TargetBody::Undead)
                        })
                    {
                        facts.push(format!("{target}: immune to mind-affecting fear"));
                        continue;
                    }
                    if state
                        .encounter
                        .fear_immunity
                        .get(&target)
                        .is_some_and(|until| *until > state.encounter.seconds)
                    {
                        facts.push(format!("{target}: immune after successful aura save"));
                        continue;
                    }
                    if state
                        .encounter
                        .conditions
                        .iter()
                        .any(|c| c.source == f.name && c.target == target)
                    {
                        facts.push(format!("{target}: already affected by this aura"));
                        continue;
                    }
                    if saved(
                        d,
                        &target,
                        SaveKind::Will,
                        val(&f, "will_dc")?,
                        r.saves.get(&target).copied(),
                        &mut facts,
                    )? {
                        state.encounter.fear_immunity.insert(
                            target,
                            state
                                .encounter
                                .seconds
                                .checked_add(86400)
                                .ok_or("Clock overflow")?,
                        );
                    } else if !state
                        .encounter
                        .conditions
                        .iter()
                        .any(|c| c.source == f.name && c.target == target)
                    {
                        state.encounter.conditions.push(EncounterCondition {
                            aura_radius_feet: Some(radius_feet),
                            source: f.name.clone(),
                            target,
                            name: "shaken".into(),
                            remaining_seconds: None,
                        });
                    }
                }
            }
            FeatureResolution::SurpriseFeint => {
                if state.encounter.rage.is_some() {
                    return Err("Cannot use Bluff while raging".into());
                }
                in_range(&state, &r.target, origin.touch_reach_feet, true)?;
                let a = &state.actors[&r.target];
                let c = state
                    .encounter
                    .combat
                    .get(&r.target)
                    .ok_or("Record target HD, Intelligence and opposed skill first")?;
                if c.intelligence == 0 {
                    return Err("An unintelligent creature cannot be feinted".into());
                }
                let s = self.evaluate(d);
                let mods = s
                    .stats
                    .iter()
                    .map(|(k, v)| (k.clone(), (v - 10).div_euclid(2)))
                    .collect();
                let bluff = s
                    .skills
                    .get("Bluff")
                    .copied()
                    .or_else(|| self.untrained_skill_base("Bluff", &mods, 0))
                    .ok_or("Bluff unavailable")?;
                let penalty = if c.intelligence <= 2 { 8 } else { 0 }
                    + if a.creature_type != "humanoid" { 4 } else { 0 };
                let total = die(r.roll)? as i32 + bluff - penalty;
                let opposed = die(r.opposed_roll)? as i32 + c.sense_motive + c.base_attack_bonus;
                // Opposed-check ties use the larger modifier, then require a reroll.
                let own = bluff - penalty;
                let other = c.sense_motive + c.base_attack_bonus;
                if total == opposed && own == other {
                    return Err("Opposed feint tied with equal modifiers; reroll both dice".into());
                }
                let success = total > opposed || total == opposed && own > other;
                if success {
                    state.encounter.feints.insert(
                        r.target.clone(),
                        state
                            .encounter
                            .seconds
                            .checked_add(6)
                            .ok_or("Clock overflow")?,
                    );
                }
                let improved = self.pack.entities.iter().any(|e| {
                    e.name.eq_ignore_ascii_case("Improved Feint")
                        && crate::effects::ability_matches_draft(d, e)
                });
                let action = if val(&f, "feint_move_action")? == 1 {
                    if improved {
                        "swift"
                    } else {
                        "move"
                    }
                } else if improved {
                    "move"
                } else {
                    "standard"
                };
                facts.push(format!("Bluff {total} vs Sense Motive + BAB {opposed}; {action} action; {}",if success{"target denied Dexterity to the next melee attack or spell on or before your next turn; remain in melee"}else{"feint failed"}));
                return finish(state, success, facts);
            }
            FeatureResolution::Rage => {
                if origin.body != TargetBody::Living {
                    return Err("Rage requires structured living Constitution and HP rules".into());
                }
                if state.encounter.rage.is_some()
                    || state.encounter.rage_used
                    || state.encounter.fatigued
                {
                    return Err(
                        "Rage is already active, was used this encounter, or fatigue prevents it"
                            .into(),
                    );
                }
                spend(&mut state, &f, 1)?;
                let bonus = val(&f, "ability_bonus")?;
                let rounds = val(&f, "rounds")?;
                let hp_bonus = i64::from(bonus / 2) * i64::from(d.total_level());
                let hp = state
                    .health
                    .get_mut("self")
                    .ok_or("Initialize self HP before raging")?;
                hp.maximum = hp.maximum.checked_add(hp_bonus).ok_or("HP overflow")?;
                hp.current = hp.current.checked_add(hp_bonus).ok_or("HP overflow")?;
                state.encounter.rage = Some(RageState {
                    remaining_seconds: u32::try_from(rounds)
                        .map_err(|_| "Invalid rage duration")?
                        .checked_mul(6)
                        .ok_or("Duration overflow")?,
                    ability_bonus: bonus,
                    will_bonus: val(&f, "will_bonus")?,
                    hp_bonus,
                    tireless: val(&f, "tireless")? != 0,
                });
                state.encounter.rage_used = true;
                facts.push(format!("Rage started: {rounds} rounds; STR/CON +{bonus}; AC -2; {hp_bonus} actual HP gained (not temporary HP)."));
            }
        }
        finish(state, true, facts)
    }
    pub fn end_rage(&self, d: &CharacterDraft) -> Result<PlayResult, String> {
        let mut state = d.class_play.clone();
        let mut facts = vec![];
        end_rage_state(&mut state, &mut facts)?;
        finish(state, true, facts)
    }
    pub fn advance_ability_time(
        &self,
        d: &CharacterDraft,
        seconds: u32,
    ) -> Result<PlayResult, String> {
        if seconds == 0 {
            return Err("Elapsed seconds must be positive".into());
        }
        let mut state = d.class_play.clone();
        let mut facts = vec![];
        state.encounter.seconds = state
            .encounter
            .seconds
            .checked_add(u64::from(seconds))
            .ok_or("Clock overflow")?;
        crate::combat::expire(&mut state, &mut facts);
        if let Some(rage) = &mut state.encounter.rage {
            rage.remaining_seconds = rage.remaining_seconds.saturating_sub(seconds);
            if rage.remaining_seconds == 0 {
                end_rage_state(&mut state, &mut facts)?;
            }
        }
        let now = state.encounter.seconds;
        if state
            .tactics
            .fighting
            .as_ref()
            .is_some_and(|c| c.expires <= now)
        {
            state.tactics.fighting = None;
        }
        state.tactics.mettle.retain(|_, until| *until > now);
        if state
            .tactics
            .channel
            .as_ref()
            .is_some_and(|c| c.expires.is_some_and(|until| until <= now))
        {
            crate::touch_charges::discharge(&mut state, "Full-attack round ended");
            facts.push("Full-attack channeled spell discharged at round end".into());
        }
        state.necromancy.spell_bonds.retain(|target, b| {
            let keep = b.expires.is_none_or(|until| until > now);
            if !keep {
                facts.push(format!("{target}: {} expired; control ended", b.source));
            }
            keep
        });
        state.encounter.feints.retain(|_, t| *t >= now);
        state.encounter.fear_immunity.retain(|_, t| *t > now);
        for c in &mut state.encounter.conditions {
            if let Some(t) = &mut c.remaining_seconds {
                *t = t.saturating_sub(seconds);
            }
        }
        state
            .encounter
            .conditions
            .retain(|c| c.remaining_seconds != Some(0));
        refresh_aura_conditions(&mut state);
        facts.push(format!("Advanced class ability clock by {seconds} seconds"));
        finish(state, true, facts)
    }
    pub fn end_ability_encounter(&self, d: &CharacterDraft) -> Result<PlayResult, String> {
        let mut state = d.class_play.clone();
        let mut facts = vec![];
        if state
            .tactics
            .channel
            .as_ref()
            .is_some_and(|c| c.mode == pcgen_core::tactics::ChargeMode::FullChannel)
        {
            if let Some(f) = crate::touch_charges::discharge(&mut state, "Encounter ended") {
                facts.push(f);
            }
        }
        if state.encounter.rage.is_some() {
            end_rage_state(&mut state, &mut facts)?;
        }
        state.tactics.wounded_in_melee.clear();
        state.encounter.rage_used = false;
        state.encounter.fatigued = false;
        state.encounter.feints.clear();
        state
            .encounter
            .conditions
            .retain(|c| c.aura_radius_feet.is_none());
        facts.push("Encounter ended: fatigue and encounter-only states cleared; daily expenditure and 24-hour fear immunity retained.".into());
        finish(state, true, facts)
    }
}
fn end_rage_state(state: &mut ClassPlay, facts: &mut Vec<String>) -> Result<(), String> {
    let rage = state.encounter.rage.take().ok_or("No active rage")?;
    let hp = state.health.get_mut("self").ok_or("Self HP unavailable")?;
    hp.maximum = hp.maximum.checked_sub(rage.hp_bonus).ok_or("HP overflow")?;
    hp.current = hp.current.checked_sub(rage.hp_bonus).ok_or("HP overflow")?;
    let a = state
        .actors
        .get_mut("self")
        .ok_or("Self actor unavailable")?;
    a.dead = hp.current <= -10;
    if hp.current < 0 || a.dead {
        a.unconscious = true;
        hp.stable = false;
    }
    state.encounter.fatigued = !rage.tireless;
    facts.push(format!(
        "Rage ended: removed {} actual HP; HP {}/{}; {}",
        rage.hp_bonus,
        hp.current,
        hp.maximum,
        if rage.tireless {
            "no fatigue"
        } else {
            "fatigued until encounter ends"
        }
    ));
    Ok(())
}
