//! Ordinary forms for executable class abilities; the engine owns all arithmetic.
use super::{
    forms::{begin, checked, input, number, value},
    ui::escape,
};
use pcgen_core::{class_play::*, magic::*};
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
fn actor_select(out: &mut String, d: &CharacterDraft) {
    out.push_str("<label>Target<select name=target>");
    for id in d.class_play.actors.keys() {
        let _ = write!(out, "<option>{}</option>", escape(id));
    }
    out.push_str("</select></label>");
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft, engine: &Engine) {
    super::necromancy_forms::render(out, id, d, engine);
    super::combat_forms::render(out,id,d,engine);
    super::tactical_forms::render(out, id, d, engine);
    out.push_str("<section id=class-abilities><h2>Resolve class abilities</h2><p>Actions update encounter HP and saved resources together. Area abilities include every eligible actor in range; enter all required saves before submitting.</p>");
    for (target, a) in &d.class_play.actors {
        let _ = write!(
            out,
            "<details><summary>{} · combat facts</summary>",
            escape(target)
        );
        begin(out, id, d.class_play.revision, "combat_facts");
        let _ = write!(
            out,
            "<input type=hidden name=target value=\"{}\">",
            escape(target)
        );
        let c = d.class_play.encounter.combat.get(target);
        checked(
            out,
            "Immune to fear / mind-affecting fear",
            "fear_immune",
            c.is_some_and(|c| c.fear_immune),
        );
        for (label, name, n) in [
            ("Touch AC", "touch_ac", c.map(|c| i64::from(c.touch_ac))),
            ("Hit Dice", "hit_dice", c.map(|c| i64::from(c.hit_dice))),
            (
                "Intelligence (0 for mindless)",
                "intelligence",
                c.map(|c| i64::from(c.intelligence)),
            ),
            (
                "Sense Motive total",
                "sense_motive",
                c.map(|c| i64::from(c.sense_motive)),
            ),
            (
                "Base attack bonus",
                "base_attack_bonus",
                c.map(|c| i64::from(c.base_attack_bonus)),
            ),
        ] {
            input(
                out,
                label,
                name,
                &n.map(|n| n.to_string()).unwrap_or_default(),
                "number",
            );
        }
        let _ = a;
        out.push_str("<button>Save combat facts</button></form></details>");
    }
    match engine.class_feature_values(d) {
        Ok(features) => {
            for f in features {
                let Some(rule) = f.resolution else { continue };
                if rule == FeatureResolution::QuickCast {
                    continue;
                }
                let _ = write!(
                    out,
                    "<details><summary>{} · {}</summary>",
                    escape(&f.class_id),
                    escape(&f.name)
                );
                begin(out, id, d.class_play.revision, "resolve_feature");
                let _=write!(out,"<input type=hidden name=class value=\"{}\"><input type=hidden name=feature value=\"{}\">",escape(&f.class_id),escape(&f.name));
                match rule {
                    FeatureResolution::KnightChallenge { mode } => {
                        if mode == pcgen_core::tactics::ChallengeMode::Fighting {
                            actor_select(out, d);
                        } else {
                            for (i, target) in d.class_play.actors.keys().enumerate() {
                                let _ = write!(
                                    out,
                                    "<input type=hidden name=target_{i} value=\"{}\">",
                                    escape(target)
                                );
                                input(
                                    out,
                                    &format!("{target}: Will d20 (if eligible)"),
                                    &format!("save_{i}"),
                                    "",
                                    "number",
                                );
                            }
                        }
                    }
                    FeatureResolution::BreathWeapon => {
                        out.push_str("<label>Direction<select name=target><option value=east>East</option><option value=west>West</option><option value=north>North</option><option value=south>South</option></select></label>");
                        input(
                            out,
                            "Shared breath damage roll",
                            "damage_roll",
                            "",
                            "number",
                        );
                        input(out, "Recharge d4", "opposed_roll", "", "number");
                        for (i, target) in d.class_play.actors.keys().enumerate() {
                            let _ = write!(
                                out,
                                "<input type=hidden name=target_{i} value=\"{}\">",
                                escape(target)
                            );
                            input(
                                out,
                                &format!("{target}: Reflex d20 (if in area)"),
                                &format!("save_{i}"),
                                "",
                                "number",
                            );
                            input(
                                out,
                                &format!("{target}: massive-damage Fortitude d20 (if needed)"),
                                &format!("massive_{i}"),
                                "",
                                "number",
                            );
                        }
                    }
                    FeatureResolution::QuickCast => unreachable!(),
                    FeatureResolution::Vitality {
                        touch,
                        radius_feet,
                        dice_sides,
                        points,
                        self_only,
                        ..
                    } => {
                        if radius_feet.is_none() {
                            if self_only {
                                out.push_str("<input type=hidden name=target value=self>");
                            } else {
                                actor_select(out, d);
                            }
                        }
                        if touch {
                            super::necromancy_forms::touch_inputs(out, d);
                            input(
                                out,
                                "Melee touch attack d20 (only for damaging touches)",
                                "roll",
                                "",
                                "number",
                            );
                        }
                        if dice_sides.is_some() {
                            input(
                                out,
                                "Damage roll (one total for an area)",
                                "damage_roll",
                                "",
                                "number",
                            );
                        }
                        if points {
                            input(out, "Points to spend", "amount", "1", "number");
                        }
                        if radius_feet.is_some() {
                            out.push_str("<p>Living targets make the listed save; undead are healed by negative energy.</p>");
                        }
                        for (i, target) in d.class_play.actors.keys().enumerate() {
                            let _ = write!(
                                out,
                                "<input type=hidden name=target_{i} value=\"{}\">",
                                escape(target)
                            );
                            input(
                                out,
                                &format!("{target}: save d20 (if required)"),
                                &format!("save_{i}"),
                                "",
                                "number",
                            );
                            input(
                                out,
                                &format!("{target}: massive damage Fortitude d20 (if required)"),
                                &format!("massive_{i}"),
                                "",
                                "number",
                            );
                        }
                    }
                    FeatureResolution::FearAura { .. } => {
                        for (i, target) in d.class_play.actors.keys().enumerate() {
                            let _ = write!(
                                out,
                                "<input type=hidden name=target_{i} value=\"{}\">",
                                escape(target)
                            );
                            input(
                                out,
                                &format!("{target}: Will save d20"),
                                &format!("save_{i}"),
                                "",
                                "number",
                            );
                        }
                    }
                    FeatureResolution::SurpriseFeint => {
                        actor_select(out, d);
                        input(out, "Your Bluff d20", "roll", "", "number");
                        input(
                            out,
                            "Opponent Sense Motive d20",
                            "opposed_roll",
                            "",
                            "number",
                        );
                    }
                    FeatureResolution::Rage => {
                        out.push_str("<p>Start during your own action. Actual HP increases with Constitution and is removed when rage ends. One rage per encounter; spellcasting and spell-trigger items are blocked while raging.</p>");
                    }
                }
                out.push_str("<button>Resolve &amp; apply ability</button></form></details>");
            }
        }
        Err(e) => {
            let _ = write!(out, "<p class=error>{}</p>", escape(&e));
        }
    }
    for c in &d.class_play.encounter.conditions {
        let _ = write!(
            out,
            "<p><strong>{}</strong>: {} ({})</p>",
            escape(&c.target),
            escape(&c.name),
            escape(&c.source)
        );
    }
    for (target, until) in &d.class_play.encounter.feints {
        let _ = write!(
            out,
            "<p>{}: Surprise Casting until ability clock {until}; remain in melee.</p>",
            escape(target)
        );
    }
    if let Some(r) = &d.class_play.encounter.rage {
        let _ = write!(
            out,
            "<p>Raging: {} seconds remain.</p>",
            r.remaining_seconds
        );
        begin(out, id, d.class_play.revision, "end_rage");
        out.push_str("<button>End rage</button></form>");
    }
    if d.class_play.encounter.fatigued {
        out.push_str("<p>Fatigued: STR -2, DEX -2; cannot run or charge.</p>");
    }
    begin(out, id, d.class_play.revision, "ability_time");
    input(
        out,
        "Elapsed ability time (seconds)",
        "seconds",
        "6",
        "number",
    );
    out.push_str("<button>Advance ability time</button></form>");
    begin(out, id, d.class_play.revision, "end_ability_encounter");
    out.push_str("<button>End encounter &amp; clear encounter fatigue</button></form></section>");
}
fn optional<T: std::str::FromStr>(
    f: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<T>, String> {
    f.get(key)
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.parse().map_err(|_| format!("Invalid {key}")))
        .transpose()
}
pub(super) fn execute(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        "combat_facts" => e.save_combat_facts(
            d,
            value(f, "target")?,
            CombatFacts {
                fear_immune: f.contains_key("fear_immune"),
                touch_ac: value(f, "touch_ac")?
                    .parse()
                    .map_err(|_| "Invalid touch AC")?,
                hit_dice: number(f, "hit_dice")?,
                intelligence: number(f, "intelligence")?,
                sense_motive: value(f, "sense_motive")?
                    .parse()
                    .map_err(|_| "Invalid Sense Motive")?,
                base_attack_bonus: value(f, "base_attack_bonus")?
                    .parse()
                    .map_err(|_| "Invalid BAB")?,
            },
        ),
        "resolve_feature" => {
            let mut r = FeatureRequest {
                affliction: super::necromancy_forms::parse_rolls(f)?,
                class_id: value(f, "class")?.into(),
                feature: value(f, "feature")?.into(),
                target: f.get("target").cloned().unwrap_or_default(),
                roll: optional(f, "roll")?,
                opposed_roll: optional(f, "opposed_roll")?,
                damage_roll: optional(f, "damage_roll")?,
                amount: optional(f, "amount")?.unwrap_or(0),
                ..Default::default()
            };
            for (k, target) in f.iter().filter(|(k, _)| k.starts_with("target_")) {
                let i = &k[7..];
                if let Some(n) = optional(f, &format!("save_{i}"))? {
                    r.saves.insert(target.clone(), n);
                }
                if let Some(n) = optional(f, &format!("massive_{i}"))? {
                    r.massive_rolls.insert(target.clone(), n);
                }
            }
            e.resolve_class_feature(d, &r)
        }
        "end_rage" => e.end_rage(d),
        "ability_time" => e.advance_ability_time(d, number(f, "seconds")?),
        "end_ability_encounter" => e.end_ability_encounter(d),
        _ => Err("Unknown class ability operation".into()),
    }
}
