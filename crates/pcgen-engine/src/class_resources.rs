//! Committed single-class abilities. Preview and commit share the same resolver;
//! callers cannot supply their own remaining-resource counters.
use crate::class_play::PlayResult;
use crate::{CharacterDraft, Engine};
use pcgen_core::{class_play::*, magic::*};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ResourceAllowance {
    pub key: String,
    pub period: String,
    pub allowance: u32,
    pub spent: u32,
    pub remaining: u32,
}
fn finish(mut state: ClassPlay, success: bool, facts: Vec<String>) -> Result<PlayResult, String> {
    state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
    Ok(PlayResult {
        state,
        success,
        facts,
    })
}
impl Engine {
    pub fn class_resources(&self, d: &CharacterDraft) -> Result<Vec<ResourceAllowance>, String> {
        let s = self.evaluate(d);
        if !s.is_available() {
            return Err(s.diagnostics.join("; "));
        }
        let mut out = vec![];
        let mut add = |key: String, period: &str, allowance: u32, spent: u32| {
            out.push(ResourceAllowance {
                key,
                period: period.into(),
                allowance,
                spent,
                remaining: allowance.saturating_sub(spent),
            });
        };
        for l in crate::engine::effective_class_levels(&self.pack, d) {
            let Some(m) = crate::magic::class_magic(&self.pack, &l.class_id) else {
                continue;
            };
            if let Some(w) = m.wings.as_ref().filter(|w| l.level >= w.at_level) {
                add(
                    "Grow Wings".into(),
                    "day (seconds)",
                    l.level
                        .checked_mul(w.minutes_per_level)
                        .and_then(|v| v.checked_mul(60))
                        .ok_or("Wing allowance overflow")?,
                    d.class_play.resources.wings_seconds,
                );
            }
            for f in &m.sacred_spells {
                if let Some(t) = f
                    .tiers
                    .iter()
                    .filter(|t| t.level <= l.level)
                    .max_by_key(|t| t.level)
                {
                    let ledger = if f.period == "week" {
                        &d.class_play.resources.weekly
                    } else {
                        &d.class_play.resources.daily
                    };
                    add(
                        f.spell.clone(),
                        &f.period,
                        t.uses,
                        *ledger.get(&f.spell).unwrap_or(&0),
                    );
                }
            }
            for f in m
                .metamagic_features
                .iter()
                .filter(|f| f.at_level <= l.level)
            {
                let stat = s
                    .stats
                    .get(&f.uses_ability)
                    .ok_or("Resource ability score unavailable")?;
                let allowance =
                    (i64::from(f.uses_base) + (i64::from(*stat) - 10).div_euclid(2)).max(0);
                let key = match f.mode {
                    MetamagicMode::Cooperative => "Cooperative Metamagic",
                    MetamagicMode::Effect => "Metamagic Effect",
                };
                add(
                    key.into(),
                    "day",
                    u32::try_from(allowance).map_err(|_| "Resource allowance overflow")?,
                    *d.class_play.resources.daily.get(key).unwrap_or(&0),
                );
            }
        }
        for feature in crate::class_features::resolve_feature_values(&self.pack, d, &s.stats)? {
            if let Some(allowance) = feature.daily_uses {
                let key = format!("{}: {}", feature.class_id, feature.name);
                let spent = *d.class_play.resources.daily.get(&key).unwrap_or(&0);
                add(key, "day", allowance, spent);
            }
        }
        Ok(out)
    }
    pub fn use_class_ability(
        &self,
        d: &CharacterDraft,
        request: &ClassActionRequest,
        victim: &str,
        possessor: &str,
    ) -> Result<PlayResult, String> {
        let mut request = request.clone();
        let mut state = d.class_play.clone();
        match &mut request {
            ClassActionRequest::SacredSpell { spell, spent } => {
                let allowance = self
                    .class_resources(d)?
                    .into_iter()
                    .find(|r| r.key.eq_ignore_ascii_case(spell))
                    .ok_or("Ability not acquired")?;
                *spell = allowance.key;
                *spent = allowance.spent;
            }
            ClassActionRequest::GrowWings { spent_seconds, .. } => {
                *spent_seconds = state.resources.wings_seconds
            }
            ClassActionRequest::Exorcism { .. } => {
                if victim.trim().is_empty() || possessor.trim().is_empty() {
                    return Err("Victim and possessor IDs are required".into());
                }
                if state
                    .resources
                    .exorcised
                    .get(victim)
                    .is_some_and(|p| p.contains_key(possessor))
                {
                    return Err("This possessor is already barred from this victim for the recorded protection period".into());
                }
            }
            ClassActionRequest::MetamagicTrigger { .. } => {
                return Err("Use the item charge ledger for a spell-trigger item".into())
            }
            _ => {}
        }
        let result = self.class_action(d, &request)?;
        match &request {
            ClassActionRequest::SacredSpell { spell, spent } => {
                let allowance = self
                    .class_resources(d)?
                    .into_iter()
                    .find(|r| &r.key == spell)
                    .ok_or("Ability unavailable")?;
                let ledger = if allowance.period == "week" {
                    &mut state.resources.weekly
                } else {
                    &mut state.resources.daily
                };
                ledger.insert(
                    spell.clone(),
                    spent.checked_add(1).ok_or("Resource overflow")?,
                );
            }
            ClassActionRequest::GrowWings { use_seconds, .. } => {
                state.resources.wings_seconds = state
                    .resources
                    .wings_seconds
                    .checked_add(*use_seconds)
                    .ok_or("Wing expenditure overflow")?;
            }
            ClassActionRequest::Exorcism { .. } if result.success => {
                state
                    .resources
                    .exorcised
                    .entry(victim.into())
                    .or_default()
                    .insert(possessor.into(), 86400);
            }
            ClassActionRequest::WeaveTapestry {
                class_id, members, ..
            } if result.success => {
                state.resources.tapestry = Some(TapestryState {
                    casting_track: class_id.clone(),
                    members: members.clone(),
                    remaining_seconds: 86400,
                    stored: vec![],
                });
            }
            _ => {}
        }
        let mut facts = result.facts;
        for (key, value) in result.modifiers {
            facts.push(format!("{key}: {value:+}"));
        }
        finish(state, result.success, facts)
    }
    pub fn use_metamagic_feature(
        &self,
        d: &CharacterDraft,
        request: &MetamagicRequest,
    ) -> Result<PlayResult, String> {
        let mut request = request.clone();
        let key = match request.mode {
            MetamagicMode::Cooperative => "Cooperative Metamagic",
            MetamagicMode::Effect => "Metamagic Effect",
        };
        request.spent = *d.class_play.resources.daily.get(key).unwrap_or(&0);
        let r = self.metamagic_preview(d, &request)?;
        let mut state = d.class_play.clone();
        state.resources.daily.insert(
            key.into(),
            request.spent.checked_add(1).ok_or("Resource overflow")?,
        );
        finish(
            state,
            r.success,
            vec![
                format!(
                    "{key}: {} vs DC {}; {}; {} uses remain",
                    r.check_total,
                    r.spellcraft_dc,
                    if r.success {
                        "effect modified"
                    } else {
                        "attempt failed; use spent"
                    },
                    r.uses_remaining_after_attempt
                ),
                format!(
                    "Action: {}; duration: {}",
                    r.action,
                    r.resulting_duration.as_deref().unwrap_or("Unavailable")
                ),
            ],
        )
    }
    /// Calendar refresh does not erase ongoing commands, spells, tapestry or exorcism protection.
    pub fn refresh_class_resources(
        &self,
        d: &CharacterDraft,
        period: &str,
    ) -> Result<PlayResult, String> {
        let mut state = d.class_play.clone();
        match period {
            "day" => {
                state.resources.daily.clear();
                state.tactics.mettle_used.clear();
                state.tactics.daunting_used.clear();
                state.tactics.mettle_reward = false;
                state.tactics.extra_challenge = 0;
                state.resources.wings_seconds = 0;
                state.air_spent = 0;
                state.resources.turning_spent = 0;
                state.resources.instant_spent = 0;
                state.resources.sudden_spent.clear();
            }
            "week" => state.resources.weekly.clear(),
            "spells" => {
                state.resources.slots_spent.clear();
                state.resources.prepared_spent.clear();
            }
            _ => {
                return Err(
                    "Choose day, week, or spells (after the required spell recovery)".into(),
                )
            }
        }
        finish(state, true, vec![format!("New {period}: only resources with this refresh period were reset. Advance elapsed time separately.")])
    }
    pub fn advance_class_time(
        &self,
        d: &CharacterDraft,
        seconds: u32,
    ) -> Result<PlayResult, String> {
        if seconds == 0 {
            return Err("Elapsed time must be positive".into());
        }
        let mut state = d.class_play.clone();
        for victims in state.resources.exorcised.values_mut() {
            for remaining in victims.values_mut() {
                *remaining = remaining.saturating_sub(seconds);
            }
            victims.retain(|_, v| *v > 0);
        }
        state.resources.exorcised.retain(|_, v| !v.is_empty());
        if let Some(t) = &mut state.resources.tapestry {
            t.remaining_seconds = t.remaining_seconds.saturating_sub(seconds);
            if t.remaining_seconds == 0 {
                state.resources.tapestry = None;
            }
        }
        finish(state, true, vec![format!("Advanced tapestry and exorcism protections by {seconds} seconds. Spell control and Air effects retain their own encounter clock.")])
    }
}

impl Engine {
    pub fn store_weaving(
        &self,
        d: &CharacterDraft,
        request: &CastRequest,
    ) -> Result<PlayResult, String> {
        let t = d
            .class_play
            .resources
            .tapestry
            .as_ref()
            .ok_or("Weave a tapestry first")?;
        let targets = request
            .tapestry_targets
            .as_ref()
            .ok_or("Supply planned recipients")?;
        if !request.class_id.eq_ignore_ascii_case(&t.casting_track) {
            return Err("Use the tapestry's selected casting track".into());
        }
        let mut request = request.clone();
        request.tapestry = true;
        request.quiescent = true;
        request.stored_spells = t.stored.len() as u32;
        fund_request(d, &mut request);
        let cast = self.cast_preview(d, &request)?;
        let mut state = d.class_play.clone();
        record_cast_costs(&mut state, &request, &cast)?;
        if let Some(target) = &request.target {
            state.encounter.feints.remove(target);
        }
        if self
            .pack
            .classes
            .get(&cast.class_id)
            .is_some_and(|c| c.casting_type == pcgen_core::CastingType::Prepared)
        {
            let key = format!(
                "{}|{}",
                request.class_id.to_ascii_lowercase(),
                cast.spell.to_ascii_lowercase()
            );
            *state.resources.prepared_spent.entry(key).or_default() += 1;
        }
        let stored = StoredWeaving {
            spell: cast.spell.clone(),
            caster_level: cast.caster_level,
            save_dc: cast.save_dc,
            effective_level: cast.effective_level,
            duration: cast.duration,
            range: cast.range,
            range_feet: cast.range_feet,
            metamagic: cast.applied_metamagic,
            targets: targets.clone(),
            material_cost_gp: cast.material_cost_gp.ok_or("Material costs unavailable")?,
            xp_cost: cast.xp_cost.ok_or("XP costs unavailable")?,
        };
        let facts = vec![format!("Stored {}. Paid {} slot(s) of level {}, {} gp in components and {} XP for the planned recipients. Duration remains dormant.", stored.spell, cast.paid_slots, cast.paid_slot_level, stored.material_cost_gp, stored.xp_cost)];
        state
            .resources
            .tapestry
            .as_mut()
            .ok_or("Tapestry unavailable")?
            .stored
            .push(stored);
        finish(state, true, facts)
    }
    pub fn release_weaving(
        &self,
        d: &CharacterDraft,
        recipients: &[TapestryTargets],
    ) -> Result<PlayResult, String> {
        let t = d
            .class_play
            .resources
            .tapestry
            .as_ref()
            .ok_or("No active tapestry")?;
        if t.stored.is_empty() || recipients.len() != t.stored.len() {
            return Err(
                "Supply recipients for every stored spell; all spells release together".into(),
            );
        }
        let mut facts = vec!["Move action: release all stored spells in their original casting order; no additional spell slots are spent.".into()];
        for (spell, targets) in t.stored.iter().zip(recipients) {
            if (spell.material_cost_gp > 0 || spell.xp_cost > 0)
                && targets.recipients.len() > spell.targets.recipients.len()
            {
                return Err(
                    "Not enough prepaid material components / XP for this recipient count".into(),
                );
            }
            if targets.choice != spell.targets.choice {
                return Err("The shared spell choice was fixed at storage".into());
            }
            let resolved = self.tapestry_targets(
                d,
                &spell.spell,
                targets,
                spell.range,
                spell.range_feet,
                false,
            )?;
            let count = resolved.eligible.len();
            facts.extend(
                resolved
                    .rejected
                    .into_iter()
                    .map(|r| format!("Excluded {r}")),
            );
            facts.push(format!("Recipients: {}", resolved.eligible.join(", ")));
            facts.push(format!("{}: {count} recipients; original CL {}, DC {}; duration starts now: {}; range {:?}, {} ft. Original metamagic retained.",spell.spell,spell.caster_level,spell.save_dc,spell.duration.as_deref().unwrap_or("Unspecified"),spell.range,spell.range_feet.map(|n|n.to_string()).unwrap_or_else(||"unavailable".into())));
        }
        let mut state = d.class_play.clone();
        state
            .resources
            .tapestry
            .as_mut()
            .ok_or("Tapestry unavailable")?
            .stored
            .clear();
        finish(state, true, facts)
    }
}

impl Engine {
    pub fn register_trigger_item(
        &self,
        d: &CharacterDraft,
        id: &str,
        item: TriggerItem,
    ) -> Result<PlayResult, String> {
        if id.trim().is_empty() || d.class_play.resources.trigger_items.contains_key(id) {
            return Err("Choose a new item ID".into());
        }
        self.validate_trigger_item(&item)?;
        let mut state = d.class_play.clone();
        state.resources.trigger_items.insert(id.into(), item);
        finish(
            state,
            true,
            vec![format!(
                "Registered spell-trigger item {id}; charges do not refresh with daily abilities"
            )],
        )
    }
    pub fn use_trigger_item(
        &self,
        d: &CharacterDraft,
        id: &str,
        power: &str,
        metamagic: Vec<SpellRule>,
    ) -> Result<PlayResult, String> {
        let item = d
            .class_play
            .resources
            .trigger_items
            .get(id)
            .ok_or("Unknown spell-trigger item")?;
        let r = self.item_cast_preview(d, item, power, &metamagic)?;
        let cost = r.charges_spent;
        let mut state = d.class_play.clone();
        state
            .resources
            .trigger_items
            .get_mut(id)
            .ok_or("Unknown item")?
            .charges = item.charges.checked_sub(cost).ok_or("Not enough charges")?;
        let mut facts = r.facts;
        let effect = self.record_spell_effect(
            d,
            &mut state,
            &item.powers[power].spell,
            r.caster_level,
            r.save_dc,
            r.damage.as_ref(),
            item.kind == TriggerKind::Staff,
            &metamagic,
        )?;
        facts.push(format!(
            "Recorded casting {effect}; resolve each target's defenses once"
        ));
        finish(state, true, facts)
    }
}

pub(crate) fn fund_request(d: &CharacterDraft, request: &mut CastRequest) {
    let key = request.class_id.to_ascii_lowercase();
    request.slots_spent = d
        .class_play
        .resources
        .slots_spent
        .get(&key)
        .cloned()
        .unwrap_or_default();
    request.stored_spells = d.class_play.resources.tapestry.as_ref().map(|t|t.stored.len() as u32).unwrap_or(0);
    request.turning_spent = d.class_play.resources.turning_spent;
    request.instant_spent = d.class_play.resources.instant_spent;
    request.sudden_spent = d.class_play.resources.sudden_spent.clone();
}
fn record_cast_costs(
    state: &mut ClassPlay,
    request: &CastRequest,
    cast: &crate::magic::CastResolution,
) -> Result<(), String> {
    crate::touch_charges::discharge(state, "Another spell was cast");
    for (key, cost) in &cast.feature_costs {
        let n = state.resources.daily.entry(key.clone()).or_default();
        *n = n.checked_add(*cost).ok_or("Feature resource overflow")?;
    }
    let key = request.class_id.to_ascii_lowercase();
    let slots = state
        .resources
        .slots_spent
        .entry(key)
        .or_default()
        .entry(cast.paid_slot_level)
        .or_default();
    *slots = slots
        .checked_add(cast.paid_slots)
        .ok_or("Slot expenditure overflow")?;
    state.resources.turning_spent = state
        .resources
        .turning_spent
        .checked_add(cast.turning_cost)
        .ok_or("Turning expenditure overflow")?;
    if request.instant_metamagic.is_some() {
        state.resources.instant_spent += 1;
    }
    for rule in &request.sudden {
        let key = crate::magic::serde_rule_name(*rule).to_owned();
        *state.resources.sudden_spent.entry(key).or_default() += 1;
    }
    Ok(())
}
impl Engine {
    pub fn cast_and_record(
        &self,
        d: &CharacterDraft,
        request: &CastRequest,
    ) -> Result<PlayResult, String> {
        if request.quiescent {
            return Err("Use quiescent storage to preserve the dormant spell".into());
        }
        let mut request = request.clone();
        fund_request(d, &mut request);
        let cast = self.cast_preview(d, &request)?;
        let mut state = d.class_play.clone();
        record_cast_costs(&mut state, &request, &cast)?;
        if let Some(target) = &request.target {
            state.encounter.feints.remove(target);
        }
        if self
            .pack
            .classes
            .get(&cast.class_id)
            .is_some_and(|c| c.casting_type == pcgen_core::CastingType::Prepared)
        {
            let key = format!(
                "{}|{}",
                request.class_id.to_ascii_lowercase(),
                cast.spell.to_ascii_lowercase()
            );
            *state.resources.prepared_spent.entry(key).or_default() += 1;
        }
        let mut facts = vec![format!(
            "Cast {}: CL {}, DC {}; {} slot(s) of level {} spent; duration {}; {}",
            cast.spell,
            cast.caster_level,
            cast.save_dc,
            cast.paid_slots,
            cast.paid_slot_level,
            cast.duration.as_deref().unwrap_or("Unspecified"),
            cast.casting_time_note
        )];
        let effect = self.record_spell_effect(
            d,
            &mut state,
            &cast.spell,
            cast.caster_level,
            cast.save_dc,
            cast.damage.as_ref(),
            true,
            &cast.applied_metamagic,
        )?;
        self.arm_touch_charge(&mut state, &effect)?;
        let recorded = state
            .cast_effects
            .get_mut(&effect)
            .ok_or("Casting record unavailable")?;
        recorded.source_class = Some(request.class_id.clone());
        recorded.penetration_bonus += cast.penetration_bonus;
        recorded.bypass_sr = cast.bypass_sr;
        facts.push(format!(
            "Recorded casting {effect}; target defenses pending"
        ));
        if let Some(damage) = cast.damage {
            for (i, p) in damage.packets.iter().enumerate() {
                facts.push(format!(
                    "Damage packet {}: {}–{} before defenses (Edge {})",
                    i + 1,
                    p.minimum,
                    p.maximum,
                    p.edge
                ));
            }
        }
        finish(state, true, facts)
    }
}
