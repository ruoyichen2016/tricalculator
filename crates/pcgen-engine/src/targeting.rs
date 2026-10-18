//! Named recipients and current scene facts; no caller-supplied legality flags.
use crate::{CharacterDraft, Engine};
use pcgen_core::{class_play::*, magic::*};
use std::collections::BTreeSet;
#[derive(Debug)]
pub struct TargetResolution {
    pub eligible: Vec<String>,
    pub rejected: Vec<String>,
}
pub fn distance(a: &SceneActor, b: &SceneActor) -> u32 {
    let axis = |x: i32, w: u32, y: i32, v: u32| {
        let (x, y) = (i64::from(x), i64::from(y));
        (y - (x + i64::from(w) - 1))
            .max(x - (y + i64::from(v) - 1))
            .max(0) as u32
    };
    let dx = axis(a.x, a.space, b.x, b.space);
    let dy = axis(a.y, a.space, b.y, b.space);
    (dx.max(dy) + dx.min(dy) / 2) * 5
}
pub fn range_feet(
    range: SpellRange,
    cl: i32,
    multiplier: u32,
    fixed: Option<u32>,
) -> Result<Option<u32>, String> {
    let cl = u64::try_from(cl).map_err(|_| "Negative caster level")?;
    let base = match range {
        SpellRange::Close => Some(25 + 5 * (cl / 2)),
        SpellRange::Medium => Some(100 + 10 * cl),
        SpellRange::Long => Some(400 + 40 * cl),
        SpellRange::Fixed => fixed.map(u64::from),
        _ => None,
    };
    base.map(|n| {
        n.checked_mul(u64::from(multiplier))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("Range overflow".into())
    })
    .transpose()
}
fn unique(ids: &[String]) -> Result<(), String> {
    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err("Duplicate recipient ID".into());
    }
    Ok(())
}
impl Engine {
    pub fn save_scene_actor(
        &self,
        d: &CharacterDraft,
        id: &str,
        actor: SceneActor,
    ) -> Result<crate::class_play::PlayResult, String> {
        if id.trim().is_empty()
            || id.len() > 100
            || actor.space == 0
            || actor.space > 6
            || actor.x.unsigned_abs() > 10000
            || actor.y.unsigned_abs() > 10000
            || actor.touch_reach_feet > 100
        {
            return Err("Actor needs an ID, coordinates within 10000 squares, footprint 1–6, and reach 0–100 feet".into());
        }
        let types = [
            "aberration",
            "animal",
            "construct",
            "dragon",
            "elemental",
            "fey",
            "giant",
            "humanoid",
            "magical_beast",
            "monstrous_humanoid",
            "ooze",
            "outsider",
            "plant",
            "undead",
            "vermin",
            "object",
        ];
        if !types.contains(&actor.creature_type.as_str())
            || matches!(actor.body, TargetBody::Living)
                && ["construct", "undead", "object"].contains(&actor.creature_type.as_str())
            || matches!(
                actor.body,
                TargetBody::Construct | TargetBody::LivingConstruct
            ) && actor.creature_type != "construct"
            || actor.body == TargetBody::Undead && actor.creature_type != "undead"
            || actor.body == TargetBody::Object && actor.creature_type != "object"
        {
            return Err("Body and creature type disagree".into());
        }
        let mut state = d.class_play.clone();
        state.actors.insert(id.into(), actor);
        crate::feature_resolution::refresh_aura_conditions(&mut state);
        state.revision = state.revision.checked_add(1).ok_or("Revision overflow")?;
        Ok(crate::class_play::PlayResult {
            state,
            success: true,
            facts: vec![format!(
                "Updated scene actor {id}; existing tapestry membership is unchanged"
            )],
        })
    }
    pub fn weave_members(&self, d: &CharacterDraft, ids: &[String]) -> Result<(), String> {
        unique(ids)?;
        if !d.class_play.actors.contains_key("self") {
            return Err("Register caster scene actor with ID self".into());
        }
        for id in ids {
            if id == "self" {
                return Err("The caster is connected automatically and is not an ally slot".into());
            }
            let a = d
                .class_play
                .actors
                .get(id)
                .ok_or_else(|| format!("Unknown scene actor {id}"))?;
            if !a.living()
                || !a.allied
                || !a.willing()
                || !matches!(a.path, TargetPath::Clear | TargetPath::TransparentSolid)
            {
                return Err(format!(
                    "{id}: weaving requires a living, willing ally in sight"
                ));
            }
        }
        Ok(())
    }
    /// `storage` targets the tapestry itself. Only membership and the single
    /// casting decision bind now; creature legality and geometry bind at release.
    pub fn tapestry_targets(
        &self,
        d: &CharacterDraft,
        spell: &str,
        selection: &TapestryTargets,
        range: SpellRange,
        feet: Option<u32>,
        storage: bool,
    ) -> Result<TargetResolution, String> {
        unique(&selection.recipients)?;
        let t = d
            .class_play
            .resources
            .tapestry
            .as_ref()
            .ok_or("Weave a tapestry first")?;
        let def = self
            .pack
            .spells
            .values()
            .find(|s| s.name.eq_ignore_ascii_case(spell) || s.id.eq_ignore_ascii_case(spell))
            .ok_or("Unknown spell")?;
        let rule = def
            .profile
            .targeting
            .as_ref()
            .ok_or_else(|| format!("{}: structured targeting rules unavailable", def.name))?;
        if rule.choices.is_empty() {
            if selection.choice.as_ref().is_some_and(|c| !c.is_empty()) {
                return Err("This spell has no supported casting choice".into());
            }
        } else if !selection
            .choice
            .as_ref()
            .is_some_and(|c| rule.choices.contains(c))
        {
            return Err(format!(
                "Choose one shared spell option: {}",
                rule.choices.join(", ")
            ));
        }
        let caster = d
            .class_play
            .actors
            .get("self")
            .ok_or("Caster scene actor unavailable")?;
        let mut result = TargetResolution {
            eligible: vec![],
            rejected: vec![],
        };
        for id in &selection.recipients {
            if id != "self" && !t.members.contains(id) {
                return Err(format!("{id} is not connected to this tapestry"));
            }
            let a = d
                .class_play
                .actors
                .get(id)
                .ok_or_else(|| format!("Scene actor {id} unavailable"))?;
            if storage {
                result.eligible.push(id.clone());
                continue;
            }
            let dist = distance(caster, a);
            let reason = if a.body == TargetBody::Object || a.dead {
                Some("objects cannot receive tapestry spells")
            } else if rule.requires_willing && !a.willing() {
                Some("recipient is unwilling")
            } else if rule.living_only && !a.living() {
                Some("spell requires a living creature")
            } else if rule.allied_only && id != "self" && !a.allied {
                Some("spell requires an allied creature")
            } else if !rule.creature_types.is_empty()
                && !rule.creature_types.contains(&a.creature_type)
            {
                Some("creature type excluded by this spell")
            } else if id != "self"
                && matches!(a.path, TargetPath::Solid | TargetPath::TransparentSolid)
            {
                Some("solid barrier blocks line of effect")
            } else if range == SpellRange::Touch && dist > caster.touch_reach_feet {
                Some("outside touch reach")
            } else if range != SpellRange::Touch
                && dist > feet.ok_or("Numeric spell range unavailable")?
            {
                Some("outside spell range")
            } else if id != "self"
                && a.path == TargetPath::Obscured
                && dist > caster.touch_reach_feet
            {
                Some("target cannot be seen or touched")
            } else {
                None
            };
            if let Some(why) = reason {
                result.rejected.push(format!("{id}: {why} ({dist} feet)"));
            } else {
                result.eligible.push(id.clone());
            }
        }
        if !storage {
            if let Some(max) = rule.max_pair_distance_feet {
                for (i, id) in result.eligible.iter().enumerate() {
                    for other in &result.eligible[i + 1..] {
                        if distance(&d.class_play.actors[id], &d.class_play.actors[other]) > max {
                            return Err(format!("{id} and {other} exceed the spell's {max}-foot separation limit; select a smaller group"));
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}
