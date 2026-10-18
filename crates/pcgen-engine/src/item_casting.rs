//! Spell-trigger activation is not ordinary spellcasting and cannot enter a tapestry.
use crate::{CharacterDraft, Engine};
use pcgen_core::{class_play::*, magic::*};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct ItemCastResolution {
    pub range_feet: Option<u32>,
    pub duration: Option<String>,
    pub area_multiplier: u32,
    pub action: String,
    pub caster_level: i32,
    pub save_dc: i32,
    pub charges_spent: u32,
    pub warmage_edge: Option<i32>,
    pub damage: Option<crate::class_actions::DamageResolution>,
    pub facts: Vec<String>,
}
impl Engine {
    pub fn validate_trigger_item(&self, item: &TriggerItem) -> Result<(), String> {
        if item.powers.is_empty()
            || item.powers.len() > 20
            || (item.kind == TriggerKind::Wand && item.powers.len() != 1)
        {
            return Err("Wands require one power; staffs require 1–20 powers".into());
        }
        for (id, power) in &item.powers {
            if id.trim().is_empty() {
                return Err("Power ID is required".into());
            }
            let spell = self
                .pack
                .spells
                .values()
                .find(|s| {
                    s.name.eq_ignore_ascii_case(&power.spell)
                        || s.id.eq_ignore_ascii_case(&power.spell)
                })
                .ok_or("Unknown item spell")?;
            let level = *spell
                .class_levels
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&power.spell_list))
                .map(|(_, v)| v)
                .ok_or("Creation spell list unavailable")?;
            if level > 9
                || item.caster_level == 0
                || item.caster_level > 10000
                || item.charges > 50
                || power.base_charges == 0
                || power.base_charges > 50
                || item.kind == TriggerKind::Wand && (level > 4 || power.base_charges != 1)
                || item.kind == TriggerKind::Staff && item.caster_level < 8
            {
                return Err("Invalid item: wands hold spells up to level 4 and use one base charge; staffs require CL 8+; charges must be 0–50".into());
            }
            let effective_level = power.heightened_level.unwrap_or(level);
            if effective_level < level
                || effective_level > 9
                || (item.kind == TriggerKind::Wand && effective_level > 4)
            {
                return Err("Invalid stored Heighten level".into());
            }
        }
        Ok(())
    }
    pub fn item_cast_preview(
        &self,
        d: &CharacterDraft,
        item: &TriggerItem,
        power_id: &str,
        metamagic: &[SpellRule],
    ) -> Result<ItemCastResolution, String> {
        if d.class_play.encounter.rage.is_some() {
            return Err("Cannot activate spell-trigger items while raging".into());
        }
        let power = item.powers.get(power_id).ok_or("Unknown item power")?;
        let spell = self
            .pack
            .spells
            .values()
            .find(|s| {
                s.name.eq_ignore_ascii_case(&power.spell) || s.id.eq_ignore_ascii_case(&power.spell)
            })
            .ok_or("Unknown item spell")?;
        let level = *spell
            .class_levels
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&power.spell_list))
            .map(|(_, v)| v)
            .ok_or("Item spell level is not defined on the creation spell list")?;
        self.validate_trigger_item(item)?;
        let effective_level = power.heightened_level.unwrap_or(level);
        let catalog = self.spell_catalog(d, &item.casting_track)?;
        let access = catalog
            .iter()
            .find(|s| s.spell.eq_ignore_ascii_case(&spell.name))
            .ok_or(
                "Spell is not on the selected user's class list (Use Magic Device is not assumed)",
            )?;
        if access.prohibited {
            return Err("Focused Studies / prohibited school blocks activation".into());
        }
        // Known spell, current slots and minimum casting ability are not spell-trigger prerequisites.
        let snap = self.evaluate(d);
        if !snap.is_available() {
            return Err(snap.diagnostics.join("; "));
        }
        let class = self
            .pack
            .classes
            .values()
            .find(|c| c.id.eq_ignore_ascii_case(&item.casting_track))
            .ok_or("Unknown casting track")?;
        let own_cl = *snap
            .caster_levels
            .get(&class.id)
            .ok_or("Caster level unavailable")?;
        let mut cl = item.caster_level as i32;
        let mut dc = 10 + effective_level as i32 + (effective_level as i32 / 2);
        let mut edge = Some(0);
        if item.kind == TriggerKind::Staff {
            cl = cl.max(own_cl);
            let ability = class
                .spellcasting_ability
                .as_ref()
                .ok_or("Staff user's casting ability unavailable")?;
            dc = 10
                + effective_level as i32
                + (snap
                    .stats
                    .get(ability)
                    .ok_or("Staff user's ability score unavailable")?
                    - 10)
                    .div_euclid(2);
            let state = crate::apply_effects(&self.pack, d);
            if !state.diagnostics.is_empty() {
                return Err(state.diagnostics.join("; "));
            }
            dc += crate::item_casting::dc_bonus(&state, &spell.school, &spell.name, &class.id);
            if let Some(ability) = crate::magic::class_magic(&self.pack, &class.id)
                .and_then(|m| m.edge_ability.as_ref())
            {
                edge = match spell.profile.hit_point_damage {
                    Some(true) => Some(
                        (snap
                            .stats
                            .get(ability)
                            .ok_or("Warmage Edge ability unavailable")?
                            - 10)
                            .div_euclid(2)
                            .max(0),
                    ),
                    Some(false) => Some(0),
                    None => None,
                };
            }
        }
        if metamagic.contains(&SpellRule::Sanctum) {
            return Err("Sanctum Spell needs explicit location context; not supported by this activation form".into());
        }
        let mut duration = spell.profile.duration.clone();
        let mut range_multiplier = 1;
        let mut action =
            "Standard action, or the spell's longer casting time; no attack of opportunity"
                .to_string();
        let (charges, mut facts) = if metamagic.is_empty() {
            (
                power.base_charges,
                vec!["Normal spell-trigger activation; no item creation feat is required".into()],
            )
        } else {
            let result = self.class_action(
                d,
                &ClassActionRequest::MetamagicTrigger {
                    spell: spell.name.clone(),
                    item_creation_feat: match item.kind {
                        TriggerKind::Wand => "Craft Wand",
                        TriggerKind::Staff => "Craft Staff",
                    }
                    .into(),
                    metamagic: metamagic.to_vec(),
                    charges: item.charges,
                    base_charges: power.base_charges,
                },
            )?;
            let effect = result
                .trigger_effect
                .as_ref()
                .ok_or("Metamagic result unavailable")?;
            duration = effect.duration.clone();
            range_multiplier = effect.range_multiplier;
            action = result.action.clone();
            (result.charges_spent.ok_or("Charge cost unavailable")?, {
                let mut facts = result.facts;
                facts.push(result.action);
                facts
            })
        };
        if charges > item.charges {
            return Err("Insufficient item charges".into());
        }
        let damage = spell
            .profile
            .damage
            .as_ref()
            .map(|p| {
                crate::class_actions::damage_range(
                    p,
                    cl,
                    edge.ok_or("Warmage Edge applicability unavailable")?,
                    metamagic,
                )
            })
            .transpose()?;
        facts.push(format!(
            "{:?}: CL {cl}; save DC {dc}; {} charge(s) spent; Warmage Edge {}",
            item.kind,
            charges,
            edge.map(|v| v.to_string()).unwrap_or("unavailable".into())
        ));
        facts.push("No character spell slots spent. Item activation cannot be routed through Eldritch Tapestry or Quiescent Weaving. Initial saves and spell resistance use the recorded casting; other target effects resolve separately.".into());
        if let Some(damage) = &damage {
            for (i, p) in damage.packets.iter().enumerate() {
                facts.push(format!(
                    "Damage packet {}: {}–{} before defenses",
                    i + 1,
                    p.minimum,
                    p.maximum
                ));
            }
        }
        let range_feet = crate::targeting::range_feet(
            spell.profile.range,
            cl,
            range_multiplier,
            spell.profile.fixed_range_feet,
        )?;
        let area_multiplier = if metamagic.contains(&SpellRule::Widen) {
            2
        } else {
            1
        };
        facts.push(format!(
            "Range: {}; duration: {}; area multiplier: {area_multiplier}; {action}",
            range_feet
                .map(|n| format!("{n} feet"))
                .unwrap_or_else(|| format!("{:?}", spell.profile.range)),
            duration.as_deref().unwrap_or("unavailable")
        ));
        Ok(ItemCastResolution {
            range_feet,
            duration,
            area_multiplier,
            action,
            caster_level: cl,
            save_dc: dc,
            charges_spent: charges,
            warmage_edge: edge,
            damage,
            facts,
        })
    }
}
pub(crate) fn dc_bonus(
    state: &crate::effects::EffectState,
    school: &str,
    spell: &str,
    track: &str,
) -> i32 {
    let keys = [
        "DC.ALL".to_owned(),
        format!("DC.SCHOOL.{school}"),
        format!("DC.SPELL.{spell}"),
        format!("DC.CLASS.{track}"),
    ];
    crate::bonuses::stacked_components(&state.typed_bonuses, |b| {
        if keys.iter().any(|k| k.eq_ignore_ascii_case(&b.target)) {
            vec!["dc".into()]
        } else {
            vec![]
        }
    })
    .get("dc")
    .copied()
    .unwrap_or(0)
}

pub fn item_templates() -> Result<std::collections::BTreeMap<String, TriggerItem>, String> {
    serde_json::from_str(include_str!("../../../data/rules/spell-trigger-items.json"))
        .map_err(|e| e.to_string())
}
