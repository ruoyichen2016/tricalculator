//! Situational class abilities use class levels, never advanced spellcasting levels.
use crate::{CharacterDraft, Engine};
use pcgen_core::magic::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TriggerEffect {
    pub duration: Option<String>,
    pub range_multiplier: u32,
}
#[derive(Debug, Serialize)]
pub struct ClassActionResolution {
    pub trigger_effect: Option<TriggerEffect>,
    pub feature: String,
    pub action: String,
    pub success: bool,
    pub facts: Vec<String>,
    /// Situational values, never silently added to unrelated character totals.
    pub modifiers: std::collections::BTreeMap<String, i32>,
    pub charges_spent: Option<u32>,
}
impl Engine {
    pub fn class_action(
        &self,
        draft: &CharacterDraft,
        request: &ClassActionRequest,
    ) -> Result<ClassActionResolution, String> {
        let snapshot = self.evaluate(draft);
        if !snapshot.is_available() {
            return Err(snapshot.diagnostics.join("; "));
        }
        let levels = crate::engine::effective_class_levels(&self.pack, draft);
        let ability = |key: &str| {
            snapshot
                .stats
                .get(key)
                .map(|v| (v - 10).div_euclid(2))
                .ok_or_else(|| format!("{key} is unavailable"))
        };
        let mut result = ClassActionResolution {
            trigger_effect: None,
            feature: String::new(),
            action: String::new(),
            success: true,
            facts: vec![],
            modifiers: Default::default(),
            charges_spent: None,
        };
        match request {
            ClassActionRequest::GrowWings {
                spent_seconds,
                use_seconds,
                armor_or_restrictive_clothing,
                grappling,
            } => {
                let (level, wings) = levels
                    .iter()
                    .find_map(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id)?
                            .wings
                            .as_ref()
                            .filter(|w| l.level >= w.at_level)
                            .map(|w| (l.level, w))
                    })
                    .ok_or("Grow Wings is not acquired")?;
                if *armor_or_restrictive_clothing || *grappling {
                    return Err(
                        "Wings cannot grow in armor, restrictive clothing, or a grapple".into(),
                    );
                }
                let total = level
                    .checked_mul(wings.minutes_per_level)
                    .and_then(|n| n.checked_mul(60))
                    .ok_or("Wing duration overflow")?;
                if *use_seconds == 0
                    || spent_seconds
                        .checked_add(*use_seconds)
                        .is_none_or(|n| n > total)
                {
                    return Err("Requested flight exceeds the remaining daily duration".into());
                }
                result.feature = "Grow Wings".into();
                result.action = "Standard action to grow or dismiss".into();
                result.facts = vec![
                    format!(
                        "Flight: {} feet, {} maneuverability",
                        wings.speed_feet, wings.maneuverability
                    ),
                    format!(
                        "Daily duration: {total} seconds; {} seconds remain after this use",
                        total - spent_seconds - use_seconds
                    ),
                    "Supernatural ability; ordinary spell metamagic does not apply".into(),
                ];
            }
            ClassActionRequest::Exorcism {
                roll,
                possessor_hit_dice,
                possessor_charisma,
            } => {
                let level = levels
                    .iter()
                    .find(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id)
                            .is_some_and(|m| m.exorcism_from_level.is_some_and(|n| l.level >= n))
                    })
                    .ok_or("Exorcism is not acquired")?
                    .level;
                if !(1..=20).contains(roll)
                    || *possessor_hit_dice == 0
                    || *possessor_hit_dice > 10000
                    || !(1..=10000).contains(possessor_charisma)
                {
                    return Err(
                        "Supply a d20 roll, positive possessor HD, and positive Charisma score"
                            .into(),
                    );
                }
                let check = *roll as i64 + level as i64 + ability("CHA")? as i64;
                let dc = 10
                    + *possessor_hit_dice as i64
                    + (*possessor_charisma as i64 - 10).div_euclid(2);
                result.feature = "Exorcism".into();
                result.action = "Full-round action".into();
                result.success = check >= dc;
                result.facts = vec![
                    format!(
                        "Class-level check: {roll} + {level} + {} = {check}; DC {dc}",
                        ability("CHA")?
                    ),
                    if result.success {
                        "Possessor expelled; it cannot possess this victim again for 24 hours"
                            .into()
                    } else {
                        "Possession continues; natural 20 is not automatic success".into()
                    },
                    "No turning attempts or spell slots spent; supernatural ability".into(),
                ];
            }
            ClassActionRequest::Detect { spell } => {
                let feature = levels
                    .iter()
                    .find_map(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id)?
                            .detections
                            .iter()
                            .find(|f| l.level >= f.at_level && f.spell.eq_ignore_ascii_case(spell))
                    })
                    .ok_or("This detection ability is not acquired")?;
                // SRD: an unspecified SLA caster level defaults to the creature's HD.
                let hd = snapshot.total_level;
                result.feature = feature.spell.clone();
                result.action = "Standard action; maintain concentration".into();
                result.facts=vec![format!("At will; spell-like ability caster level {hd}"),format!("Save DC {} where the spell allows a save",10+feature.spell_level as i32+ability("CHA")?),"No spell slots or turning attempts spent; ordinary spell metamagic does not apply".into()];
            }
            ClassActionRequest::WeaveTapestry {
                class_id,
                roll,
                threatened,
                distracted,
                members,
            } => {
                if !levels.iter().any(|l| {
                    crate::magic::class_magic(&self.pack, &l.class_id)
                        .is_some_and(|m| m.tapestry_from_level.is_some_and(|n| l.level >= n))
                }) {
                    return Err("Eldritch Tapestry is not acquired".into());
                }
                let class = self
                    .pack
                    .classes
                    .values()
                    .find(|c| {
                        c.id.eq_ignore_ascii_case(class_id)
                            && c.spell_power == pcgen_core::SpellPowerType::Arcane
                    })
                    .ok_or("Choose an acquired arcane casting track")?;
                self.spell_catalog(draft, class_id)?;
                let maximum = ability(
                    class
                        .spellcasting_ability
                        .as_deref()
                        .ok_or("Casting ability unavailable")?,
                )?
                .max(0) as u32;
                self.weave_members(draft, members)?;
                if members.len() as u32 > maximum {
                    return Err("Too many allies for the casting ability modifier".into());
                }
                let allies = members.len();
                let die = match roll {
                    Some(n) if (1..=20).contains(n) => *n,
                    Some(_) => return Err("D20 must be 1 to 20".into()),
                    None if *threatened || *distracted => {
                        return Err("Cannot take 10 while threatened or distracted".into())
                    }
                    None => 10,
                };
                let skill = snapshot
                    .skills
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("Craft (Weaving)"))
                    .map(|(_, v)| *v)
                    .or_else(|| {
                        self.untrained_skill_base(
                            "Craft (Weaving)",
                            &snapshot
                                .stats
                                .iter()
                                .map(|(k, v)| (k.clone(), (v - 10).div_euclid(2)))
                                .collect(),
                            0,
                        )
                    })
                    .ok_or("Craft (Weaving) modifier is unavailable")?;
                let check = die as i32 + skill;
                result.feature = "Eldritch Tapestry".into();
                result.action = "10 minutes".into();
                result.success = check >= 15;
                result.facts=vec![format!("Craft (Weaving): {die} + {skill} = {check}; DC 15"),format!("{allies} allies, maximum {maximum}; caster connected without using an ally place"),if result.success {"Lasts 24 hours or until replaced by a new tapestry".into()}else{"No tapestry created".into()}];
            }
            ClassActionRequest::MetamagicTrigger {
                spell,
                item_creation_feat,
                metamagic,
                charges,
                base_charges,
            } => {
                if !levels.iter().any(|l| {
                    crate::magic::class_magic(&self.pack, &l.class_id)
                        .is_some_and(|m| m.trigger_from_level.is_some_and(|n| l.level >= n))
                }) {
                    return Err("Metamagic Spell Trigger is not acquired".into());
                }
                if !["Craft Wand", "Craft Staff"].contains(&item_creation_feat.as_str())
                    || !draft
                        .abilities
                        .iter()
                        .any(|(_, name)| name.eq_ignore_ascii_case(item_creation_feat))
                {
                    return Err(
                        "The corresponding Craft Wand or Craft Staff feat is required".into(),
                    );
                }
                let spell = self
                    .pack
                    .spells
                    .values()
                    .find(|s| s.name.eq_ignore_ascii_case(spell))
                    .ok_or("Unknown trigger spell")?;
                let state = crate::apply_effects(&self.pack, draft);
                let mut effective = 0;
                let mut duration = spell.profile.duration.clone();
                let mut multiplier = 1;
                if *base_charges == 0 {
                    return Err("Metamagic Spell Trigger requires a charge-consuming item".into());
                }
                let mut cost = *base_charges;
                for (i, rule) in metamagic.iter().enumerate() {
                    if metamagic[..i].contains(rule) {
                        return Err("A metamagic feat cannot be applied twice".into());
                    }
                    let feat = self
                        .pack
                        .entities
                        .iter()
                        .find(|e| {
                            e.metadata.spell_rule == Some(*rule)
                                && crate::effects::ability_matches_draft(draft, e)
                        })
                        .ok_or("The applied metamagic feat is not possessed")?;
                    if feat.requirements.iter().any(|p| {
                        crate::effects::predicate_result(Some(p), draft, &state) != Some(true)
                    }) {
                        return Err("Metamagic prerequisites are unmet or unavailable".into());
                    }
                    if *rule == SpellRule::Still {
                        result
                            .facts
                            .push("Still Spell has no benefit on spell-trigger items".into());
                    }
                    let delta = crate::metamagic::transform(
                        &self.pack,
                        spell,
                        *rule,
                        false,
                        &mut effective,
                        &mut duration,
                        spell.profile.range,
                        &mut multiplier,
                    )?;
                    cost = cost
                        .checked_add(crate::class_features::improved_cost(
                            &self.pack, draft, delta,
                        ))
                        .ok_or("Charge cost overflow")?;
                }
                if *charges < cost {
                    return Err(format!("Requires {cost} charges"));
                }
                result.trigger_effect = Some(TriggerEffect {
                    duration: duration.clone(),
                    range_multiplier: multiplier,
                });
                result.charges_spent = Some(cost);
                result.feature = "Metamagic Spell Trigger".into();
                result.action = if metamagic.contains(&SpellRule::Quicken) {
                    "Swift action"
                } else {
                    "Use the item's activation time"
                }
                .into();
                result.facts.push(format!("{cost} charges consumed; {} remain. Duration: {}; range multiplier {multiplier}; area multiplier {}",charges-cost,duration.unwrap_or("Unspecified".into()),if metamagic.contains(&SpellRule::Widen){2}else{1}));
            }
            ClassActionRequest::SacredDefense {
                target_kind,
                desecrated,
            } => {
                let level = levels
                    .iter()
                    .find(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id)
                            .is_some_and(|m| m.sacred_features)
                    })
                    .ok_or("Sacred Exorcist is not acquired")?
                    .level;
                result.feature = "Sacred defenses".into();
                result.action = "Passive; situational bonuses do not alter unrelated checks".into();
                if level >= 2 {
                    result.modifiers.insert("possession_save_sacred".into(), 4);
                    result
                        .modifiers
                        .insert("possession_dispel_sacred".into(), 2);
                    result.modifiers.insert(
                        "evil_outsider_or_undead_charm_compulsion_save_sacred".into(),
                        2,
                    );
                    result.facts.push("Resist Possession: +4 sacred on possession saves; +2 sacred on dispel checks against possession; +2 sacred on saves against charms and compulsions from evil outsiders or undead".into());
                }
                if level >= 3 {
                    let chosen_foe = draft
                        .choices
                        .get("Sacred Exorcist.ChosenFoe")
                        .ok_or("Choose a permanent Chosen Foe in the character editor")?;
                    if !["undead", "evil_outsider"].contains(&chosen_foe.as_str()) {
                        return Err("Choose undead or evil_outsider as Chosen Foe".into());
                    }
                    let bonus = if level >= 9 {
                        3
                    } else if level >= 6 {
                        2
                    } else {
                        1
                    };
                    let applied = if chosen_foe == target_kind { bonus } else { 0 };
                    for skill in ["Bluff", "Intimidate", "Listen", "Sense Motive", "Spot"] {
                        result
                            .modifiers
                            .insert(format!("{skill}_competence"), applied);
                    }
                    result
                        .modifiers
                        .insert("spell_resistance_check".into(), applied);
                    result.facts.push(format!("Chosen Foe: +{} competence to Bluff, Intimidate, Listen, Sense Motive and Spot; +{} on caster-level checks to overcome spell resistance against this target",if chosen_foe==target_kind{bonus}else{0},if chosen_foe==target_kind{bonus}else{0}));
                }
                if level >= 5 {
                    result.facts.push(if *desecrated {"Consecrated Presence: suppressed by Desecrate; no aura bonuses while suppressed".into()}else{"Consecrated Presence: 20-foot moving Consecrate aura; +3 sacred on turning Charisma checks, undead -1 on attacks, damage and saves, no undead creation or summoning. A dedicated altar doubles these modifiers; an altar to another deity instead loses its connection.".into()});
                }
            }
            ClassActionRequest::SacredSpell { spell, spent } => {
                let level = levels
                    .iter()
                    .find(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id)
                            .is_some_and(|m| m.sacred_features)
                    })
                    .ok_or("Sacred Exorcist is not acquired")?
                    .level;
                let feature = levels
                    .iter()
                    .find_map(|l| {
                        crate::magic::class_magic(&self.pack, &l.class_id)?
                            .sacred_spells
                            .iter()
                            .find(|s| s.spell.eq_ignore_ascii_case(spell))
                    })
                    .ok_or("This Sacred Exorcist spell-like ability is not defined")?;
                let uses = feature
                    .tiers
                    .iter()
                    .filter(|t| t.level <= level)
                    .max_by_key(|t| t.level)
                    .map(|t| t.uses)
                    .ok_or("This Sacred Exorcist spell-like ability is not acquired")?;
                let period = &feature.period;
                if *spent >= uses {
                    return Err(format!("No {spell} uses remain this {period}"));
                }
                result.feature = spell.clone();
                result.action = "Standard action; spell-like ability".into();
                result.facts = vec![
                    format!(
                        "{} uses remaining this {period}; caster level {} (total Hit Dice)",
                        uses - spent - 1,
                        snapshot.total_level
                    ),
                    format!(
                        "Save DC {} (Charisma-based)",
                        10 + feature.spell_level as i32 + ability("CHA")?
                    ),
                    "Spell-like abilities use neither spell slots nor ordinary metamagic feats"
                        .into(),
                ];
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Serialize)]
pub struct QuiescentResolution {
    pub release_action: String,
    pub release_order: Vec<crate::magic::CastResolution>,
    pub slots_used: std::collections::BTreeMap<String, std::collections::BTreeMap<u32, u32>>,
    pub turning_used: u32,
}
impl Engine {
    /// Plan creation and release together. Stored effects consume their slots at
    /// preparation, not again at release; no duration elapses while dormant.
    pub fn quiescent_preview(
        &self,
        draft: &CharacterDraft,
        spells: &[CastRequest],
    ) -> Result<QuiescentResolution, String> {
        if spells.is_empty() {
            return Err("Supply at least one spell to store".into());
        }
        let mut result=QuiescentResolution{release_action:"Move action; release ALL stored spells in casting order. Durations start now; supplied targets and ranges describe the release moment.".into(),release_order:vec![],slots_used:Default::default(),turning_used:0};
        for (index, spell) in spells.iter().enumerate() {
            let mut request = spell.clone();
            request.tapestry = true;
            request.quiescent = true;
            request.stored_spells = request
                .stored_spells
                .checked_add(index as u32)
                .ok_or("Stored spell count overflow")?;
            let key = request.class_id.to_ascii_lowercase();
            if let Some(spent) = result.slots_used.get(&key) {
                for (level, count) in spent {
                    let old = request.slots_spent.entry(*level).or_default();
                    *old = old.checked_add(*count).ok_or("Spent slots overflow")?;
                }
            }
            request.turning_spent = request
                .turning_spent
                .checked_add(result.turning_used)
                .ok_or("Turning use overflow")?;
            let mut cast = self.cast_preview(draft, &request)?;
            cast.effect_timing="Duration starts on move-action release; no additional slots or turning attempts spent at release".into();
            *result
                .slots_used
                .entry(key)
                .or_default()
                .entry(cast.paid_slot_level)
                .or_default() += cast.paid_slots;
            result.turning_used += cast.turning_cost;
            result.release_order.push(cast);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DamagePacket {
    pub dice: u32,
    pub die_sides: u32,
    pub flat: i32,
    pub edge: i32,
    pub minimum: i64,
    pub maximum: i64,
}
#[derive(Debug, Clone, Serialize)]
pub struct DamageResolution {
    pub packets: Vec<DamagePacket>,
    pub area: bool,
    pub note: String,
}
pub(crate) fn damage_range(
    profile: &DamageProfile,
    caster_level: i32,
    edge: i32,
    metamagic: &[SpellRule],
) -> Result<DamageResolution, String> {
    let bindings = std::collections::HashMap::from([("CASTERLEVEL".into(), caster_level)]);
    let eval = |expression: &str| {
        pcgen_formula::Formula::parse(expression)
            .and_then(|f| f.eval_i32(&bindings))
            .map_err(|e| format!("Damage formula: {e}"))
    };
    let dice = eval(&profile.dice)?;
    let count = eval(&profile.packets)?;
    let flat = eval(&profile.flat)?;
    if !(1..=1000).contains(&dice)
        || !(1..=1000).contains(&count)
        || !(2..=1000).contains(&profile.die_sides)
    {
        return Err("Damage dice/packet count is outside the supported positive range".into());
    }
    let empower = metamagic.contains(&SpellRule::Empower);
    let maximize = metamagic.contains(&SpellRule::Maximize);
    let mut packets = vec![];
    for index in 0..count {
        let bonus = if index == 0 { edge } else { 0 };
        let low = dice as i64 + flat as i64 + bonus as i64;
        let high = dice as i64 * profile.die_sides as i64 + flat as i64 + bonus as i64;
        // Maximize and Empower have separate benefits, not 1.5 * maximum.
        let (minimum, maximum) = match (empower, maximize) {
            (true, true) => (high + low.div_euclid(2), high + high.div_euclid(2)),
            (true, false) => (low + low.div_euclid(2), high + high.div_euclid(2)),
            (false, true) => (high, high),
            (false, false) => (low, high),
        };
        packets.push(DamagePacket {
            dice: dice as u32,
            die_sides: profile.die_sides,
            flat,
            edge: bonus,
            minimum,
            maximum,
        });
    }
    Ok(DamageResolution{packets,area:profile.area,note:"Before saves, criticals, resistance and immunity. The first packet receives Edge (choose which missile gets it); area damage applies to each affected creature. Empower includes the additive damage subtotal; Maximize + Empower adds half the normally rolled result separately.".into()})
}
