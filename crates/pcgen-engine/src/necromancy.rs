//! RAW afflictions and rebuking share the encounter ledger and atomic commit path.
use crate::{class_play::PlayResult, CharacterDraft, Engine};
use pcgen_core::{class_play::*, necromancy::*};
use std::collections::BTreeMap;
const DAY: u64 = 86_400;
pub(crate) fn require_actor(s: &ClassPlay, target: &str) -> Result<(), String> {
    if s.combat_state
        .exposures
        .values()
        .any(|e| e.target == target && e.saved.is_none())
    {
        return Err(format!(
            "{target}: stench save pending; resolve it before using affected values"
        ));
    }
    if s.necromancy
        .actors
        .get(target)
        .is_some_and(|a| a.needs_level_rebuild)
    {
        Err(format!("{target}: unavailable after permanent level loss; rebuild the external actor's class-dependent facts first"))
    } else {
        Ok(())
    }
}
fn finish(mut state: ClassPlay, facts: Vec<String>) -> Result<PlayResult, String> {
    crate::feature_resolution::refresh_aura_conditions(&mut state);
    state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
    Ok(PlayResult {
        state,
        success: true,
        facts,
    })
}
pub fn negative_levels(s: &ClassPlay, target: &str) -> i32 {
    s.necromancy
        .negative_levels
        .iter()
        .filter(|x| x.target == target)
        .count() as i32
}
pub(crate) fn score(s: &ClassPlay, target: &str, key: &str) -> Result<i32, String> {
    let a = s
        .necromancy
        .actors
        .get(target)
        .ok_or("Record affliction ability scores first")?;
    let base = *a
        .scores
        .get(key)
        .ok_or_else(|| format!("{target}: {key} unavailable"))?;
    Ok(crate::combat::modified_score(
        s,
        target,
        key,
        (base
            - a.damage.get(key).copied().unwrap_or(0) as i32
            - a.drain.get(key).copied().unwrap_or(0) as i32)
            .max(0),
    ))
}
pub(crate) fn ability_delta(s: &ClassPlay, target: &str, key: &str) -> i32 {
    s.necromancy
        .actors
        .get(target)
        .and_then(|a| a.scores.get(key))
        .map_or(0, |base| {
            (score(s, target, key).expect("registered base score") - 10).div_euclid(2)
                - (base - 10).div_euclid(2)
        })
}
pub(crate) fn save_delta(s: &ClassPlay, target: &str, key: &str) -> i32 {
    -negative_levels(s, target) + ability_delta(s, target, key)
}
fn conditional_save(
    engine: &Engine,
    d: &CharacterDraft,
    target: &str,
    disease: bool,
) -> Result<i32, String> {
    if target != "self" {
        return Ok(0);
    }
    let (name, key) = if disease {
        (
            "Mental Bastion",
            "sleep_stun_poison_paralysis_disease_save_bonus",
        )
    } else {
        ("Negative Energy Resistance", "save_bonus")
    };
    match engine
        .class_feature_values(d)?
        .iter()
        .find(|f| f.class_id == "Dread Necromancer" && f.name == name)
    {
        Some(f) => f
            .values
            .get(key)
            .copied()
            .ok_or_else(|| format!("{name}: save bonus unavailable")),
        None => Ok(0),
    }
}
fn fort(
    s: &ClassPlay,
    target: &str,
    dc: i32,
    roll: Option<u32>,
    extra: i32,
    facts: &mut Vec<String>,
) -> Result<bool, String> {
    require_actor(s, target)?;
    let roll = roll
        .filter(|n| (1..=20).contains(n))
        .ok_or("Fortitude d20 1-20 required")?;
    let bonus = s
        .defenses
        .get(target)
        .ok_or("Record Fortitude defense first")?
        .fortitude
        + save_delta(s, target, "CON")
        + extra;
    let yes = roll == 20 || roll != 1 && roll as i32 + bonus >= dc;
    facts.push(format!(
        "{target}: Fortitude {roll} + {bonus} vs {dc}: {}",
        if yes { "saved" } else { "failed" }
    ));
    Ok(yes)
}
fn damage(
    s: &mut ClassPlay,
    target: &str,
    rule: &DiseaseRule,
    r: &AfflictionRolls,
    extra: i32,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    let old_con = score(s, target, "CON")?;
    let mut amounts = BTreeMap::new();
    for (key, sides) in &rule.damage {
        let n = r
            .damage
            .get(key)
            .copied()
            .filter(|n| *n >= 1 && *n <= *sides)
            .ok_or_else(|| format!("{key}: disease damage roll 1-{sides} required"))?;
        amounts.insert(key.clone(), n.min(score(s, target, key)? as u32));
    }
    let secondary = rule.blindness && amounts.values().any(|n| *n >= 2) || rule.drain;
    for (key, n) in &amounts {
        *s.necromancy
            .actors
            .get_mut(target)
            .ok_or("Ability scores unavailable")?
            .damage
            .entry(key.clone())
            .or_default() += n;
    }
    let secondary_failed = secondary && !fort(s, target, rule.dc, r.secondary_save, extra, facts)?;
    for (key, n) in &amounts {
        let drain = u32::from(rule.drain && secondary_failed && *n > 0);
        let a = s.necromancy.actors.get_mut(target).unwrap();
        *a.damage.get_mut(key).unwrap() -= drain;
        *a.drain.entry(key.clone()).or_default() += drain;
        facts.push(format!(
            "{target}: {key} damage {}, permanent drain {drain}",
            n - drain
        ));
    }
    if rule.blindness && secondary_failed {
        s.encounter.conditions.push(EncounterCondition {
            source: rule.name.clone(),
            target: target.into(),
            name: "blinded".into(),
            remaining_seconds: None,
            aura_radius_feet: None,
        });
    }
    let new_con = score(s, target, "CON")?;
    if new_con != old_con {
        let hd = s
            .encounter
            .combat
            .get(target)
            .ok_or("Record target HD first")?
            .hit_dice as i64;
        let delta = i64::from((new_con - 10).div_euclid(2) - (old_con - 10).div_euclid(2)) * hd;
        let negative_hp = i64::from(negative_levels(s, target)) * 5;
        let hp = s.health.get_mut(target).ok_or("Record target HP first")?;
        let next = (hp.maximum + negative_hp + delta).max(hd) - negative_hp;
        s.necromancy
            .actors
            .get_mut(target)
            .unwrap()
            .constitution_hp_loss += hp.maximum - next;
        hp.current += next - hp.maximum;
        hp.maximum = next;
        facts.push(format!(
            "{target}: Constitution updates HP to {}/{}",
            hp.current, hp.maximum
        ));
    }
    for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
        if score(s, target, key)? == 0 {
            let actor = s.actors.get_mut(target).ok_or("Unknown actor")?;
            if key == "CON" {
                actor.dead = true;
                actor.unconscious = true;
            } else if ["INT", "WIS", "CHA"].contains(&key) {
                actor.unconscious = true;
            }
            s.encounter.conditions.push(EncounterCondition {
                source: "Ability Score Loss".into(),
                target: target.into(),
                name: "helpless".into(),
                remaining_seconds: None,
                aura_radius_feet: None,
            });
        }
    }
    Ok(())
}
pub(crate) fn touch_riders(
    engine: &Engine,
    draft: &CharacterDraft,
    s: &mut ClassPlay,
    target: &str,
    r: &AfflictionRolls,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    let actor = s.actors.get(target).ok_or("Unknown target")?;
    let living = actor.body == TargetBody::Living;
    let immune_disease = actor.dead
        || !living
        || s.necromancy
            .actors
            .get(target)
            .is_some_and(|a| a.immune_disease);
    let immune_drain = actor.dead
        || !living
        || s.necromancy
            .actors
            .get(target)
            .is_some_and(|a| a.immune_drain);
    if let Some(armed) = s.necromancy.armed_disease.take() {
        if immune_disease {
            facts.push(format!("{target}: disease immune; armed touch discharged"));
        } else if !fort(
            s,
            target,
            armed.dc,
            r.save,
            conditional_save(engine, draft, target, true)?,
            facts,
        )? {
            if s.necromancy
                .diseases
                .iter()
                .any(|c| c.target == target && c.rule.name == armed.rule.name)
            {
                facts.push("Existing disease does not stack".into());
            } else {
                damage(
                    s,
                    target,
                    &armed.rule,
                    r,
                    conditional_save(engine, draft, target, true)?,
                    facts,
                )?;
                s.necromancy.diseases.push(DiseaseCase {
                    target: target.into(),
                    rule: armed.rule,
                    due: s.encounter.seconds + DAY,
                    successes: 0,
                });
            }
        }
    }
    if let Some(armed) = s.necromancy.armed_drain.take() {
        if immune_drain {
            facts.push(format!(
                "{target}: energy drain immune; armed touch discharged"
            ));
        } else {
            let feature = engine
                .class_feature_values(draft)?
                .into_iter()
                .find(|f| f.class_id == "Dread Necromancer" && f.name == "Enervating Touch")
                .ok_or("Enervating Touch no longer acquired")?;
            let spent = s
                .resources
                .daily
                .entry("Dread Necromancer: Enervating Touch".into())
                .or_default();
            *spent = spent
                .checked_add(armed.count)
                .filter(|n| *n <= feature.daily_uses.unwrap_or(0))
                .ok_or("Enervating Touch daily negative-level budget exhausted")?;
            let hd = s
                .encounter
                .combat
                .get(target)
                .ok_or("Record HD first")?
                .hit_dice;
            let hp = s.health.get_mut(target).ok_or("Record HP first")?;
            hp.current -= 5 * i64::from(armed.count);
            hp.maximum -= 5 * i64::from(armed.count);
            for _ in 0..armed.count {
                let lost_slot = if target == "self" {
                    let mut current = draft.clone();
                    current.class_play = s.clone();
                    engine
                        .evaluate(&current)
                        .spell_slots
                        .into_iter()
                        .filter_map(|(key, cap)| {
                            let (track, level) = key.rsplit_once('|')?;
                            let level = level.parse::<u32>().ok()?;
                            let spent = s
                                .resources
                                .slots_spent
                                .get(&track.to_ascii_lowercase())
                                .and_then(|m| m.get(&level))
                                .copied()
                                .unwrap_or(0);
                            (cap > spent).then_some((level, key))
                        })
                        .max()
                        .map(|(_, key)| key)
                } else {
                    None
                };
                s.necromancy.negative_levels.push(NegativeLevel {
                    target: target.into(),
                    dc: armed.dc,
                    due: s.encounter.seconds + DAY,
                    lost_slot,
                });
            }
            if s.health.contains_key("self") {
                crate::combat::apply_temporary(
                    s,
                    "self",
                    "Enervating Touch",
                    5 * i64::from(armed.count),
                    s.encounter.seconds + 3600,
                    facts,
                )?;
            }
            if negative_levels(s, target) >= hd as i32 {
                let a = s.actors.get_mut(target).unwrap();
                a.dead = true;
                a.unconscious = true;
            }
            facts.push(format!("{target}: {} negative levels; -5 HP and -1 to rolls/effective level each; individual saves due in 24 hours",armed.count));
        }
    }
    Ok(())
}
impl Engine {
    pub fn necromancy_diseases(&self) -> Vec<DiseaseRule> {
        crate::magic::class_magic(&self.pack, "Dread Necromancer")
            .map(|m| m.diseases.clone())
            .unwrap_or_default()
    }
    pub fn save_affliction_actor(
        &self,
        d: &CharacterDraft,
        target: &str,
        mut actor: AfflictedActor,
    ) -> Result<PlayResult, String> {
        if !d.class_play.actors.contains_key(target)
            || ["STR", "DEX", "CON", "INT", "WIS", "CHA"].iter().any(|k| {
                let undead = d
                    .class_play
                    .actors
                    .get(target)
                    .is_some_and(|a| a.body == TargetBody::Undead);
                if undead && *k == "CON" {
                    false
                } else {
                    actor.scores.get(*k).is_none_or(|n| {
                        !(if undead && *k == "INT" { 0 } else { 1 }..=100).contains(n)
                    })
                }
            })
        {
            return Err("Register actor and all six ability scores (1-100)".into());
        }
        let mut s = d.class_play.clone();
        if let Some(old) = s.necromancy.actors.get(target) {
            actor.damage = old.damage.clone();
            actor.drain = old.drain.clone();
            actor.last_recovery = old.last_recovery;
            actor.constitution_hp_loss = old.constitution_hp_loss;
            actor.permanent_levels_lost = old.permanent_levels_lost;
            actor.needs_level_rebuild = old.needs_level_rebuild;
        } else {
            actor.last_recovery = s.encounter.seconds;
        }
        if target == "self" && actor.scores != d.stats {
            return Err("Self baseline scores must match the character editor".into());
        }
        s.necromancy.actors.insert(target.into(), actor);
        finish(s, vec![format!("{target}: saved affliction facts")])
    }
    pub fn arm_necromantic_touch(
        &self,
        d: &CharacterDraft,
        disease: Option<&str>,
        count: u32,
    ) -> Result<PlayResult, String> {
        let name = if disease.is_some() {
            "Scabrous Touch"
        } else {
            "Enervating Touch"
        };
        let f = self
            .class_feature_values(d)?
            .into_iter()
            .find(|f| f.class_id == "Dread Necromancer" && f.name == name)
            .ok_or("Class feature not acquired")?;
        let mut s = d.class_play.clone();
        let origin = s.actors.get("self").ok_or("Register self first")?;
        if origin.dead || origin.unconscious {
            return Err("User cannot act".into());
        }
        let spent = s
            .resources
            .daily
            .entry(format!("{}: {}", f.class_id, f.name))
            .or_default();
        let cost = if disease.is_some() { 1 } else { count };
        if cost == 0
            || cost > 2
            || spent
                .checked_add(cost)
                .is_none_or(|n| n > f.daily_uses.unwrap_or(0))
        {
            return Err("Insufficient daily uses or invalid negative-level count (1-2)".into());
        }
        if let Some(name) = disease {
            if s.necromancy.armed_disease.is_some() {
                return Err("A disease touch is already armed".into());
            }
            let rule = crate::magic::class_magic(&self.pack, "Dread Necromancer")
                .and_then(|m| m.diseases.iter().find(|x| x.name == name))
                .ok_or("Unknown disease")?
                .clone();
            s.necromancy.armed_disease = Some(ArmedDisease {
                rule,
                dc: *f
                    .values
                    .get("initial_fortitude_dc")
                    .ok_or("Initial DC unavailable")?,
            });
        } else {
            if s.necromancy.armed_drain.is_some() {
                return Err("An enervating touch is already armed".into());
            }
            s.necromancy.armed_drain = Some(ArmedDrain {
                count,
                dc: *f
                    .values
                    .get("removal_save_dc")
                    .ok_or("Removal DC unavailable")?,
            });
        }
        if disease.is_some() {
            *spent += cost;
        }
        finish(
            s,
            vec![format!(
                "Swift action: {name} armed until the next successful Charnel Touch"
            )],
        )
    }
    pub fn resolve_disease_day(
        &self,
        d: &CharacterDraft,
        index: usize,
        r: &AfflictionRolls,
    ) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        let c = s
            .necromancy
            .diseases
            .get(index)
            .ok_or("Unknown disease case")?
            .clone();
        if s.encounter.seconds < c.due {
            return Err("Next disease save is not due yet".into());
        }
        let yes = fort(
            &s,
            &c.target,
            c.rule.dc,
            r.save,
            conditional_save(self, d, &c.target, true)?,
            &mut Vec::new(),
        )?;
        let mut facts = vec![format!(
            "{}: {} daily save {}",
            c.target,
            c.rule.name,
            if yes { "succeeded" } else { "failed" }
        )];
        if !yes {
            damage(
                &mut s,
                &c.target,
                &c.rule,
                r,
                conditional_save(self, d, &c.target, true)?,
                &mut facts,
            )?;
        }
        let current = &mut s.necromancy.diseases[index];
        current.successes = if yes { current.successes + 1 } else { 0 };
        current.due += DAY;
        if current.successes >= 2 {
            s.necromancy.diseases.remove(index);
            facts.push(
                "Two consecutive successes: disease cured; existing ability damage remains".into(),
            );
        }
        finish(s, facts)
    }
    pub fn cure_disease(&self, d: &CharacterDraft, index: usize) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        if index >= s.necromancy.diseases.len() {
            return Err("Unknown disease case".into());
        }
        s.necromancy.diseases.remove(index);
        finish(
            s,
            vec!["External disease cure recorded; ability damage and drain remain".into()],
        )
    }
    pub fn rebuke_undead(
        &self,
        d: &CharacterDraft,
        check: u32,
        dice: [u32; 2],
        _targets: &[String],
    ) -> Result<PlayResult, String> {
        if !(1..=20).contains(&check) || dice.iter().any(|n| !(1..=6).contains(n)) {
            return Err("Supply a d20 and two d6 rolls".into());
        }
        let snap = self.evaluate(d);
        if !snap.is_available() {
            return Err("Character snapshot unavailable".into());
        }
        let t = snap.turning.ok_or("No turning ability")?;
        if t.mode != crate::TurningMode::Rebuke {
            return Err("This action requires rebuking".into());
        }
        let mut s = d.class_play.clone();
        if s.resources.turning_spent >= t.times_per_day.max(0) as u32 {
            return Err("No rebuking attempts remaining".into());
        }
        let origin = s.actors.get("self").ok_or("Register self")?;
        if origin.dead || origin.unconscious {
            return Err("User cannot act".into());
        }
        let all_targets: Vec<_> = s
            .actors
            .iter()
            .filter(|(id, a)| {
                id.as_str() != "self"
                    && a.body == TargetBody::Undead
                    && !a.dead
                    && !s.necromancy.commands.contains_key(*id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let targets = &all_targets;
        let cha = (snap.stats.get("CHA").ok_or("CHA unavailable")? - 10).div_euclid(2);
        let result = check as i32 + cha;
        let offset = if result <= 0 {
            -4
        } else if result >= 22 {
            4
        } else {
            (result - 1) / 3 - 3
        };
        let max_hd = (t.level as i32 + offset).max(0) as u32;
        let mut budget = (dice[0] as i32 + dice[1] as i32 + t.damage_bonus).max(0) as u32;
        let mut eligible = vec![];
        let mut unique = std::collections::BTreeSet::new();
        for id in targets {
            if !unique.insert(id) {
                return Err("Duplicate rebuking target".into());
            }
            require_actor(&s, id)?;
            let a = s.actors.get(id).ok_or("Unknown target")?;
            if a.body != TargetBody::Undead || a.dead {
                return Err("Select active undead only".into());
            }
            if crate::targeting::distance(origin, a) > 60
                || !matches!(a.path, TargetPath::Clear | TargetPath::Obscured)
            {
                continue;
            }
            let hd = s
                .encounter
                .combat
                .get(id)
                .ok_or("Record undead HD")?
                .hit_dice;
            let resistance = s
                .necromancy
                .actors
                .get(id)
                .ok_or("Record undead turn resistance")?
                .turn_resistance;
            eligible.push((
                crate::targeting::distance(origin, a),
                id.clone(),
                hd + resistance,
                hd,
            ));
        }
        eligible.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut facts = vec![format!(
            "Rebuking: maximum effective HD {max_hd}; HD budget {budget}"
        )];
        for (_, id, effective, _hd) in eligible {
            if effective > max_hd || effective > budget {
                continue;
            }
            budget -= effective;
            let used: u32 = s.necromancy.commands.values().sum();
            if t.level >= 2 * effective
                && used - s.necromancy.commands.get(&id).copied().unwrap_or(0) + effective
                    <= t.level
            {
                s.necromancy.commands.insert(id.clone(), effective);
                facts.push(format!(
                    "{id}: commanded indefinitely (separate rebuking pool)"
                ));
            } else {
                s.encounter
                    .conditions
                    .retain(|c| !(c.target == id && c.source == "Rebuke Undead"));
                s.encounter.conditions.push(EncounterCondition {
                    source: "Rebuke Undead".into(),
                    target: id.clone(),
                    name: "cowering".into(),
                    remaining_seconds: Some(60),
                    aura_radius_feet: None,
                });
                facts.push(format!(
                    "{id}: cowering for 10 rounds; attacks against it gain +2"
                ));
            }
        }
        s.resources.turning_spent += 1;
        finish(s, facts)
    }
    pub fn release_rebuked_undead(
        &self,
        d: &CharacterDraft,
        target: &str,
    ) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        s.necromancy
            .commands
            .remove(target)
            .ok_or("Target is not commanded")?;
        finish(s, vec![format!("{target}: command relinquished")])
    }
}
impl Engine {
    /// A permanent level loss edits the sole advancement history, never an offset level.
    pub fn resolve_negative_level(
        &self,
        d: &CharacterDraft,
        index: usize,
        roll: u32,
        removed_ranks: &BTreeMap<String, f64>,
        lost_ability: Option<&str>,
        lost_feat: Option<&str>,
        external_hp_loss: Option<u32>,
    ) -> Result<(CharacterDraft, Vec<String>), String> {
        let mut next = d.clone();
        let mut s = d.class_play.clone();
        let n = s
            .necromancy
            .negative_levels
            .get(index)
            .ok_or("Unknown negative level")?
            .clone();
        if s.encounter.seconds < n.due {
            return Err("The 24-hour save is not due yet".into());
        }
        let mut facts = vec![];
        let success = fort(
            &s,
            &n.target,
            n.dc,
            Some(roll),
            conditional_save(self, d, &n.target, false)?,
            &mut facts,
        )?;
        s.necromancy.negative_levels.remove(index);
        let hp = s.health.get_mut(&n.target).ok_or("HP unavailable")?;
        hp.maximum += 5;
        hp.current += 5;
        if !success {
            if n.target == "self" {
                if d.is_gestalt() {
                    return Err("Permanent level loss requires an explicit lane history; this action handles single-class progression".into());
                }
                let frame = next.advancement.pop().ok_or("No level to lose")?;
                if next.advancement.is_empty() {
                    s.actors.get_mut("self").ok_or("Register self")?.dead = true;
                } else {
                    let lane = frame.lanes.first().ok_or("Missing advancement lane")?;
                    let class = self
                        .pack
                        .classes
                        .get(&lane.class_id)
                        .ok_or("Missing class definition")?;
                    let int = next.stats.get("INT").ok_or("INT unavailable")?;
                    let points = (class.skill_points_per_level as i32 + (int - 10).div_euclid(2))
                        .max(1) as f64
                        + if next.race_id == "Human" { 1.0 } else { 0.0 };
                    let class_skills = crate::effects::class_skill_names(&self.pack, d);
                    let mut used = 0.0;
                    let mut allocation = removed_ranks.clone();
                    if allocation.values().all(|n| *n == 0.0) {
                        let mut remaining = points;
                        let mut ranked: Vec<_> = next
                            .skills
                            .iter()
                            .map(|(name, ranks)| (name.clone(), *ranks))
                            .collect();
                        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                        for (name, ranks) in ranked {
                            let cost = if class_skills.iter().any(|x| x.eq_ignore_ascii_case(&name))
                            {
                                1.0
                            } else {
                                2.0
                            };
                            let lost = ranks.min(remaining / cost);
                            allocation.insert(name, lost);
                            remaining -= lost * cost;
                            if remaining <= 0.0 {
                                break;
                            }
                        }
                        facts.push("Unknown skill history: RAW fallback removes ranks from highest-ranked skills".into());
                    }
                    for (name, ranks) in &allocation {
                        let existing = next
                            .skills
                            .get_mut(name)
                            .ok_or("Unknown skill in level-loss allocation")?;
                        if !ranks.is_finite() || *ranks < 0.0 || *ranks > *existing {
                            return Err("Invalid removed skill ranks".into());
                        }
                        used += ranks
                            * if class_skills.iter().any(|x| x.eq_ignore_ascii_case(name)) {
                                1.0
                            } else {
                                2.0
                            };
                        *existing -= ranks;
                    }
                    if used > points {
                        return Err(format!(
                            "Removed ranks cost {used}; last level supplied {points} skill points"
                        ));
                    }
                    if frame.character_level % 4 == 0 {
                        let key = lost_ability
                            .map(str::to_string)
                            .or_else(|| {
                                next.stats
                                    .iter()
                                    .max_by_key(|(_, v)| *v)
                                    .map(|(k, _)| k.clone())
                            })
                            .ok_or("Ability increase history unavailable")?;
                        *next.stats.get_mut(&key).ok_or("Unknown lost ability")? -= 1;
                    }
                    let gained: Vec<_> = next
                        .feat_acquired_at
                        .iter()
                        .filter(|(_, level)| **level == frame.character_level)
                        .map(|(name, _)| name.clone())
                        .collect();
                    if frame.character_level % 3 == 0
                        && gained.is_empty()
                        && lost_feat.is_none()
                        && !next.abilities.is_empty()
                    {
                        return Err("Select the feat acquired at the lost level".into());
                    }
                    for name in gained.iter().map(String::as_str).chain(lost_feat) {
                        next.abilities.retain(|(_, f)| f != name);
                        next.feat_acquired_at.remove(name);
                    }
                    let level = next.advancement.len() as i32;
                    let midpoint = 500 * level * level;
                    let previous: i32 = next
                        .advancement
                        .iter()
                        .take(next.advancement.len() - 1)
                        .flat_map(|f| &f.lanes)
                        .map(|l| l.experience_gained)
                        .sum();
                    next.advancement.last_mut().unwrap().lanes[0].experience_gained =
                        midpoint - previous;
                    let learning_prefix = format!("{}.AdvancedLearning.", lane.class_id);
                    next.choices.retain(|key, _| {
                        key.strip_prefix(&learning_prefix)
                            .and_then(|n| n.parse::<u32>().ok())
                            .is_none_or(|n| n < lane.class_level)
                    });
                    next.class_play = s.clone();
                    let before_snapshot = self.evaluate(d);
                    let after_snapshot = self.evaluate(&next);
                    if !before_snapshot.is_available() || !after_snapshot.is_available() {
                        return Err(format!(
                            "Permanent level-loss evaluation unavailable: {}",
                            after_snapshot.diagnostics.join("; ")
                        ));
                    }
                    if let Some(defense) = s.defenses.get_mut("self") {
                        for (key, total) in [
                            ("Fortitude", &mut defense.fortitude),
                            ("Reflex", &mut defense.reflex),
                            ("Will", &mut defense.will),
                        ] {
                            let before = *before_snapshot
                                .saves
                                .get(key)
                                .ok_or("Previous save unavailable")?
                                + 1;
                            let after = *after_snapshot
                                .saves
                                .get(key)
                                .ok_or("Post-loss save unavailable")?;
                            *total = total
                                .checked_add(after - before)
                                .ok_or("Save total overflow")?;
                        }
                    }
                    let change =
                        i64::from(after_snapshot.hit_points - (before_snapshot.hit_points + 5));
                    let hp = s.health.get_mut("self").unwrap();
                    hp.maximum += change;
                    hp.current += change;
                    if let Some(a) = s.necromancy.actors.get_mut("self") {
                        a.scores = next.stats.clone();
                    }
                    if let Some(c) = s.encounter.combat.get_mut("self") {
                        c.hit_dice = c.hit_dice.saturating_sub(1);
                    }
                }
            } else {
                let loss = external_hp_loss
                    .filter(|n| *n > 0 && *n <= 1000)
                    .ok_or("Supply the HP gained at the victim's most recent level")?;
                let hp = s.health.get_mut(&n.target).unwrap();
                hp.maximum -= i64::from(loss);
                hp.current -= i64::from(loss);
                let c = s
                    .encounter
                    .combat
                    .get_mut(&n.target)
                    .ok_or("Record victim HD")?;
                c.hit_dice = c.hit_dice.saturating_sub(1);
                let a = s
                    .necromancy
                    .actors
                    .get_mut(&n.target)
                    .ok_or("Record victim abilities")?;
                a.permanent_levels_lost += 1;
                a.needs_level_rebuild = true;
                if c.hit_dice == 0 {
                    s.actors.get_mut(&n.target).unwrap().dead = true;
                }
                facts.push("External actor lost a level: update its class-dependent saves, BAB and abilities from its own advancement history before further actions".into());
            }
            facts.push(
                "Failed save: negative level removed, most recent character level permanently lost"
                    .into(),
            );
        } else {
            facts.push("Negative level removed; 5 HP restored".into());
        }
        s.revision = s.revision.checked_add(1).ok_or("Revision overflow")?;
        next.class_play = s;
        Ok((next, facts))
    }
    pub fn recover_ability_damage(
        &self,
        d: &CharacterDraft,
        target: &str,
        bed_rest: bool,
    ) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        let a = s
            .necromancy
            .actors
            .get_mut(target)
            .ok_or("Record ability scores")?;
        if s.encounter.seconds < a.last_recovery + DAY {
            return Err("A full recovery day has not elapsed".into());
        }
        let before_con = score(&d.class_play, target, "CON")?;
        let was_comatose = ["INT", "WIS", "CHA"]
            .iter()
            .any(|key| score(&d.class_play, target, key) == Ok(0));
        let a = s.necromancy.actors.get_mut(target).unwrap();
        a.last_recovery += DAY;
        for n in a.damage.values_mut() {
            *n = n.saturating_sub(if bed_rest { 2 } else { 1 });
        }
        let after_con = score(&s, target, "CON")?;
        let hd = s.encounter.combat.get(target).ok_or("Record HD")?.hit_dice as i64;
        let delta =
            i64::from((after_con - 10).div_euclid(2) - (before_con - 10).div_euclid(2)) * hd;
        let a = s.necromancy.actors.get_mut(target).unwrap();
        let delta = delta.min(a.constitution_hp_loss);
        a.constitution_hp_loss -= delta;
        let hp = s.health.get_mut(target).ok_or("Record HP")?;
        hp.maximum += delta;
        hp.current += delta;
        let recovered = ["STR", "DEX", "CON", "INT", "WIS", "CHA"]
            .iter()
            .all(|key| score(&s, target, key).is_ok_and(|n| n > 0));
        if recovered {
            s.encounter
                .conditions
                .retain(|c| !(c.target == target && c.source == "Ability Score Loss"));
            if was_comatose && s.health.get(target).is_some_and(|h| h.current >= 0) {
                if let Some(a) = s.actors.get_mut(target) {
                    if !a.dead {
                        a.unconscious = false;
                    }
                }
            }
        }
        finish(
            s,
            vec!["One day of ability damage recovery; permanent drain is unchanged".into()],
        )
    }
    pub fn transform_dread_lich(
        &self,
        d: &CharacterDraft,
        rolls: &[u32],
    ) -> Result<(CharacterDraft, Vec<String>), String> {
        if d.class_play.necromancy.lich {
            return Err("Already transformed".into());
        }
        if !d
            .class_levels()
            .iter()
            .any(|c| c.class_id == "Dread Necromancer" && c.level >= 20)
        {
            return Err("Dread Necromancer level 20 required".into());
        }
        let humanoid = self.pack.races.get(&d.race_id).is_some_and(|r| {
            r.traits
                .race_type
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("Humanoid"))
                || r.type_tags
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("Humanoid"))
        });
        if !humanoid {
            return Err("Only humanoids gain this transformation".into());
        }
        let total = d.advancement.iter().map(|f| f.lanes.len()).sum::<usize>();
        if rolls.len() != total || rolls.iter().any(|n| !(1..=12).contains(n)) {
            return Err("Supply one d12 HP reroll for every existing Hit Die".into());
        }
        if d.class_play.actors.get("self").is_none_or(|a| a.dead) {
            return Err("Register a living humanoid before transformation".into());
        }
        let mut next = d.clone();
        for (l, n) in next
            .advancement
            .iter_mut()
            .flat_map(|f| &mut f.lanes)
            .zip(rolls)
        {
            l.hit_points_gained = Some(*n as i32);
        }
        next.class_play.necromancy.lich = true;
        next.stats.remove("CON");
        let actor = next
            .class_play
            .actors
            .get_mut("self")
            .ok_or("Register self")?;
        actor.body = TargetBody::Undead;
        actor.creature_type = "Undead".into();
        if let Some(a) = next.class_play.necromancy.actors.get_mut("self") {
            a.immune_disease = true;
            a.immune_drain = true;
        }
        next.class_play
            .necromancy
            .diseases
            .retain(|c| c.target != "self");
        next.class_play
            .necromancy
            .negative_levels
            .retain(|c| c.target != "self");
        let snapshot = self.evaluate(&next);
        if !snapshot.is_available() {
            return Err(format!(
                "Lich snapshot unavailable: {}",
                snapshot.diagnostics.join("; ")
            ));
        }
        next.class_play.defenses.insert(
            "self".into(),
            SpellDefenses {
                fortitude: *snapshot
                    .saves
                    .get("Fortitude")
                    .ok_or("Lich Fortitude unavailable")?,
                reflex: *snapshot
                    .saves
                    .get("Reflex")
                    .ok_or("Lich Reflex unavailable")?,
                will: *snapshot.saves.get("Will").ok_or("Lich Will unavailable")?,
                spell_resistance: next
                    .class_play
                    .defenses
                    .get("self")
                    .map(|d| d.spell_resistance)
                    .unwrap_or(0),
            },
        );
        let maximum = i64::from(snapshot.hit_points);
        let hp = next
            .class_play
            .health
            .get_mut("self")
            .ok_or("Record self HP")?;
        let damage = hp.maximum - hp.current;
        hp.maximum = maximum;
        hp.current = maximum - damage;
        if hp.current <= 0 {
            let actor = next.class_play.actors.get_mut("self").unwrap();
            actor.dead = true;
            actor.unconscious = true;
        }
        next.class_play.revision += 1;
        Ok((next,vec!["Humanoid transformed: undead type, no Constitution, all HD rerolled as d12; phylactery created at no gold or XP cost".into()]))
    }
}
impl Engine {
    pub fn resolve_undead_spell(
        &self,
        d: &CharacterDraft,
        r: &UndeadSpellRequest,
    ) -> Result<PlayResult, String> {
        let animate = r.cast.spell.eq_ignore_ascii_case("Animate Dead");
        if !animate && !r.cast.spell.eq_ignore_ascii_case("Control Undead") {
            return Err("Choose Animate Dead or Control Undead".into());
        }
        if r.targets.is_empty() {
            return Err("Select targets".into());
        }
        let preview = self.cast_preview(d, &r.cast)?;
        if !r.cast.metamagic.is_empty() || r.cast.versatile || r.cast.heighten_to.is_some() {
            return Err("This single-class action requires an unmodified casting".into());
        }
        let result = self.cast_and_record(d, &r.cast)?;
        let mut s = result.state;
        let mut facts = result.facts;
        let origin = s.actors.get("self").ok_or("Register self")?.clone();
        let mastery = self
            .class_feature_values(d)?
            .into_iter()
            .find(|f| f.class_id == "Dread Necromancer" && f.name == "Undead Mastery");
        let cl = u32::try_from(preview.caster_level).map_err(|_| "Negative caster level")?;
        let mut total = 0;
        let mut selected = std::collections::BTreeSet::new();
        let mut new_hd = BTreeMap::new();
        for id in &r.targets {
            if !selected.insert(id) {
                return Err("Duplicate target".into());
            }
            require_actor(&s, id)?;
            let a = s.actors.get(id).ok_or("Unknown target")?;
            if !matches!(a.path, TargetPath::Clear | TargetPath::Obscured) {
                return Err("Target line of effect blocked".into());
            }
            if animate {
                if !a.dead || a.body != TargetBody::Living || !r.intact_remains {
                    return Err(
                        "Animation requires intact, anatomically suitable non-undead remains"
                            .into(),
                    );
                }
                if crate::targeting::distance(&origin, a) > origin.touch_reach_feet {
                    return Err("Corpse is outside touch reach".into());
                }
                let racial = *r
                    .racial_hit_dice
                    .get(id)
                    .ok_or("Supply racial HD, excluding class levels, for each corpse")?;
                let hd = racial.max(1) * if r.zombie { 2 } else { 1 };
                if hd > 20 {
                    return Err(
                        "Animate Dead cannot create a skeleton or zombie above 20 HD".into(),
                    );
                }
                new_hd.insert(id.clone(), hd);
                total += hd;
            } else {
                if a.body != TargetBody::Undead || a.dead {
                    return Err("Control Undead requires active undead".into());
                }
                if crate::targeting::distance(&origin, a) > 25 + 5 * (cl / 2) {
                    return Err("Target outside Close range".into());
                }
                for other in &r.targets {
                    let b = s.actors.get(other).ok_or("Unknown target")?;
                    if crate::targeting::distance(a, b) > 30 {
                        return Err(
                            "Control Undead targets must be within 30 feet of each other".into(),
                        );
                    }
                }
                total += s
                    .encounter
                    .combat
                    .get(id)
                    .ok_or("Record target HD")?
                    .hit_dice;
            }
        }
        if animate {
            let create_cap = 2 * cl * if r.desecrated_altar { 2 } else { 1 };
            if total > create_cap {
                return Err(format!("Creation budget exceeded: {total}/{create_cap} HD"));
            }
            let cost = total.checked_mul(25).ok_or("Material cost overflow")?;
            if r.onyx_gp < cost {
                return Err(format!("Provide {cost} gp of black onyx"));
            }
            for id in &r.release {
                if s.necromancy.animated.get(id).is_none() {
                    return Err("Only previous Animate Dead subjects may be released here".into());
                }
                s.necromancy.animated.remove(id);
            }
            let cap = match &mastery {
                Some(m) => u32::try_from(
                    *m.values
                        .get("animate_dead_control_hd")
                        .ok_or("Undead Mastery control cap unavailable")?,
                )
                .map_err(|_| "Negative control cap")?,
                None => 4 * cl,
            };
            let old: u32 = s.necromancy.animated.values().map(|b| b.hit_dice).sum();
            if old + total > cap {
                return Err(format!(
                    "Select previous subjects to release: control {}/{cap} HD",
                    old + total
                ));
            }
            for (id, hd) in new_hd {
                let rolled = *r
                    .hp_totals
                    .get(&id)
                    .filter(|n| **n >= hd && **n <= 12 * hd)
                    .ok_or("Supply a valid d12 HP total for each new undead")?;
                let a = s
                    .necromancy
                    .actors
                    .get_mut(&id)
                    .ok_or("Record corpse ability scores first")?;
                a.scores.remove("CON");
                a.scores.insert("INT".into(), 0);
                a.scores.insert("WIS".into(), 10);
                a.scores.insert("CHA".into(), 1);
                if r.zombie {
                    *a.scores.get_mut("STR").ok_or("STR unavailable")? += 2;
                    let dex = a.scores.get_mut("DEX").ok_or("DEX unavailable")?;
                    *dex = (*dex - 2).max(1);
                } else {
                    *a.scores.get_mut("DEX").ok_or("DEX unavailable")? += 2;
                }
                if mastery.is_some() {
                    *a.scores.get_mut("STR").unwrap() += 4;
                    *a.scores.get_mut("DEX").unwrap() += 4;
                }
                a.damage.clear();
                a.drain.clear();
                a.immune_disease = true;
                a.immune_drain = true;
                let maximum = i64::from(
                    rolled
                        + if mastery.is_some() { 2 * hd } else { 0 }
                        + if r.desecrated_altar {
                            if r.desecrate_altar {
                                2 * hd
                            } else {
                                hd
                            }
                        } else {
                            0
                        },
                );
                s.health.insert(
                    id.clone(),
                    HitPoints {
                        maximum,
                        current: maximum,
                        temporary: 0,
                        stable: true,
                    },
                );
                let actor = s.actors.get_mut(&id).unwrap();
                actor.dead = false;
                actor.unconscious = false;
                actor.body = TargetBody::Undead;
                actor.creature_type = "Undead".into();
                s.combat_state.reductions.insert(
                    id.clone(),
                    vec![pcgen_core::combat::PhysicalReduction {
                        amount: 5,
                        bypass: vec![vec![if r.zombie {
                            pcgen_core::combat::WeaponTrait::Slashing
                        } else {
                            pcgen_core::combat::WeaponTrait::Bludgeoning
                        }]],
                    }],
                );
                let c = s
                    .encounter
                    .combat
                    .get_mut(&id)
                    .ok_or("Record corpse combat facts")?;
                c.hit_dice = hd;
                c.intelligence = 0;
                c.base_attack_bonus = (hd / 2) as i32;
                c.fear_immune = true;
                let dex = (*a.scores.get("DEX").unwrap() - 10).div_euclid(2);
                c.touch_ac = 10 + dex;
                s.defenses.insert(
                    id.clone(),
                    SpellDefenses {
                        fortitude: (hd / 3) as i32,
                        reflex: (hd / 3) as i32 + dex,
                        will: 2 + (hd / 2) as i32,
                        spell_resistance: 0,
                    },
                );
                s.necromancy.animated.insert(
                    id.clone(),
                    UndeadBond {
                        source: "Animate Dead".into(),
                        hit_dice: hd,
                        expires: None,
                        orders: String::new(),
                    },
                );
                facts.push(format!(
                    "{id}: {} created, {hd}d12, {maximum} HP; Undead Mastery {}",
                    if r.zombie { "zombie" } else { "skeleton" },
                    if mastery.is_some() {
                        "+4 STR/DEX enhancement and +2 HP/HD"
                    } else {
                        "not acquired"
                    }
                ));
            }
            s.necromancy.onyx_spent_gp = s
                .necromancy
                .onyx_spent_gp
                .checked_add(cost)
                .ok_or("Onyx ledger overflow")?;
            facts.push(format!(
                "Consumed black onyx worth {cost} gp; indefinite Animate Dead control pool"
            ));
        } else {
            let cha = (self.evaluate(d).stats.get("CHA").ok_or("CHA unavailable")? - 10)
                .div_euclid(2)
                .max(0) as u32;
            let cap = cl * (2 + if mastery.is_some() { cha } else { 0 });
            if total > cap {
                return Err(format!(
                    "Control Undead target budget {total}/{cap} HD exceeded"
                ));
            }
            for id in &r.targets {
                let defense = s.defenses.get(id).ok_or("Record undead saves and SR")?;
                if defense.spell_resistance > 0 && !preview.bypass_sr {
                    let roll = r
                        .penetration
                        .get(id)
                        .copied()
                        .filter(|n| (1..=20).contains(n))
                        .ok_or("Supply SR caster-level d20")?;
                    if (roll as i32 + preview.caster_level + preview.penetration_bonus)
                        < defense.spell_resistance as i32
                    {
                        facts.push(format!("{id}: spell resisted"));
                        continue;
                    }
                }
                let roll = r
                    .saves
                    .get(id)
                    .copied()
                    .filter(|n| (1..=20).contains(n))
                    .ok_or("Supply Will d20 for every target")?;
                let bonus = defense.will + save_delta(&s, id, "WIS");
                if roll == 20 || roll != 1 && roll as i32 + bonus >= preview.save_dc {
                    facts.push(format!("{id}: Will save succeeded"));
                    continue;
                }
                let hd = s.encounter.combat[id].hit_dice;
                s.necromancy.spell_bonds.insert(
                    id.clone(),
                    UndeadBond {
                        source: "Control Undead".into(),
                        hit_dice: hd,
                        expires: Some(s.encounter.seconds + 60 * u64::from(cl)),
                        orders: String::new(),
                    },
                );
                facts.push(format!(
                    "{id}: controlled for {cl} minutes; understands spoken orders in any language"
                ));
            }
        }
        finish(s, facts)
    }
    pub fn order_undead(
        &self,
        d: &CharacterDraft,
        target: &str,
        order: &str,
    ) -> Result<PlayResult, String> {
        if order.trim().is_empty() || order.len() > 1000 {
            return Err("Enter a short order".into());
        }
        let mut s = d.class_play.clone();
        let origin = s.actors.get("self").ok_or("Register self")?;
        if origin.dead || origin.unconscious {
            return Err("Cannot issue orders while unable to act".into());
        }
        if s.necromancy.commands.contains_key(target) {
            return finish(
                s,
                vec![format!(
                    "Standard action: mental command to {target}: {order}"
                )],
            );
        }
        let bond = if s.necromancy.spell_bonds.contains_key(target) {
            s.necromancy.spell_bonds.get_mut(target)
        } else {
            s.necromancy.animated.get_mut(target)
        }
        .ok_or("No active control over this undead")?;
        if bond
            .expires
            .is_some_and(|until| until <= s.encounter.seconds)
        {
            return Err("Control expired".into());
        }
        bond.orders = order.into();
        finish(s, vec![format!("Spoken order to {target}: {order}")])
    }
}
impl Engine {
    pub fn rebuild_drained_actor(
        &self,
        d: &CharacterDraft,
        target: &str,
        scores: BTreeMap<String, i32>,
        combat: CombatFacts,
        defense: SpellDefenses,
    ) -> Result<PlayResult, String> {
        if target == "self" {
            return Err("Self level loss is derived directly from advancement".into());
        }
        let mut s = d.class_play.clone();
        let old = s.encounter.combat.get(target).ok_or("Record HD")?;
        if old.hit_dice != combat.hit_dice {
            return Err("Rebuilt HD must match the post-loss Hit Dice".into());
        }
        if ["STR", "DEX", "CON", "INT", "WIS", "CHA"]
            .iter()
            .any(|k| scores.get(*k).is_none_or(|n| !(1..=100).contains(n)))
        {
            return Err("All six post-loss baseline abilities are required".into());
        }
        let a = s
            .necromancy
            .actors
            .get_mut(target)
            .ok_or("Missing affliction facts")?;
        if !a.needs_level_rebuild {
            return Err("Actor does not require a level rebuild".into());
        }
        a.scores = scores;
        a.needs_level_rebuild = false;
        s.encounter.combat.insert(target.into(), combat);
        s.defenses.insert(target.into(), defense);
        finish(s,vec![format!("{target}: post-loss ability scores, BAB, saves and combat facts rebuilt; actor available")])
    }
}
