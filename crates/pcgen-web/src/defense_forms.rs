use super::{
    forms::{begin, check, checked, input, number, value},
    ui::escape,
};
use pcgen_core::{class_play::*, magic::Energy};
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
fn actors(out: &mut String, d: &CharacterDraft) {
    out.push_str("<label>Target<select name=\"target\">");
    for id in d.class_play.actors.keys() {
        let _ = write!(out, "<option>{}</option>", escape(id));
    }
    out.push_str("</select></label>");
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft) {
    out.push_str("<section id=\"defenses\"><h2>Damage & defenses</h2><p>Resolve targets of recorded spells, then review their HP. Enter target defenses and protections before resolving damage. Advance ongoing effects individually.</p>");
    if d.class_play.actors.is_empty() {
        out.push_str("<p class=\"empty-state\">Add a scene actor to set up defenses and HP.</p>");
    }
    for target in d.class_play.actors.keys() {
        actor_defenses(out, id, d, target);
    }
    for (target, hp) in &d.class_play.health {
        let _ = write!(
            out,
            "<p><strong>{}</strong>: HP {} / {}; temporary {}; {}</p>",
            escape(target),
            hp.current,
            hp.maximum,
            hp.temporary,
            if d.class_play.actors.get(target).is_some_and(|a| a.dead) {
                "dead / destroyed"
            } else if hp.current < 0 {
                if hp.stable {
                    "stable"
                } else {
                    "dying"
                }
            } else if hp.current == 0 {
                "disabled"
            } else {
                "alive"
            }
        );
    }
    out.push_str("<details><summary>Resolve a dying creature’s turn</summary>");
    begin(out, id, d.class_play.revision, "dying_turn");
    actors(out, d);
    input(
        out,
        "Dying turn: stabilization d100 (1-10 succeeds)",
        "percentile",
        "",
        "number",
    );
    out.push_str("<button>Resolve dying turn</button></form></details>");
    for (key, c) in &d.class_play.concentration_checks {
        let _ = write!(out, "<p>{}</p>", escape(&c.fact));
        if c.success.is_none()
            && d.class_play
                .spells
                .get(&c.spell)
                .is_some_and(|s| s.ended.is_none() && s.controller == SpellController::Incantatrix)
        {
            begin(out, id, d.class_play.revision, "damage_concentration");
            let _ = write!(
                out,
                "<input type=hidden name=check value=\"{}\">",
                escape(key)
            );
            input(out, "Concentration d20", "roll", "", "number");
            out.push_str("<button>Resolve concentration</button></form>");
        }
    }
    for (key, c) in d.class_play.cast_effects.iter().rev() {
        let _ = write!(
            out,
            "<details><summary>{} · {} · CL {} / DC {}</summary>",
            escape(key),
            escape(&c.spell),
            c.caster_level,
            c.save_dc
        );
        let _ = write!(
            out,
            "<p>Round {} / {} · {}</p>",
            (c.round + 1).min(c.rounds),
            c.rounds,
            if c.ended { "ended" } else { "active" }
        );
        if c.rounds > 1 && !c.ended {
            begin(out, id, d.class_play.revision, "advance_effect");
            let _ = write!(
                out,
                "<input type=hidden name=casting value=\"{}\">",
                escape(key)
            );
            check(out, "End now (neutralized or removed)", "end");
            out.push_str("<button>Advance one effect round / end</button></form>");
        }
        for (target, r) in &c.resolved {
            let _ = write!(
                out,
                "<p><strong>{}</strong>: {}</p>",
                escape(target),
                escape(&r.facts.join(" · "))
            );
        }
        if c.requires_touch {
            out.push_str("<p>Delivery is controlled by the held charge in Tactical actions. History cannot deliver another touch.</p>");
        } else if !c.ended {
            if let Some(rule) = &c.defense {
                let _ = write!(
                    out,
                    "<p>{:?} / {:?}; SR {}; penetration bonus {:+}; damage range {}</p>",
                    rule.save,
                    rule.on_save,
                    if rule.spell_resistance {
                        "applies"
                    } else {
                        "does not apply"
                    },
                    c.penetration_bonus,
                    c.damage_bounds
                        .map(|(a, b)| format!("{a}–{b}"))
                        .unwrap_or("unavailable".into())
                );
                begin(out, id, d.class_play.revision, "resolve_defenses");
                let _ = write!(
                    out,
                    "<input type=\"hidden\" name=\"casting\" value=\"{}\">",
                    escape(key)
                );
                actors(out, d);
                input(
                    out,
                    "Caster SR check d20 (when needed)",
                    "sr_roll",
                    "",
                    "number",
                );
                input(
                    out,
                    "Target saving throw d20 (when needed)",
                    "save_roll",
                    "",
                    "number",
                );
                input(
                    out,
                    "Damage roll before defenses (required unless prevented)",
                    "damage",
                    "",
                    "number",
                );
                input(
                    out,
                    "Massive damage Fortitude d20 (50+ damage, DC 15)",
                    "massive",
                    "",
                    "number",
                );
                input(
                    out,
                    "Effect die (d4 or d6, when needed)",
                    "effect",
                    "",
                    "number",
                );
                input(
                    out,
                    "Dimension Hop destination X",
                    "destination_x",
                    "",
                    "number",
                );
                input(
                    out,
                    "Dimension Hop destination Y",
                    "destination_y",
                    "",
                    "number",
                );
                out.push_str("<label>Destination visibility<select name=destination_path><option value=unknown>Unrecorded</option><option value=clear>Visible clear path</option><option value=transparent>Visible through transparent barrier</option></select></label>");
                for effect in d.class_play.combat_state.effects.values() {
                    input(
                        out,
                        &format!(
                            "Dispel {} on {} (CL {}) d20",
                            effect.name, effect.target, effect.caster_level
                        ),
                        &format!("dispel_{}", effect.casting),
                        "",
                        "number",
                    );
                }
                check(out, "Target voluntarily waives saving throw", "waive");
                check(
                    out,
                    "Target has lowered SR for this interval (standard action already taken)",
                    "lowered",
                );
                out.push_str("<button>Resolve this event and apply HP</button></form>");
            } else {
                out.push_str(
                    "<p>Unavailable: this spell needs a structured saving throw / SR rule.</p>",
                );
            }
        }
        out.push_str("</details>");
    }
    out.push_str("</section>");
}
pub(super) fn execute(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    let signed = |k: &str| {
        value(f, k)?
            .parse::<i32>()
            .map_err(|_| format!("Enter a numeric {k}"))
    };
    let roll = |k: &str| -> Result<Option<u32>, String> {
        let s = value(f, k)?;
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s.parse().map_err(|_| format!("Invalid {k}"))?))
        }
    };
    if value(f, "op")? == "damage_concentration" {
        return e.resolve_damage_concentration(d, value(f, "check")?, number(f, "roll")?);
    }
    if value(f, "op")? == "dying_turn" {
        return e.dying_turn(d, value(f, "target")?, number(f, "percentile")?);
    }
    if value(f, "op")? == "advance_effect" {
        return e.advance_damage_effect(d, value(f, "casting")?, f.contains_key("end"));
    }
    if value(f, "op")? == "save_health" {
        let mut p = Protections::default();
        for (key, energy) in energies() {
            p.resistance.insert(energy, number(f, key)?);
            if f.contains_key(&format!("immune_{key}")) {
                p.immunity.push(energy);
            }
        }
        p.evasion = match value(f, "evade")? {
            "none" => Evasion::None,
            "evasion" => Evasion::Evasion,
            "improved" => Evasion::Improved,
            _ => return Err("Invalid evasion".into()),
        };
        p.armor_blocks_evasion = f.contains_key("armor_blocks");
        p.helpless = f.contains_key("helpless");
        p.bound_or_squeezing = f.contains_key("bound");
        p.spell_immunity = f.contains_key("spell_immunity");
        return e.save_encounter_health(
            d,
            value(f, "target")?,
            HitPoints {
                maximum: i64::from(signed("maximum")?),
                current: i64::from(signed("current")?),
                temporary: i64::from(signed("temporary")?),
                stable: f.contains_key("stable"),
            },
            p,
        );
    }
    if value(f, "op")? == "save_defenses" {
        e.save_spell_defenses(
            d,
            value(f, "target")?,
            SpellDefenses {
                fortitude: signed("fortitude")?,
                reflex: signed("reflex")?,
                will: signed("will")?,
                spell_resistance: number(f, "sr")?,
            },
        )
    } else {
        let damage = value(f, "damage")?;
        e.resolve_spell_defenses(
            d,
            value(f, "casting")?,
            &DefenseRequest {
                effect_roll: roll("effect")?,
                dispel_rolls: f
                    .iter()
                    .filter_map(|(k, v)| {
                        k.strip_prefix("dispel_")
                            .filter(|_| !v.is_empty())
                            .map(|id| (id.to_owned(), v))
                    })
                    .map(|(id, v)| {
                        v.parse()
                            .map(|n| (id, n))
                            .map_err(|_| "Invalid dispel roll")
                    })
                    .collect::<Result<_, _>>()?,
                destination: if value(f, "destination_x")?.is_empty()
                    && value(f, "destination_y")?.is_empty()
                {
                    None
                } else {
                    Some((signed("destination_x")?, signed("destination_y")?))
                },
                destination_path: match f.get("destination_path").map(String::as_str) {
                    Some("clear") => Some(TargetPath::Clear),
                    Some("transparent") => Some(TargetPath::TransparentSolid),
                    _ => None,
                },
                target: value(f, "target")?.into(),
                massive_roll: roll("massive")?,
                sr_roll: roll("sr_roll")?,
                save_roll: roll("save_roll")?,
                waive_save: f.contains_key("waive"),
                sr_lowered: f.contains_key("lowered"),
                damage_roll: if damage.is_empty() {
                    None
                } else {
                    Some(damage.parse().map_err(|_| "Invalid damage")?)
                },
            },
        )
    }
}

fn energies() -> [(&'static str, Energy); 5] {
    [
        ("acid", Energy::Acid),
        ("cold", Energy::Cold),
        ("electricity", Energy::Electricity),
        ("fire", Energy::Fire),
        ("sonic", Energy::Sonic),
    ]
}

fn actor_defenses(out: &mut String, id: &str, d: &CharacterDraft, target: &str) {
    let _ = write!(
        out,
        r#"<details><summary>{} · HP &amp; defenses</summary>"#,
        escape(target)
    );
    let def = d.class_play.defenses.get(target);
    begin(out, id, d.class_play.revision, "save_defenses");
    let _ = write!(
        out,
        r#"<input type="hidden" name="target" value="{}"><h3>Saving throws &amp; spell resistance</h3>"#,
        escape(target)
    );
    for (label, key, v) in [
        (
            "Fortitude total",
            "fortitude",
            def.map(|d| i64::from(d.fortitude)),
        ),
        ("Reflex total", "reflex", def.map(|d| i64::from(d.reflex))),
        ("Will total", "will", def.map(|d| i64::from(d.will))),
        (
            "Spell resistance (0 = none)",
            "sr",
            def.map(|d| i64::from(d.spell_resistance)),
        ),
    ] {
        input(
            out,
            label,
            key,
            &v.map(|n| n.to_string()).unwrap_or_default(),
            "number",
        );
    }
    out.push_str("<button>Save target defenses</button></form>");
    begin(out, id, d.class_play.revision, "save_health");
    let _ = write!(
        out,
        r#"<input type="hidden" name="target" value="{}"><h3>HP &amp; protections</h3><p>Saved values are filled in below. For energy resistance, use the highest applicable value. A blank HP field means it has not been recorded.</p>"#,
        escape(target)
    );
    let hp = d.class_play.health.get(target);
    for (label, key, v) in [
        ("Maximum HP", "maximum", hp.map(|h| h.maximum)),
        ("Current HP", "current", hp.map(|h| h.current)),
        ("Temporary HP", "temporary", hp.map(|h| h.temporary)),
    ] {
        input(
            out,
            label,
            key,
            &v.map(|n| n.to_string()).unwrap_or_default(),
            "number",
        );
    }
    checked(
        out,
        "Stable at negative HP",
        "stable",
        hp.is_some_and(|h| h.stable),
    );
    let p = d.class_play.protections.get(target);
    for (key, energy) in energies() {
        let n = p
            .and_then(|p| p.resistance.get(&energy))
            .copied()
            .unwrap_or(0);
        input(
            out,
            &format!("{key} resistance (0 = none)"),
            key,
            &n.to_string(),
            "number",
        );
        checked(
            out,
            &format!("Immune to {key}"),
            &format!("immune_{key}"),
            p.is_some_and(|p| p.immunity.contains(&energy)),
        );
    }
    out.push_str(r#"<label>Evasion<select name="evade">"#);
    for (key, label, e) in [
        ("none", "None", Evasion::None),
        ("evasion", "Evasion", Evasion::Evasion),
        ("improved", "Improved evasion", Evasion::Improved),
    ] {
        let _ = write!(
            out,
            r#"<option value="{}" {}>{}</option>"#,
            key,
            if p.map(|p| p.evasion).unwrap_or(Evasion::None) == e {
                "selected"
            } else {
                ""
            },
            label
        );
    }
    out.push_str("</select></label>");
    checked(
        out,
        "Worn armor blocks this evasion ability",
        "armor_blocks",
        p.is_some_and(|p| p.armor_blocks_evasion),
    );
    checked(out, "Helpless", "helpless", p.is_some_and(|p| p.helpless));
    checked(
        out,
        "Bound or squeezing",
        "bound",
        p.is_some_and(|p| p.bound_or_squeezing),
    );
    checked(
        out,
        "Spell immunity (SR-based)",
        "spell_immunity",
        p.is_some_and(|p| p.spell_immunity),
    );
    out.push_str("<button>Record encounter HP and protections</button></form></details>");
}
