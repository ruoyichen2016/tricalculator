//! A single charge lifecycle for ordinary touches and Duskblade channeling.
use crate::{class_play::PlayResult, CharacterDraft, Engine};
use pcgen_core::{class_play::*, tactics::*};
pub(crate) fn discharge(s: &mut ClassPlay, reason: &str) -> Option<String> {
    let charge = s.tactics.channel.take()?;
    if let Some(c) = s.cast_effects.get_mut(&charge.casting) {
        c.ended = true;
    }
    Some(format!("{}: charge discharged ({reason})", charge.casting))
}
impl Engine {
    pub(crate) fn arm_touch_charge(&self, s: &mut ClassPlay, key: &str) -> Result<(), String> {
        let cast = s.cast_effects.get(key).ok_or("Casting unavailable")?;

        let profile = &self
            .pack
            .spells
            .values()
            .find(|v| v.name == cast.spell)
            .ok_or("Touch profile unavailable")?
            .profile;
        if profile.range != pcgen_core::magic::SpellRange::Touch {
            return Ok(());
        }
        let uses = profile
            .touch_uses
            .as_ref()
            .map(|f| crate::spell_impacts::formula(f, cast.caster_level))
            .transpose()?
            .unwrap_or(1);
        let remaining = u32::try_from(uses)
            .ok()
            .filter(|n| *n > 0)
            .ok_or("Touch count unavailable")?;
        s.cast_effects.get_mut(key).unwrap().requires_touch = true;
        s.tactics.channel = Some(ChannelState {
            casting: key.into(),
            mode: ChargeMode::Touch,
            remaining,
            expires: None,
            targets: Default::default(),
            attacks_used: 0,
            attack_limit: 0,
            attacks_closed: true,
        });
        Ok(())
    }
    pub fn deliver_held_touch(
        &self,
        d: &CharacterDraft,
        r: &TouchRequest,
    ) -> Result<PlayResult, String> {
        let mut charge = d
            .class_play
            .tactics
            .channel
            .clone()
            .ok_or("No held touch charge")?;
        if charge.mode == ChargeMode::FullChannel {
            return Err("Full-attack channeling must use its weapon delivery; ordinary touch cannot extend it".into());
        }
        let casting = d
            .class_play
            .cast_effects
            .get(&charge.casting)
            .ok_or("Casting unavailable")?;
        if casting.ended || charge.remaining == 0 {
            return Err("Touch charge exhausted".into());
        }
        let mut current = d.clone();
        let mut facts = vec![];
        crate::necromancy::require_actor(&d.class_play, "self")?;
        crate::necromancy::require_actor(&d.class_play, &r.defense.target)?;
        let origin = d.class_play.actors.get("self").ok_or("Register self")?;
        let target = d
            .class_play
            .actors
            .get(&r.defense.target)
            .ok_or("Unknown touch target")?;
        if origin.dead
            || origin.unconscious
            || crate::combat::condition(&d.class_play, "self", "paralyzed")
            || crate::combat::condition(&d.class_play, "self", "panicked")
        {
            return Err("Caster cannot deliver a touch".into());
        }
        if target.dead
            || crate::targeting::distance(origin, target) > origin.touch_reach_feet
            || !matches!(target.path, TargetPath::Clear | TargetPath::Obscured)
        {
            return Err("Touch target dead, blocked or outside reach".into());
        }
        let snap = self.evaluate(d);
        if !snap.is_available() {
            return Err("Character snapshot unavailable".into());
        }
        let mut critical = false;
        if !target.willing && r.defense.target != "self" {
            let die = r
                .roll
                .filter(|n| (1..=20).contains(n))
                .ok_or("Touch attack d20 required")?;
            let ac = d
                .class_play
                .encounter
                .combat
                .get(&r.defense.target)
                .ok_or("Target touch AC unavailable")?
                .touch_ac
                + crate::necromancy::ability_delta(&d.class_play, &r.defense.target, "DEX");
            let bonus = snap.melee_attack + snap.attack_bonuses.get("ALL").copied().unwrap_or(0)
                - if crate::combat::condition(&d.class_play, "self", "sickened") {
                    2
                } else {
                    0
                };
            facts.push(format!(
                "Melee touch d20 {die} + {bonus} vs touch AC {ac}; no weapon damage"
            ));
            if die == 1 || die != 20 && die as i32 + bonus < ac {
                facts.push("Touch missed; charge retained without spending another slot".into());
                return crate::feature_resolution::finish(current.class_play, true, facts);
            }
            if die == 20
                && casting.damage_bounds.is_some()
                && !crate::combat::immune(
                    &d.class_play,
                    &r.defense.target,
                    BodyImmunity::CriticalHits,
                )
            {
                let confirm = r
                    .confirmation
                    .filter(|n| (1..=20).contains(n))
                    .ok_or("Touch critical threat requires confirmation d20")?;
                critical = confirm == 20 || confirm != 1 && confirm as i32 + bonus >= ac;
                facts.push(format!(
                    "Touch critical confirmed: {critical}; use the full spell damage dice total"
                ));
            }
        } else {
            facts.push("Willing recipient: touch requires no attack roll".into());
        }
        let result =
            self.resolve_delivered_spell(&current, &charge.casting, &r.defense, critical)?;
        current.class_play = result.state;
        facts.extend(result.facts);
        charge.remaining -= 1;
        current
            .class_play
            .cast_effects
            .get_mut(&charge.casting)
            .ok_or("Casting disappeared")?
            .contact_sequence += 1;
        if charge.remaining == 0 {
            current.class_play.tactics.channel = Some(charge);
            if let Some(f) = discharge(&mut current.class_play, "All touches delivered") {
                facts.push(f);
            }
        } else {
            facts.push(format!(
                "{} touches remaining; a save or SR still consumes the delivered touch",
                charge.remaining
            ));
            current.class_play.tactics.channel = Some(charge);
        }
        crate::feature_resolution::finish(current.class_play, true, facts)
    }
    /// Physical contact with an object consumes the charge even if that object is ineligible.
    pub fn record_charge_object_contact(&self, d: &CharacterDraft) -> Result<PlayResult, String> {
        let mut state = d.class_play.clone();
        if state
            .tactics
            .channel
            .as_ref()
            .is_some_and(|c| c.mode == ChargeMode::FullChannel)
        {
            return Err("Use full-attack weapon delivery or advance to round end".into());
        }
        let charge = state.tactics.channel.as_mut().ok_or("No held charge")?;
        charge.remaining -= 1;
        let mut facts =
            vec!["Incidental object contact: one touch expended; no target effect inferred".into()];
        if charge.remaining == 0 {
            if let Some(f) = discharge(&mut state, "Incidental object contact") {
                facts.push(f);
            }
        }
        crate::feature_resolution::finish(state, true, facts)
    }
}
