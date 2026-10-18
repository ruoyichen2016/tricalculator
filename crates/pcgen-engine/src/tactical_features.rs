//! Executable PHB II tactical actions use shared saves, mitigation and HP transitions.
use crate::{class_features::ResolvedClassFeature, class_play::PlayResult, CharacterDraft, Engine};
use pcgen_core::{class_play::*, magic::*};
impl Engine {
    pub(crate) fn resolve_breath(
        &self,
        d: &CharacterDraft,
        r: &FeatureRequest,
        f: &ResolvedClassFeature,
    ) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        let mut facts = vec!["Breath Weapon: standard action".into()];
        let key = format!("{}: {}", f.class_id, f.name);
        if s.encounter
            .cooldowns
            .get(&key)
            .is_some_and(|until| *until > s.encounter.seconds)
        {
            return Err("Breath weapon is recharging".into());
        }
        let m =
            crate::magic::class_magic(&self.pack, &f.class_id).ok_or("Class magic unavailable")?;
        let rule = m
            .choices
            .iter()
            .find(|c| c.options.iter().any(|o| o.breath.is_some()))
            .and_then(|c| {
                d.choices
                    .get(&c.key)
                    .and_then(|selected| c.options.iter().find(|o| &o.value == selected))
            })
            .and_then(|o| o.breath.as_ref())
            .ok_or("Choose a valid totem dragon")?;
        let value = |key: &str| {
            f.values
                .get(key)
                .copied()
                .ok_or_else(|| format!("Breath rule missing {key}"))
        };
        let dice = i64::from(value("damage_d6")?);
        let amount = r
            .damage_roll
            .filter(|n| *n >= dice && *n <= 6 * dice)
            .ok_or("Supply a valid shared breath damage roll")?;
        let recharge = r
            .opposed_roll
            .filter(|n| (1..=4).contains(n))
            .ok_or("Supply a recharge d4")?;
        let range = value(if rule.cone { "cone_feet" } else { "line_feet" })? as u32;
        let direction = match r.target.as_str() {
            "east" => (1i64, 0i64),
            "west" => (-1, 0),
            "north" => (0, -1),
            "south" => (0, 1),
            _ => return Err("Choose a cardinal breath direction".into()),
        };
        let origin = s.actors.get("self").ok_or("Register self")?.clone();
        if origin.space != 1 {
            return Err("Choose a 5-foot origin before resolving this breath template".into());
        }
        let targets: Vec<_> = s
            .actors
            .iter()
            .filter(|(id, a)| {
                if id.as_str() == "self"
                    || a.dead
                    || !matches!(a.path, TargetPath::Clear | TargetPath::Obscured)
                {
                    return false;
                }
                (0..a.space).any(|x| {
                    (0..a.space).any(|y| {
                        let dx = i64::from(a.x) + i64::from(x) - i64::from(origin.x);
                        let dy = i64::from(a.y) + i64::from(y) - i64::from(origin.y);
                        let forward = dx * direction.0 + dy * direction.1;
                        let side = dx * direction.1 - dy * direction.0;
                        let distance = (dx.abs().max(dy.abs()) + dx.abs().min(dy.abs()) / 2) * 5;
                        forward > 0
                            && distance <= i64::from(range)
                            && if rule.cone {
                                side.abs() <= forward
                            } else {
                                side == 0
                            }
                    })
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        for target in targets {
            crate::necromancy::require_actor(&s, &target)?;
            let actor = s.actors[&target].clone();
            let p = s
                .protections
                .get(&target)
                .ok_or("Record target energy protections and evasion")?;
            let immune = p.immunity.contains(&rule.energy);
            let saved = if immune {
                false
            } else {
                crate::feature_resolution::saved(
                    d,
                    &target,
                    SaveKind::Reflex,
                    value("reflex_dc")?,
                    r.saves.get(&target).copied(),
                    &mut facts,
                )?
            };
            let evade = crate::encounter_health::evasion_applies(&actor, p);
            let n = if saved && evade && p.evasion != Evasion::None {
                0
            } else if saved || evade && p.evasion == Evasion::Improved {
                amount / 2
            } else {
                amount
            };
            let n = crate::encounter_health::energy_after_resistance(p, rule.energy, n, &mut facts);
            if n > 0 {
                let fort = crate::feature_resolution::save_bonus(d, &target, SaveKind::Fortitude)?;
                crate::encounter_health::apply_damage(
                    &mut s,
                    &target,
                    n,
                    r.massive_rolls.get(&target).copied(),
                    fort,
                    &mut facts,
                )?;
            }
        }
        s.encounter
            .cooldowns
            .insert(key, s.encounter.seconds + u64::from(recharge) * 6);
        facts.push(format!(
            "Shared {}d6 roll {amount}; {:?} {}; recharge {recharge} rounds",
            dice,
            rule.energy,
            if rule.cone { "cone" } else { "line" }
        ));
        crate::feature_resolution::finish(s, true, facts)
    }
}
use pcgen_core::tactics::*;
impl Engine {
    pub fn save_tactical_facts(
        &self,
        d: &CharacterDraft,
        target: &str,
        f: TacticalFacts,
    ) -> Result<PlayResult, String> {
        if !d.class_play.actors.contains_key(target)
            || !f.challenge_rating.is_finite()
            || !(0.0..=1000.0).contains(&f.challenge_rating)
            || f.armor_class.unsigned_abs() > 1000
        {
            return Err("Register actor and bounded CR / AC".into());
        }
        let mut s = d.class_play.clone();
        s.tactics.actors.insert(target.into(), f);
        crate::feature_resolution::finish(s, true, vec![format!("{target}: tactical facts saved")])
    }
    pub(crate) fn resolve_knight_challenge(
        &self,
        d: &CharacterDraft,
        r: &FeatureRequest,
        f: &ResolvedClassFeature,
        mode: ChallengeMode,
    ) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        let mut facts = vec![format!("{}: swift action", f.name)];
        let resource = self
            .class_feature_values(d)?
            .into_iter()
            .find(|f| f.class_id == "Knight" && f.name == "Knight's Challenge")
            .ok_or("Knight's Challenge not acquired")?;
        let cap = resource
            .daily_uses
            .ok_or("Challenge daily budget unavailable")?
            + s.tactics.extra_challenge;
        let spent = s
            .resources
            .daily
            .entry("Knight: Knight's Challenge".into())
            .or_default();
        if *spent >= cap {
            return Err("No knight's challenges remaining".into());
        }
        *spent += 1;
        let origin = s.actors.get("self").ok_or("Register self")?.clone();
        let now = s.encounter.seconds;
        let targets = if mode == ChallengeMode::Fighting {
            if let Some(active) = &s.tactics.fighting {
                if active.expires > now
                    && !s.actors.get(&active.target).is_none_or(|a| a.dead)
                    && !s.tactics.actors.get(&active.target).is_some_and(|a| a.fled)
                {
                    return Err("The original fighting challenge foe is still active".into());
                }
            }
            vec![r.target.clone()]
        } else {
            s.actors
                .iter()
                .filter(|(id, a)| {
                    id.as_str() != "self"
                        && !a.allied
                        && !a.dead
                        && a.path == TargetPath::Clear
                        && crate::targeting::distance(&origin, a) <= 100
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        let duration = *f
            .values
            .get("duration_rounds")
            .ok_or("Challenge duration unavailable")? as u64
            * 6;
        for target in targets {
            crate::necromancy::require_actor(&s, &target)?;
            let actor = s.actors.get(&target).ok_or("Unknown challenge target")?;
            let c = s
                .encounter
                .combat
                .get(&target)
                .ok_or("Record target Intelligence")?;
            let t = s
                .tactics
                .actors
                .get(&target)
                .ok_or("Record target challenge rating and language")?;
            let strong = t.challenge_rating >= f64::from(d.total_level()) - 2.0;
            let eligible = c.intelligence >= 5
                && t.language
                && !actor.dead
                && !t.fled
                && if mode == ChallengeMode::Daunting {
                    !strong && t.hears
                } else {
                    strong
                };
            if !eligible {
                facts.push(format!(
                    "{target}: ineligible; challenge expenditure remains spent"
                ));
                continue;
            }
            if mode == ChallengeMode::Fighting {
                let bonus = *f
                    .values
                    .get("fighting_bonus")
                    .ok_or("Fighting bonus unavailable")?;
                s.tactics.fighting = Some(FightingChallenge {
                    target: target.clone(),
                    bonus,
                    expires: now + duration,
                });
                facts.push(format!("{target}: +{bonus} morale to attacks and damage against this foe; +{bonus} morale to Will saves"));
                continue;
            }
            let used = if mode == ChallengeMode::Mettle {
                &mut s.tactics.mettle_used
            } else {
                &mut s.tactics.daunting_used
            };
            if !used.insert(target.clone()) {
                facts.push(format!(
                    "{target}: already targeted by this challenge today"
                ));
                continue;
            }
            if mode == ChallengeMode::Daunting
                && (c.fear_immune || actor.body == TargetBody::Undead)
            {
                facts.push(format!("{target}: fear immune"));
                continue;
            }
            if !crate::feature_resolution::saved(
                d,
                &target,
                SaveKind::Will,
                *f.values.get("will_dc").ok_or("Challenge DC unavailable")?,
                r.saves.get(&target).copied(),
                &mut facts,
            )? {
                if mode == ChallengeMode::Mettle {
                    s.tactics.mettle.insert(target.clone(), now + duration);
                    facts.push(format!("{target}: must prefer attacking the knight; ends if another actor attacks it"));
                } else {
                    s.encounter.conditions.push(EncounterCondition {
                        source: "Daunting Challenge".into(),
                        target: target.clone(),
                        name: "shaken".into(),
                        remaining_seconds: Some(duration as u32),
                        aura_radius_feet: None,
                    });
                }
            }
        }
        crate::feature_resolution::finish(s, true, facts)
    }
    /// Record who attacked whom, including attacks that miss; Test of Mettle ends on attack, not damage.
    pub fn record_challenge_attack(
        &self,
        d: &CharacterDraft,
        attacker: &str,
        target: &str,
    ) -> Result<PlayResult, String> {
        let mut s = d.class_play.clone();
        if !s.actors.contains_key(attacker) || !s.actors.contains_key(target) {
            return Err("Register both actors")?;
        }
        let mut facts = vec![format!("Attack event: {attacker} -> {target}")];
        if attacker != "self" && s.tactics.mettle.remove(target).is_some() {
            facts.push("Test of Mettle ended for the attacked target".into());
        }
        if target == "self" && s.health.get("self").is_some_and(|h| h.current <= 0) {
            if s.tactics
                .fighting
                .as_ref()
                .is_some_and(|c| c.target == attacker)
            {
                *s.resources
                    .daily
                    .entry("Knight: Knight's Challenge".into())
                    .or_default() += 2;
                s.tactics.fighting = None;
                facts.push("Defeat by challenged foe: two additional challenge uses lost".into());
            }
            if s.tactics.mettle.contains_key(attacker) && !s.tactics.mettle_reward {
                s.tactics.mettle_reward = true;
                s.tactics.extra_challenge = 1;
                facts.push(
                    "Test of Mettle defeat: one additional daily use gained, once per day".into(),
                );
            }
        }
        crate::feature_resolution::finish(s, true, facts)
    }
    pub fn arcane_channel(
        &self,
        d: &CharacterDraft,
        r: &ChannelRequest,
    ) -> Result<PlayResult, String> {
        let f = self
            .class_feature_values(d)?
            .into_iter()
            .find(|f| f.class_id == "Duskblade" && f.name == "Arcane Channeling")
            .ok_or("Arcane Channeling not acquired")?;
        let snap = self.evaluate(d);
        if !snap.is_available() {
            return Err("Character snapshot unavailable".into());
        }
        let held = if r.resume {
            Some(
                d.class_play
                    .tactics
                    .channel
                    .as_ref()
                    .ok_or("No held charge")?,
            )
        } else {
            None
        };
        if held.is_some_and(|c| c.mode == pcgen_core::tactics::ChargeMode::Touch) {
            return Err(
                "Ordinary touch charges use the held-touch form, not weapon channeling".into(),
            );
        }
        if r.opportunity && (!r.resume || r.strikes.len() != 1) {
            return Err(
                "An opportunity delivers one existing charge through one provoked attack".into(),
            );
        }
        let full = held.map_or(r.full_attack, |c| {
            c.mode == pcgen_core::tactics::ChargeMode::FullChannel
        });
        if full && f.values.get("full_attack") != Some(&1) {
            return Err("Full-attack channeling requires Duskblade level 13".into());
        }
        let max = if full {
            1 + (snap.base_attack_bonus.saturating_sub(1) / 5) as usize
        } else {
            1
        };
        if r.strikes.is_empty() && !(r.resume && r.finish_attacks && full) || r.strikes.len() > max
        {
            return Err("Supply the normal iterative melee attacks for this action".into());
        }
        let mut facts=vec!["Arcane Channeling: casting does not provoke; weapon damage resolves before the touch spell".into()];
        let mut current = d.clone();
        let channel = if r.resume {
            let held = d
                .class_play
                .tactics
                .channel
                .clone()
                .ok_or("No held channeling charge")?;
            if held
                .expires
                .is_some_and(|until| until <= d.class_play.encounter.seconds)
            {
                return Err("Channeling expired".into());
            }
            held
        } else {
            let spell = self
                .pack
                .spells
                .values()
                .find(|s| s.name.eq_ignore_ascii_case(&r.cast.spell))
                .ok_or("Unknown touch spell")?;
            if spell.profile.range != SpellRange::Touch
                || !matches!(
                    spell.profile.casting_time,
                    Some(
                        SpellCastingTime::Standard
                            | SpellCastingTime::Swift
                            | SpellCastingTime::Immediate
                    )
                )
            {
                return Err("Channeling requires a known touch spell with casting time at most one standard action".into());
            }
            if !r.cast.metamagic.is_empty() || r.cast.heighten_to.is_some() {
                return Err(
                    "Spontaneous metamagic increases casting time beyond the channeling limit"
                        .into(),
                );
            }
            if r.cast.quick_cast || r.cast.quiescent {
                return Err(
                    "Channeling uses its own action and cannot use Quick Cast or quiescent storage"
                        .into(),
                );
            }
            if spell.profile.defense.is_none()
                || (spell.profile.impact.is_none()
                    && (spell.profile.hit_point_damage != Some(true)
                        || spell.profile.damage.is_none()
                        || (spell.profile.damage_kind.is_none() && spell.profile.energy.is_none())))
            {
                return Err(
                    "Touch spell resolution unavailable; no partial action committed".into(),
                );
            }
            let result = self.cast_and_record(d, &r.cast)?;
            let key = result
                .state
                .cast_effects
                .keys()
                .find(|k| !d.class_play.cast_effects.contains_key(*k))
                .ok_or("New casting record unavailable")?
                .clone();
            current.class_play = result.state;
            facts.extend(result.facts);
            ChannelState {
                casting: key,
                mode: if full {
                    pcgen_core::tactics::ChargeMode::FullChannel
                } else {
                    pcgen_core::tactics::ChargeMode::StandardChannel
                },
                remaining: 1,
                attacks_used: 0,
                attack_limit: max as u32,
                attacks_closed: false,
                expires: if full {
                    Some(current.class_play.encounter.seconds + 6)
                } else {
                    None
                },
                targets: Default::default(),
            }
        };
        let mut channel = channel;
        if full
            && !r.opportunity
            && !r.strikes.is_empty()
            && (channel.attacks_closed
                || channel.attacks_used + r.strikes.len() as u32 > channel.attack_limit)
        {
            return Err("Full-attack sequence closed or iterative attacks exhausted; the index cannot be reset by another submission".into());
        }
        current.class_play.tactics.channel = Some(channel.clone());
        if r.opportunity {
            let reflexes = self.pack.entities.iter().any(|v| {
                v.name.eq_ignore_ascii_case("Combat Reflexes")
                    && crate::effects::ability_matches_draft(d, v)
            });
            let dex = (*snap.stats.get("DEX").ok_or("DEX unavailable")? - 10)
                .div_euclid(2)
                .max(0) as u32;
            let limit = 1 + if reflexes { dex } else { 0 };
            let epoch = current.class_play.encounter.seconds / 6;
            if current.class_play.tactics.opportunity_round != epoch {
                current.class_play.tactics.opportunity_round = epoch;
                current.class_play.tactics.opportunities_used = 0;
            }
            if current.class_play.tactics.opportunities_used >= limit {
                return Err("Opportunity attack budget exhausted for this round".into());
            }
            current.class_play.tactics.opportunities_used += 1;
            facts.push("Recorded one provoked opportunity; normal attack bonus, outside the iterative sequence".into());
        }
        let origin = current
            .class_play
            .actors
            .get("self")
            .ok_or("Register self")?
            .clone();
        if origin.dead || origin.unconscious {
            return Err("User cannot act".into());
        }
        for strike in &r.strikes {
            let (state, hit, weapon_facts) = self.weapon_hit(
                &current,
                strike,
                if full && !r.opportunity {
                    channel.attacks_used as i32 * 5
                } else {
                    0
                },
            )?;
            if full && !r.opportunity {
                channel.attacks_used += 1;
            }
            current.class_play = state;
            facts.extend(weapon_facts);
            if !hit {
                continue;
            }
            if !channel.targets.insert(strike.target.clone()) {
                facts.push("Same target: spell affects it only once per channeling round".into());
                continue;
            }
            if current.class_play.actors[&strike.target].dead {
                facts.push("Weapon damage killed the target before the spell rider".into());
                continue;
            }
            let mut defense = strike.spell.clone();
            defense.target = strike.target.clone();
            let result =
                self.resolve_delivered_spell(&current, &channel.casting, &defense, false)?;
            current.class_play = result.state;
            facts.extend(result.facts);
        }
        channel.attacks_closed = full
            && (channel.attacks_closed
                || channel.attacks_used >= channel.attack_limit
                || r.finish_attacks);
        if full {
            facts.push(format!("Full attack: {}/{} iterative attacks used; sequence {}; distinct spell targets {}; charge expires at {} seconds",channel.attacks_used,channel.attack_limit,if channel.attacks_closed{"closed"}else{"open"},channel.targets.len(),channel.expires.unwrap()));
        }
        if !full && !channel.targets.is_empty() {
            current.class_play.tactics.channel = Some(channel);
            if let Some(f) = crate::touch_charges::discharge(
                &mut current.class_play,
                "Standard channel delivered its one touch (FAQ p.17)",
            ) {
                facts.push(f);
            }
        } else if channel.expires.is_some() || channel.targets.is_empty() {
            current.class_play.tactics.channel = Some(channel);
        }
        crate::feature_resolution::finish(current.class_play, true, facts)
    }
}
