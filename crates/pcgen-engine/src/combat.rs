//! Shared physical mitigation and source-tracked spell state. No caller-supplied final damage.
use crate::{class_play::PlayResult, CharacterDraft, Engine};
use pcgen_core::{class_play::*, combat::*};
use std::collections::BTreeSet;
pub(crate) fn immune(s: &ClassPlay, target: &str, kind: BodyImmunity) -> bool {
    s.actors
        .get(target)
        .is_some_and(|a| a.body.immunities().contains(&kind) || a.body == TargetBody::Construct)
        || s.combat_state
            .immunities
            .get(target)
            .is_some_and(|a| a.contains(&kind))
}
pub(crate) fn condition(s: &ClassPlay, target: &str, name: &str) -> bool {
    s.encounter
        .conditions
        .iter()
        .any(|c| c.target == target && c.name == name)
        || s.combat_state.effects.values().any(|e| {
            e.target == target
                && e.expires > s.encounter.seconds
                && matches!(
                    (&e.condition, name),
                    (SpellCondition::Fatigued, "fatigued")
                        | (SpellCondition::Paralyzed, "paralyzed")
                        | (SpellCondition::Panicked, "panicked")
                        | (SpellCondition::Sickened, "sickened")
                )
        })
        || target == "self" && name == "fatigued" && s.encounter.fatigued
}
pub(crate) fn modified_score(s: &ClassPlay, target: &str, key: &str, base: i32) -> i32 {
    if condition(s, target, "paralyzed") && ["STR", "DEX"].contains(&key) {
        return 0;
    }
    let mut n = base;
    if ["STR", "DEX"].contains(&key) && condition(s, target, "fatigued") {
        n -= 2;
    }
    let mut penalty = 0;
    let mut floor = 0;
    for e in s
        .combat_state
        .effects
        .values()
        .filter(|e| e.target == target && e.expires > s.encounter.seconds)
    {
        if let SpellCondition::AbilityPenalty {
            abilities,
            amount,
            floor: f,
        } = &e.condition
        {
            if abilities.iter().any(|a| a == key) {
                penalty = penalty.max(*amount as i32);
                floor = floor.max(*f);
            }
        }
    }
    if penalty > 0 {
        n = (n - penalty).max(floor.min(n));
    }
    n.max(0)
}
pub(crate) fn apply_temporary(
    s: &mut ClassPlay,
    target: &str,
    source: &str,
    n: i64,
    expires: u64,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    let hp = s
        .health
        .get_mut(target)
        .ok_or("Temporary HP recipient HP unavailable")?;
    let entries = s.combat_state.temporary.entry(target.into()).or_default();
    let old = entries.get(source).map_or(0, |e| e.remaining);
    if n > old {
        hp.temporary = hp
            .temporary
            .checked_add(n - old)
            .ok_or("Temporary HP overflow")?;
        entries.insert(
            source.into(),
            TemporaryHP {
                remaining: n,
                expires,
            },
        );
    }
    facts.push(format!(
        "{target}: {source} temporary HP {} (same-source amounts do not stack)",
        n.max(old)
    ));
    Ok(())
}
pub(crate) fn consume_temporary(s: &mut ClassPlay, target: &str, mut n: i64) {
    if let Some(entries) = s.combat_state.temporary.get_mut(target) {
        let mut keys: Vec<_> = entries
            .iter()
            .map(|(key, e)| (e.expires, key.clone()))
            .collect();
        keys.sort();
        for (_, key) in keys {
            let e = entries.get_mut(&key).unwrap();
            let spent = n.min(e.remaining);
            e.remaining -= spent;
            n -= spent;
            if n == 0 {
                break;
            }
        }
    }
}
pub(crate) fn expire(s: &mut ClassPlay, facts: &mut Vec<String>) {
    let now = s.encounter.seconds;
    for (target, entries) in &mut s.combat_state.temporary {
        entries.retain(|name, e| {
            if e.expires <= now {
                if let Some(h) = s.health.get_mut(target) {
                    h.temporary = (h.temporary - e.remaining).max(0);
                }
                facts.push(format!("{target}: {name} temporary HP expired"));
                false
            } else {
                true
            }
        });
    }
    s.combat_state.effects.retain(|_, e| e.expires > now);
    s.combat_state.stench.retain(|_, e| e.expires > now);
    refresh_stench(s);
}
pub(crate) fn refresh_stench(s: &mut ClassPlay) {
    let mut present = BTreeSet::new();
    for (key, aura) in &s.combat_state.stench {
        let Some(origin) = s.actors.get(&aura.target).filter(|a| !a.dead) else {
            continue;
        };
        if !s.combat_state.effects.values().any(|e| {
            e.casting == aura.casting
                && e.target == aura.target
                && matches!(e.condition, SpellCondition::Paralyzed)
        }) {
            continue;
        }
        for (target, a) in &s.actors {
            if target == "self"
                || !a.living()
                || immune(s, target, BodyImmunity::Poison)
                || crate::targeting::distance(origin, a) > aura.radius_feet
            {
                continue;
            }
            let id = format!("{key}:{target}");
            present.insert(id);
        }
    }
    s.combat_state
        .exposures
        .retain(|id, e| e.saved.is_some() || present.contains(id));
    for id in present {
        if !s.combat_state.exposures.contains_key(&id) {
            if let Some((key, aura)) = s
                .combat_state
                .stench
                .iter()
                .find(|(key, _)| id.starts_with(&format!("{key}:")))
            {
                let target = id[key.len() + 1..].to_string();
                s.combat_state.exposures.insert(
                    id,
                    StenchExposure {
                        aura: key.clone(),
                        target,
                        saved: None,
                    },
                );
                let _ = aura;
            }
        }
    }
    s.combat_state
        .exposures
        .retain(|_, e| s.combat_state.stench.contains_key(&e.aura));
}
pub(crate) fn physical_after_dr(
    rules: &[PhysicalReduction],
    traits: &BTreeSet<WeaponTrait>,
    damage: i64,
    facts: &mut Vec<String>,
) -> i64 {
    let dr = rules
        .iter()
        .filter(|r| {
            !r.bypass.iter().any(|alternative| {
                !alternative.is_empty() && alternative.iter().all(|t| traits.contains(t))
            })
        })
        .map(|r| r.amount)
        .max()
        .unwrap_or(0);
    let result = (damage - i64::from(dr)).max(0);
    facts.push(format!(
        "Weapon damage {damage}; highest applicable DR {dr}; final weapon damage {result}"
    ));
    result
}
impl Engine {
    pub fn save_weapon_setup(
        &self,
        d: &CharacterDraft,
        mut setup: WeaponSetup,
    ) -> Result<PlayResult, String> {
        let eq = self
            .pack
            .equipment
            .get(&setup.equipment)
            .ok_or("Choose a known RAW weapon")?;
        if eq.damage.is_none()
            || !eq.type_tags.iter().any(|t| t.eq_ignore_ascii_case("Melee"))
            || setup.enhancement > 10
            || setup.alignments.iter().any(|a| {
                !matches!(
                    a,
                    WeaponTrait::Good
                        | WeaponTrait::Evil
                        | WeaponTrait::Lawful
                        | WeaponTrait::Chaotic
                )
            })
        {
            return Err("Invalid melee weapon properties".into());
        }
        if eq
            .wield
            .as_deref()
            .is_some_and(|w| w.eq_ignore_ascii_case("TwoHanded"))
        {
            setup.two_handed = true;
        }
        let mut s = d.class_play.clone();
        s.combat_state.weapon = Some(setup);
        crate::feature_resolution::finish(s,true,vec!["Weapon setup saved; dice, damage type and critical profile come from RAW equipment".into()])
    }
    pub fn save_physical_reduction(
        &self,
        d: &CharacterDraft,
        target: &str,
        rules: Vec<PhysicalReduction>,
    ) -> Result<PlayResult, String> {
        if !d.class_play.actors.contains_key(target)
            || rules.len() > 100
            || rules
                .iter()
                .any(|r| r.amount > 1000 || r.bypass.iter().any(|a| a.is_empty()))
        {
            return Err("Invalid DR facts".into());
        }
        let mut s = d.class_play.clone();
        s.combat_state.reductions.insert(target.into(), rules);
        crate::feature_resolution::finish(s,true,vec![format!("{target}: physical DR facts saved; alternatives and combined requirements are distinct")])
    }
    pub fn resolve_stench_exposure(
        &self,
        d: &CharacterDraft,
        id: &str,
        roll: u32,
    ) -> Result<PlayResult, String> {
        let e = d
            .class_play
            .combat_state
            .exposures
            .get(id)
            .ok_or("Unknown stench exposure")?;
        if e.saved.is_some() {
            return Err("Exposure already resolved".into());
        }
        let aura = d
            .class_play
            .combat_state
            .stench
            .get(&e.aura)
            .ok_or("Stench expired")?;
        let mut facts = vec![];
        // This save is the pending fact itself, so temporarily release the pending-actor guard.
        let mut input = d.clone();
        input
            .class_play
            .combat_state
            .exposures
            .get_mut(id)
            .unwrap()
            .saved = Some(false);
        let saved = crate::feature_resolution::saved(
            &input,
            &e.target,
            pcgen_core::magic::SaveKind::Fortitude,
            aura.dc,
            Some(roll),
            &mut facts,
        )?;
        let mut s = d.class_play.clone();
        s.combat_state.exposures.get_mut(id).unwrap().saved = Some(saved);
        if !saved {
            s.combat_state.effects.insert(
                id.into(),
                ActiveSpell {
                    casting: aura.casting.clone(),
                    name: "Ghoul Touch stench".into(),
                    target: e.target.clone(),
                    caster_level: aura.caster_level,
                    expires: aura.expires,
                    condition: SpellCondition::Sickened,
                },
            );
        }
        crate::feature_resolution::finish(s, true, facts)
    }
}
impl Engine {
    pub(crate) fn weapon_hit(
        &self,
        d: &CharacterDraft,
        strike: &pcgen_core::tactics::MeleeStrike,
        iterative: i32,
    ) -> Result<(ClassPlay, bool, Vec<String>), String> {
        let setup = d
            .class_play
            .combat_state
            .weapon
            .as_ref()
            .ok_or("Save a weapon setup before channeling")?;
        let eq = self
            .pack
            .equipment
            .get(&setup.equipment)
            .ok_or("Weapon definition unavailable")?;
        let parse = |text: &str| -> Result<(i64, i64), String> {
            let (n, s) = text.split_once('d').ok_or("Weapon dice unavailable")?;
            let n = n.parse::<i64>().map_err(|_| "Invalid weapon dice")?;
            let s = s.parse::<i64>().map_err(|_| "Invalid weapon die")?;
            if n < 1 || s < 2 || n > 100 || s > 100 {
                return Err("Unsupported weapon dice".into());
            }
            Ok((n, s))
        };
        let (n, sides) = parse(eq.damage.as_deref().ok_or("Weapon damage unavailable")?)?;
        if !(n..=n * sides).contains(&strike.weapon_dice_roll) || !(1..=20).contains(&strike.roll) {
            return Err(format!(
                "Supply weapon dice total {n}–{} and d20 1–20",
                n * sides
            ));
        }
        let snap = self.evaluate(d);
        if !snap.is_available() {
            return Err(snap.diagnostics.join("; "));
        }
        let origin = d.class_play.actors.get("self").ok_or("Register self")?;
        if condition(&d.class_play, "self", "paralyzed")
            || condition(&d.class_play, "self", "panicked")
        {
            return Err("This condition prevents channeling a melee attack".into());
        }
        let actor = d
            .class_play
            .actors
            .get(&strike.target)
            .ok_or("Unknown weapon target")?;
        crate::necromancy::require_actor(&d.class_play, &strike.target)?;
        if actor.dead
            || crate::targeting::distance(origin, actor) > origin.touch_reach_feet
            || !matches!(actor.path, TargetPath::Clear | TargetPath::Obscured)
        {
            return Err("Weapon target blocked, dead or beyond reach".into());
        }
        if condition(&d.class_play, &strike.target, "paralyzed") {
            crate::necromancy::score(&d.class_play, &strike.target, "DEX")?;
        }
        let ac = d
            .class_play
            .tactics
            .actors
            .get(&strike.target)
            .ok_or("Record target normal AC")?
            .armor_class
            + crate::necromancy::ability_delta(&d.class_play, &strike.target, "DEX");
        let masterwork = setup.masterwork || matches!(setup.material, WeaponMaterial::Adamantine);
        let effects = crate::effects::apply_effects(&self.pack, d);
        let normalized = |s: &str| s.trim().to_ascii_lowercase().replace(' ', "_");
        let proficient = effects
            .weapon_proficiencies
            .contains(&normalized(eq.proficiency.as_deref().unwrap_or(&eq.name)))
            || effects.weapon_proficiencies.contains(&normalized(&eq.name));
        let global_attack: [&str; 5] = [
            "ALL",
            "COMBAT.TOHIT",
            "COMBAT.TOHIT.MELEE",
            "TOHIT",
            "TOHIT.MELEE",
        ];
        let attack_bonus: i32 = global_attack
            .iter()
            .map(|key| snap.attack_bonuses.get(*key).copied().unwrap_or(0))
            .sum();
        let target_attack = snap
            .attack_bonuses
            .get(&format!("TARGET.{}.TOHIT.MORALE", strike.target))
            .copied()
            .unwrap_or(0);
        let target_damage = snap
            .attack_bonuses
            .get(&format!("TARGET.{}.DAMAGE.MORALE", strike.target))
            .copied()
            .unwrap_or(0);
        let bonus = snap.melee_attack + attack_bonus + target_attack
            - if proficient { 0 } else { 4 }
            + setup.enhancement.max(u32::from(masterwork)) as i32
            - iterative
            - if condition(&d.class_play, "self", "sickened") {
                2
            } else {
                0
            }
            + if condition(&d.class_play, &strike.target, "paralyzed") {
                4
            } else {
                0
            };
        let mut facts = vec![format!(
            "{}: {} d20 {} + {bonus} vs normal AC {ac}",
            strike.target, eq.name, strike.roll
        )];
        let event = self.record_challenge_attack(d, "self", &strike.target)?;
        let mut state = event.state;
        facts.extend(event.facts);
        if strike.roll == 1 || strike.roll != 20 && strike.roll as i32 + bonus < ac {
            facts.push("Weapon missed; no spell delivery".into());
            return Ok((state, false, facts));
        }
        let threat = 21 - eq.crit_range.ok_or("Weapon critical range unavailable")?;
        let mult = eq
            .crit_mult
            .as_deref()
            .ok_or("Critical multiplier unavailable")?
            .trim_start_matches('x')
            .parse::<i64>()
            .map_err(|_| "Invalid critical multiplier")?;
        let critical = if strike.roll as i32 >= threat
            && !immune(&state, &strike.target, BodyImmunity::CriticalHits)
        {
            let die = strike
                .confirmation_roll
                .filter(|n| (1..=20).contains(n))
                .ok_or("Weapon threat requires confirmation d20")?;
            die == 20 || die != 1 && die as i32 + bonus >= ac
        } else {
            false
        };
        let rolled = if critical {
            strike
                .critical_dice_roll
                .filter(|r| (n * mult..=n * sides * mult).contains(r))
                .ok_or("Confirmed critical requires the full multiplied weapon dice total")?
        } else {
            strike.weapon_dice_roll
        };
        let strength = (*snap.stats.get("STR").ok_or("STR unavailable")? - 10).div_euclid(2);
        let str_damage = if setup.two_handed
            && strength > 0
            && !eq
                .wield
                .as_deref()
                .is_some_and(|w| w.eq_ignore_ascii_case("Light"))
        {
            strength * 3 / 2
        } else {
            strength
        };
        let mut traits = BTreeSet::new();
        for (tag, kind) in [
            ("Bludgeoning", WeaponTrait::Bludgeoning),
            ("Piercing", WeaponTrait::Piercing),
            ("Slashing", WeaponTrait::Slashing),
        ] {
            if eq.type_tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                traits.insert(kind);
            }
        }
        if traits.is_empty() {
            return Err("RAW physical weapon damage types unavailable".into());
        }
        if setup.enhancement >= 1 {
            traits.insert(WeaponTrait::Magic);
        }
        if setup.enhancement >= 6 {
            traits.insert(WeaponTrait::Epic);
        }
        traits.extend(setup.alignments.iter().copied());
        match setup.material {
            WeaponMaterial::Silver => {
                traits.insert(WeaponTrait::Silver);
            }
            WeaponMaterial::ColdIron => {
                traits.insert(WeaponTrait::ColdIron);
            }
            WeaponMaterial::Adamantine => {
                traits.insert(WeaponTrait::Adamantine);
            }
            _ => {}
        }
        let silver = i64::from(
            setup.material == WeaponMaterial::Silver
                && (traits.contains(&WeaponTrait::Slashing)
                    || traits.contains(&WeaponTrait::Piercing)),
        );
        let multiplier = if critical { mult } else { 1 };
        let damage = (rolled
            + (i64::from(str_damage) + i64::from(setup.enhancement) + i64::from(target_damage)
                - silver)
                * multiplier
            - if condition(&state, "self", "sickened") {
                2 * multiplier
            } else {
                0
            })
        .max(1);
        let rules = state
            .combat_state
            .reductions
            .get(&strike.target)
            .ok_or("Record target DR (an explicit empty list means no DR)")?;
        let amount = physical_after_dr(rules, &traits, damage, &mut facts);
        let fort = crate::feature_resolution::save_bonus(
            d,
            &strike.target,
            pcgen_core::magic::SaveKind::Fortitude,
        )?;
        if amount > 0 {
            state.tactics.wounded_in_melee.insert(strike.target.clone());
            crate::encounter_health::apply_damage(
                &mut state,
                &strike.target,
                amount,
                strike.spell.massive_roll,
                fort,
                &mut facts,
            )?;
        }
        facts.push("Weapon hit; touch-spell resolution is independent of physical DR".into());
        Ok((state, true, facts))
    }
}
