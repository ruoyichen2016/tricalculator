//! Static, typed forms for persistent necromantic state.
use super::{
    forms::{begin, checked, input, number, value},
    ui::escape,
};
use pcgen_core::necromancy::*;
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
fn hidden(out: &mut String, name: &str, v: &str) {
    let _ = write!(
        out,
        "<input type=hidden name=\"{}\" value=\"{}\">",
        escape(name),
        escape(v)
    );
}
fn rolls(out: &mut String, rule: &DiseaseRule) {
    input(out, "Fortitude d20", "affliction_save", "", "number");
    if rule.blindness || rule.drain {
        input(
            out,
            "Secondary Fortitude d20 (blindness or permanent drain)",
            "secondary_save",
            "",
            "number",
        );
    }
    for key in rule.damage.keys() {
        input(
            out,
            &format!("{key} disease damage roll (only when required)"),
            &format!("damage_{key}"),
            "",
            "number",
        );
    }
}
pub(super) fn touch_inputs(out: &mut String, d: &CharacterDraft) {
    if let Some(armed) = &d.class_play.necromancy.armed_disease {
        out.push_str("<details open><summary>Armed disease touch rolls</summary>");
        rolls(out, &armed.rule);
        out.push_str("</details>");
    }
}
pub(super) fn parse_rolls(f: &BTreeMap<String, String>) -> Result<AfflictionRolls, String> {
    fn optional(f: &BTreeMap<String, String>, k: &str) -> Result<Option<u32>, String> {
        f.get(k)
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.parse().map_err(|_| format!("Invalid {k}")))
            .transpose()
    }
    let mut r = AfflictionRolls {
        save: optional(f, "affliction_save")?,
        secondary_save: optional(f, "secondary_save")?,
        ..Default::default()
    };
    for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
        if let Some(n) = optional(f, &format!("damage_{key}"))? {
            r.damage.insert(key.into(), n);
        }
    }
    Ok(r)
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft, e: &Engine) {
    out.push_str("<section id=necromancy><h2>Afflictions &amp; undead allegiance</h2><p>Persistent state survives encounter end. Advance the ability clock before daily checks. Saves and damage rolls commit together.</p>");
    if d.class_play.necromancy.lich {
        out.push_str("<p><strong>Lich transformation applied.</strong> Undead type; no Constitution; d12 Hit Dice; free phylactery. Undead body immunities are included in the character snapshot.</p>");
    }
    for target in d.class_play.actors.keys() {
        let _ = write!(
            out,
            "<details><summary>{}: affliction facts</summary>",
            escape(target)
        );
        begin(
            out,
            id,
            d.class_play.revision,
            if d.class_play
                .necromancy
                .actors
                .get(target)
                .is_some_and(|a| a.needs_level_rebuild)
            {
                "necro_rebuild"
            } else {
                "necro_facts"
            },
        );
        hidden(out, "target", target);
        let a = d.class_play.necromancy.actors.get(target);
        if a.is_some_and(|a| a.needs_level_rebuild) {
            out.push_str("<p class=error>Unavailable: permanent level loss requires rebuilt class-dependent facts.</p>");
        }
        for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
            let v = a
                .and_then(|a| a.scores.get(key))
                .or_else(|| (target == "self").then(|| d.stats.get(key)).flatten())
                .map(|n| n.to_string())
                .unwrap_or_default();
            input(out, &format!("Baseline {key}"), key, &v, "number");
        }
        checked(
            out,
            "Disease immunity",
            "immune_disease",
            a.is_some_and(|a| a.immune_disease),
        );
        checked(
            out,
            "Energy drain immunity",
            "immune_drain",
            a.is_some_and(|a| a.immune_drain),
        );
        input(
            out,
            "Turn resistance",
            "turn_resistance",
            &a.map(|a| a.turn_resistance.to_string())
                .unwrap_or_else(|| "0".into()),
            "number",
        );
        if a.is_some_and(|a| a.needs_level_rebuild) {
            for (label, key) in [
                ("Post-loss Hit Dice", "hit_dice"),
                ("Touch AC", "touch_ac"),
                ("Intelligence", "intelligence"),
                ("Sense Motive", "sense_motive"),
                ("Base attack bonus", "bab"),
                ("Fortitude total", "fort"),
                ("Reflex total", "reflex"),
                ("Will total", "will"),
                ("Spell resistance", "sr"),
            ] {
                input(out, label, key, "", "number");
            }
            checked(out, "Fear immunity", "fear_immune", false);
        }
        out.push_str("<button>Save affliction facts</button></form>");
        if let Some(a) = a {
            for (key, n) in &a.damage {
                let _ = write!(
                    out,
                    "<p>{}: {n} {key} damage; {} permanent drain</p>",
                    escape(target),
                    a.drain.get(key).copied().unwrap_or(0)
                );
            }
        }
        begin(out, id, d.class_play.revision, "necro_recover");
        hidden(out, "target", target);
        checked(out, "Complete bed rest for this day", "bed_rest", false);
        out.push_str("<button>Apply one day of ability recovery</button></form></details>");
    }
    if let Ok(features) = e.class_feature_values(d) {
        if features
            .iter()
            .any(|f| f.class_id == "Dread Necromancer" && f.name == "Scabrous Touch")
        {
            begin(out, id, d.class_play.revision, "necro_arm_disease");
            out.push_str("<label>Disease<select name=disease>");
            // Catalog lives in native rules; display the acquired class's seven contagion choices.
            for rule in e.necromancy_diseases() {
                let _ = write!(out, "<option>{}</option>", escape(&rule.name));
            }
            out.push_str("</select></label><button>Arm Scabrous Touch (swift)</button></form>");
        }
        if features
            .iter()
            .any(|f| f.class_id == "Dread Necromancer" && f.name == "Enervating Touch")
        {
            begin(out, id, d.class_play.revision, "necro_arm_drain");
            out.push_str("<label>Negative levels<select name=count><option>1</option><option>2</option></select></label><button>Arm Enervating Touch (swift)</button></form>");
        }
        if features
            .iter()
            .any(|f| f.class_id == "Dread Necromancer" && f.name == "Lich Transformation")
            && !d.class_play.necromancy.lich
        {
            begin(out, id, d.class_play.revision, "necro_lich");
            for (i, _) in d.advancement.iter().flat_map(|f| &f.lanes).enumerate() {
                input(
                    out,
                    &format!("Hit Die {}: d12 reroll", i + 1),
                    &format!("hp_{i}"),
                    "",
                    "number",
                );
            }
            out.push_str("<button>Transform into a lich</button></form>");
        }
    }
    for spell in ["Animate Dead", "Control Undead"] {
        for class in d.class_levels() {
            if !e
                .spell_catalog(d, &class.class_id)
                .is_ok_and(|list| list.iter().any(|x| x.spell == spell && x.known))
            {
                continue;
            }
            let _ = write!(
                out,
                "<details><summary>{}: cast {}</summary>",
                escape(&class.class_id),
                spell
            );
            begin(out, id, d.class_play.revision, "necro_spell");
            hidden(out, "class", &class.class_id);
            hidden(out, "spell", spell);
            if spell == "Animate Dead" {
                checked(out, "Create zombies (otherwise skeletons)", "zombie", false);
                checked(
                    out,
                    "Remains are mostly intact and have the required bones / true anatomy",
                    "intact_remains",
                    false,
                );
                checked(
                    out,
                    "Corpses are within an active desecrate area",
                    "desecrate",
                    false,
                );
                checked(
                    out,
                    "Desecrate includes an altar dedicated to the caster's patron",
                    "desecrate_altar",
                    false,
                );
                input(out, "Black onyx provided (gp)", "onyx_gp", "0", "number");
                for target in d.class_play.necromancy.animated.keys() {
                    checked(
                        out,
                        &format!("Release previous subject {target}"),
                        &format!("release_{target}"),
                        false,
                    );
                }
            }
            for (i, (target, a)) in d.class_play.actors.iter().enumerate() {
                if target == "self"
                    || (spell == "Animate Dead" && !a.dead)
                    || (spell == "Control Undead"
                        && (a.dead || a.body != pcgen_core::class_play::TargetBody::Undead))
                {
                    continue;
                }
                hidden(out, &format!("subject_{i}"), target);
                checked(
                    out,
                    &format!("Target {target}"),
                    &format!("selected_{i}"),
                    false,
                );
                if spell == "Animate Dead" {
                    input(
                        out,
                        &format!("{target}: racial HD only (0 for a creature with class HD only)"),
                        &format!("racial_{i}"),
                        "",
                        "number",
                    );
                    input(
                        out,
                        &format!("{target}: new d12 HP roll total"),
                        &format!("hp_total_{i}"),
                        "",
                        "number",
                    );
                } else {
                    input(
                        out,
                        &format!("{target}: Will d20"),
                        &format!("will_{i}"),
                        "",
                        "number",
                    );
                    input(
                        out,
                        &format!("{target}: caster-level d20 against SR (if any)"),
                        &format!("sr_{i}"),
                        "",
                        "number",
                    );
                }
            }
            out.push_str("<button>Cast &amp; resolve undead</button></form></details>");
        }
    }
    for (target, bond) in d
        .class_play
        .necromancy
        .animated
        .iter()
        .chain(&d.class_play.necromancy.spell_bonds)
    {
        let _ = write!(
            out,
            "<p>{}: {} ({} HD); {}</p>",
            escape(target),
            escape(&bond.source),
            bond.hit_dice,
            escape(&bond.orders)
        );
        begin(out, id, d.class_play.revision, "necro_order");
        hidden(out, "target", target);
        input(out, "Spoken order", "order", "", "text");
        out.push_str("<button>Issue order</button></form>");
    }
    if let Some(a) = &d.class_play.necromancy.armed_disease {
        let _ = write!(
            out,
            "<p>Armed: {} (initial DC {})</p>",
            escape(&a.rule.name),
            a.dc
        );
    }
    if let Some(a) = &d.class_play.necromancy.armed_drain {
        let _ = write!(
            out,
            "<p>Armed: {} negative levels (24-hour removal DC {})</p>",
            a.count, a.dc
        );
    }
    for (i, c) in d.class_play.necromancy.diseases.iter().enumerate() {
        let _=write!(out,"<article><h3>{}: {}</h3><p>Next check at {} seconds; disease DC {}; consecutive successes {}/2.</p>",escape(&c.target),escape(&c.rule.name),c.due,c.rule.dc,c.successes);
        begin(out, id, d.class_play.revision, "necro_disease_day");
        hidden(out, "index", &i.to_string());
        rolls(out, &c.rule);
        out.push_str("<button>Resolve next disease day</button></form>");
        begin(out, id, d.class_play.revision, "necro_cure");
        hidden(out, "index", &i.to_string());
        out.push_str("<button>Record external disease cure</button></form></article>");
    }
    for (i, n) in d.class_play.necromancy.negative_levels.iter().enumerate() {
        let _=write!(out,"<article><h3>{}: negative level</h3><p>Individual Fortitude DC {}; due at {} seconds.</p>",escape(&n.target),n.dc,n.due);
        begin(out, id, d.class_play.revision, "necro_negative_day");
        hidden(out, "index", &i.to_string());
        input(out, "Fortitude d20", "roll", "", "number");
        if n.target == "self" {
            out.push_str("<p>On a failed save, remove the most recent level. Record ranks and any feat acquired at that level; blank ability uses the RAW highest-score fallback.</p><label>Lost ability increase<select name=lost_ability><option value=''>Unknown (highest score)</option>");
            for k in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                let _ = write!(out, "<option>{k}</option>");
            }
            out.push_str("</select></label><label>Lost feat<select name=lost_feat><option value=''>None / recorded history</option>");
            for (_, name) in &d.abilities {
                let _ = write!(out, "<option>{}</option>", escape(name));
            }
            out.push_str("</select></label>");
            for (j, (name, _)) in d.skills.iter().enumerate() {
                hidden(out, &format!("skill_{j}"), name);
                input(
                    out,
                    &format!("{name}: ranks gained at lost level"),
                    &format!("ranks_{j}"),
                    "0",
                    "number",
                );
            }
        } else {
            input(
                out,
                "HP gained at victim's most recent level (on failure)",
                "lost_hp",
                "",
                "number",
            );
        }
        out.push_str("<button>Resolve this negative level</button></form></article>");
    }
    if e.evaluate(d)
        .turning
        .is_some_and(|t| t.mode == pcgen_engine::TurningMode::Rebuke)
    {
        begin(out, id, d.class_play.revision, "necro_rebuke");
        input(out, "Turning check d20", "roll", "", "number");
        input(out, "Turning damage d6 #1", "die1", "", "number");
        input(out, "Turning damage d6 #2", "die2", "", "number");
        out.push_str("<p>All eligible undead within 60 feet are processed nearest first; currently commanded subjects are ignored.</p>");
        out.push_str("<button>Rebuke undead</button></form>");
    }
    for (target, hd) in &d.class_play.necromancy.commands {
        let _ = write!(out, "<p>{}: commanded ({hd} HD)</p>", escape(target));
        begin(out, id, d.class_play.revision, "necro_release");
        hidden(out, "target", target);
        out.push_str("<button>Relinquish command</button></form>");
    }
    out.push_str("</section>");
}
pub(super) fn execute(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        "necro_order" => e.order_undead(d, value(f, "target")?, value(f, "order")?),
        "necro_spell" => {
            let mut r = UndeadSpellRequest {
                cast: pcgen_core::magic::CastRequest {
                    class_id: value(f, "class")?.into(),
                    spell: value(f, "spell")?.into(),
                    ..Default::default()
                },
                zombie: f.contains_key("zombie"),
                intact_remains: f.contains_key("intact_remains"),
                desecrated_altar: f.contains_key("desecrate"),
                desecrate_altar: f.contains_key("desecrate_altar"),
                onyx_gp: f
                    .get("onyx_gp")
                    .filter(|v| !v.is_empty())
                    .map(|v| v.parse().map_err(|_| "Invalid onyx value"))
                    .transpose()?
                    .unwrap_or(0),
                ..Default::default()
            };
            for (key, target) in f.iter().filter(|(k, _)| k.starts_with("subject_")) {
                let i = &key[8..];
                if !f.contains_key(&format!("selected_{i}")) {
                    continue;
                }
                r.targets.push(target.clone());
                for (prefix, map) in [
                    ("racial", &mut r.racial_hit_dice),
                    ("hp_total", &mut r.hp_totals),
                    ("will", &mut r.saves),
                    ("sr", &mut r.penetration),
                ] {
                    if let Some(v) = f.get(&format!("{prefix}_{i}")).filter(|v| !v.is_empty()) {
                        map.insert(
                            target.clone(),
                            v.parse().map_err(|_| format!("Invalid {prefix} roll"))?,
                        );
                    }
                }
            }
            r.release = f
                .keys()
                .filter_map(|k| k.strip_prefix("release_").map(str::to_string))
                .collect();
            e.resolve_undead_spell(d, &r)
        }
        "necro_rebuild" => {
            let signed = |k: &str| -> Result<i32, String> {
                value(f, k)?.parse().map_err(|_| format!("Invalid {k}"))
            };
            let mut scores = BTreeMap::new();
            for k in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                scores.insert(k.into(), signed(k)?);
            }
            e.rebuild_drained_actor(
                d,
                value(f, "target")?,
                scores,
                pcgen_core::class_play::CombatFacts {
                    hit_dice: number(f, "hit_dice")?,
                    touch_ac: signed("touch_ac")?,
                    intelligence: number(f, "intelligence")?,
                    sense_motive: signed("sense_motive")?,
                    base_attack_bonus: signed("bab")?,
                    fear_immune: f.contains_key("fear_immune"),
                },
                pcgen_core::class_play::SpellDefenses {
                    fortitude: signed("fort")?,
                    reflex: signed("reflex")?,
                    will: signed("will")?,
                    spell_resistance: number(f, "sr")?,
                },
            )
        }
        "necro_facts" => {
            let mut a = AfflictedActor {
                immune_disease: f.contains_key("immune_disease"),
                immune_drain: f.contains_key("immune_drain"),
                turn_resistance: number(f, "turn_resistance")?,
                ..Default::default()
            };
            for k in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                a.scores.insert(
                    k.into(),
                    f.get(k)
                        .filter(|v| !v.is_empty())
                        .map(|v| v.parse().map_err(|_| "Invalid ability score"))
                        .transpose()?
                        .unwrap_or(0),
                );
            }
            e.save_affliction_actor(d, value(f, "target")?, a)
        }
        "necro_arm_disease" => e.arm_necromantic_touch(d, Some(value(f, "disease")?), 1),
        "necro_arm_drain" => e.arm_necromantic_touch(d, None, number(f, "count")?),
        "necro_disease_day" => {
            e.resolve_disease_day(d, number(f, "index")? as usize, &parse_rolls(f)?)
        }
        "necro_cure" => e.cure_disease(d, number(f, "index")? as usize),
        "necro_recover" => {
            e.recover_ability_damage(d, value(f, "target")?, f.contains_key("bed_rest"))
        }
        "necro_rebuke" => e.rebuke_undead(
            d,
            number(f, "roll")?,
            [number(f, "die1")?, number(f, "die2")?],
            &f.keys()
                .filter_map(|k| k.strip_prefix("undead_").map(str::to_string))
                .collect::<Vec<_>>(),
        ),
        "necro_release" => e.release_rebuked_undead(d, value(f, "target")?),
        _ => Err("Unknown necromancy action".into()),
    }
}
pub(super) fn character_change(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<(CharacterDraft, Vec<String>), String> {
    if value(f, "op")? == "necro_lich" {
        let rolls = (0..d.advancement.iter().map(|f| f.lanes.len()).sum::<usize>())
            .map(|i| number(f, &format!("hp_{i}")))
            .collect::<Result<Vec<_>, _>>()?;
        return e.transform_dread_lich(d, &rolls);
    }
    let mut ranks = BTreeMap::new();
    for (k, name) in f.iter().filter(|(k, _)| k.starts_with("skill_")) {
        let n = value(f, &format!("ranks_{}", &k[6..]))?
            .parse()
            .map_err(|_| "Invalid removed ranks")?;
        ranks.insert(name.clone(), n);
    }
    let nonempty = |k: &str| f.get(k).filter(|v| !v.is_empty()).map(String::as_str);
    let hp = nonempty("lost_hp")
        .map(|v| v.parse().map_err(|_| "Invalid lost HP"))
        .transpose()?;
    e.resolve_negative_level(
        d,
        number(f, "index")? as usize,
        number(f, "roll")?,
        &ranks,
        nonempty("lost_ability"),
        nonempty("lost_feat"),
        hp,
    )
}
