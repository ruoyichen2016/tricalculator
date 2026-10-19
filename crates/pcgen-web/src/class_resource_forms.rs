//! Ordinary HTML forms for single-class resources and tapestry lifecycle.
use super::forms::{begin, check, input, number, value};
use super::ui::escape;
use pcgen_core::magic::*;
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
fn select(out: &mut String, label: &str, name: &str, options: &[(&str, &str)]) {
    let _ = write!(out, "<label>{label}<select name=\"{name}\">");
    for (value, text) in options {
        let _ = write!(
            out,
            "<option value=\"{}\">{}</option>",
            escape(value),
            escape(text)
        );
    }
    out.push_str("</select></label>");
}
fn targets(
    out: &mut String,
    prefix: &str,
    d: &CharacterDraft,
    selected: &[String],
    choice: Option<&str>,
) {
    out.push_str("<fieldset><legend>Recipients</legend>");
    for (i, (id, _)) in d.class_play.actors.iter().enumerate() {
        if prefix == "members_" && id == "self" {
            continue;
        }
        if prefix != "members_"
            && id != "self"
            && !d
                .class_play
                .resources
                .tapestry
                .as_ref()
                .is_some_and(|t| t.members.contains(id))
        {
            continue;
        }
        let _=write!(out,"<label class=\"check\"><input type=\"checkbox\" name=\"{prefix}target_{i}\" value=\"{}\" {}>{}</label>",escape(id),if selected.contains(id){"checked"}else{""},escape(id));
    }
    out.push_str("</fieldset>");
    if prefix != "members_" {
        let _ = write!(
            out,
            "<label>Shared spell choice<select name=\"{prefix}choice\">"
        );
        for option in [
            "",
            "warm",
            "chill",
            "acid",
            "cold",
            "electricity",
            "fire",
            "sonic",
        ] {
            let _ = write!(
                out,
                "<option value=\"{option}\" {}>{}</option>",
                if choice.unwrap_or("") == option {
                    "selected"
                } else {
                    ""
                },
                if option.is_empty() {
                    "No choice required"
                } else {
                    option
                }
            );
        }
        out.push_str("</select></label>");
    }
}
fn parse_targets(f: &BTreeMap<String, String>, prefix: &str) -> Result<TapestryTargets, String> {
    let recipients = f
        .iter()
        .filter(|(k, _)| k.starts_with(&format!("{prefix}target_")))
        .map(|(_, v)| v.clone())
        .collect();
    let choice = f
        .get(&format!("{prefix}choice"))
        .filter(|s| !s.trim().is_empty())
        .cloned();
    Ok(TapestryTargets { recipients, choice })
}
fn roll(f: &BTreeMap<String, String>) -> Result<Option<u32>, String> {
    let s = value(f, "roll")?;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(number(f, "roll")?))
    }
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft, engine: &Engine, view: &str) {
    let show_all = view.is_empty() || view == "all";
    let show_scene = show_all || view == "scene";
    let show_casting = show_all || view == "casting";
    let show_defenses = show_all || view == "defenses";
    let rev = d.class_play.revision;
    if show_scene {
        super::scene_forms::render(out, id, d);
        super::feature_forms::render(out, id, d, engine);
    }
    if show_casting {
        out.push_str("<section id=\"resources\"><h2>Class resources</h2><p>Saved expenditure. Daily refresh does not restore weekly abilities or end ongoing effects.</p>");
        let proficiencies = engine.class_proficiencies(d);
        if !proficiencies.is_empty() {
            out.push_str("<details><summary>Class-granted proficiencies</summary><ul>");
            for g in proficiencies {
                let _ = write!(
                    out,
                    "<li>Weapon groups: {} · Armor: {:?} · Shields: {:?}</li>",
                    escape(&g.weapon_groups.join(", ")),
                    g.armor,
                    g.shields
                );
            }
            out.push_str("</ul></details>");
        }
        match engine.class_resources(d) {
            Ok(resources) => {
                out.push_str("<div class=\"table-scroll\"><table><thead><tr><th>Ability</th><th>Period</th><th>Allowance</th><th>Spent</th><th>Remaining</th></tr></thead><tbody>");
                if resources.is_empty() {
                    out.push_str("<tr><td colspan=\"5\" class=\"muted\">No limited-use class abilities at this level.</td></tr>");
                }
                for r in &resources {
                    let _ = write!(
                        out,
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        escape(&r.key),
                        escape(&r.period),
                        r.allowance,
                        r.spent,
                        r.remaining
                    );
                }
                out.push_str("</tbody></table></div>");
                for r in resources
                    .iter()
                    .filter(|r| r.key == "Dispel Evil" || r.key == "Holy Aura")
                {
                    begin(out, id, rev, "ability");
                    let _=write!(out,"<input type=\"hidden\" name=\"ability\" value=\"sacred_spell\"><input type=\"hidden\" name=\"spell\" value=\"{}\"><button>Use {}</button></form>",escape(&r.key),escape(&r.key));
                }
                if resources.iter().any(|r| r.key == "Grow Wings") {
                    begin(out, id, rev, "ability");
                    out.push_str("<input type=\"hidden\" name=\"ability\" value=\"wings\"><h3>Grow Wings</h3><p>Record a completed flight interval; growing and dismissing each take a standard action.</p>");
                    input(out, "Flight duration (seconds)", "seconds", "60", "number");
                    check(out, "Wearing armor or restrictive clothing", "restricted");
                    check(out, "Grappling when growing wings", "grappling");
                    out.push_str("<button>Record flight time</button></form>");
                }
            }
            Err(e) => {
                let _ = write!(out, "<p class=\"error\">{}</p>", escape(&e));
            }
        }
        match engine.class_feature_values(d) {
            Ok(features) => {
                for f in features {
                    let _ = write!(
                        out,
                        "<details><summary>{} · {}</summary><dl>",
                        escape(&f.class_id),
                        escape(&f.name)
                    );
                    for (key, n) in f.values {
                        let _ = write!(
                            out,
                            "<dt>{}</dt><dd>{n}</dd>",
                            escape(&key.replace('_', " "))
                        );
                    }
                    out.push_str("</dl>");
                    out.push_str("</details>");
                }
            }
            Err(e) => {
                let _ = write!(out, "<p class=\"error\">{}</p>", escape(&e));
            }
        }
        begin(out, id, rev, "refresh");
        select(
            out,
            "Refresh period",
            "period",
            &[
                ("day", "New daily ability period"),
                ("week", "New weekly ability period"),
                (
                    "spells",
                    "Spells recovered after required rest / preparation",
                ),
            ],
        );
        out.push_str("<button>Refresh resources</button></form>");
        begin(out, id, rev, "advance_class");
        input(
            out,
            "Seconds elapsed for tapestry and exorcism protection",
            "seconds",
            "60",
            "number",
        );
        out.push_str("<button>Advance durations</button></form></section>");
        out.push_str("<section id=\"cast\"><h2>Cast & record</h2><p>Spell slots, Sudden feats and Instant Metamagic are spent from saved resources. Workbench previews do not spend resources.</p>");
        let tracks: std::collections::BTreeSet<_> = d
            .advancement
            .iter()
            .flat_map(|f| f.lanes.iter().map(|l| l.class_id.clone()))
            .collect();
        for track in tracks {
            let Ok(catalog) = engine.spell_catalog(d, &track) else {
                continue;
            };
            begin(out, id, rev, "cast_record");
            let _=write!(out,"<h3>{}</h3><input type=\"hidden\" name=\"track\" value=\"{}\"><label>Spell<select name=\"spell\">",escape(&track),escape(&track));
            for s in catalog.iter().filter(|s| s.known && !s.prohibited) {
                let _ = write!(
                    out,
                    "<option value=\"{}\">{} · level {}</option>",
                    escape(&s.spell),
                    escape(&s.spell),
                    s.level
                );
            }
            out.push_str("</select></label><label>Target for Surprise Casting<select name=cast_target><option value=\"\">No specific target</option>");
            for target in d.class_play.actors.keys() {
                let _ = write!(out, "<option>{}</option>", escape(target));
            }
            out.push_str(
            "</select></label><details><summary>Metamagic payment</summary><div class=\"grid\">",
        );
            for rule in [
                "empower", "enlarge", "extend", "maximize", "silent", "still", "widen", "quicken",
            ] {
                let _ = write!(out, "<fieldset><legend>{rule}</legend>");
                check(out, "Apply feat", &format!("apply_{rule}"));
                check(out, "Use Sudden feat", &format!("sudden_{rule}"));
                out.push_str("</fieldset>");
            }
            select(
                out,
                "Instant Metamagic (if acquired)",
                "instant",
                &[
                    ("", "None"),
                    ("empower", "Empower"),
                    ("enlarge", "Enlarge"),
                    ("extend", "Extend"),
                    ("maximize", "Maximize"),
                    ("silent", "Silent"),
                    ("still", "Still"),
                    ("widen", "Widen"),
                    ("quicken", "Quicken"),
                ],
            );
            out.push_str("</div></details>");
            if engine.class_feature_values(d).is_ok_and(|fs| {
                fs.iter().any(|f| {
                    f.class_id == track && f.resolution == Some(FeatureResolution::QuickCast)
                })
            }) {
                check(out, "Use Quick Cast (swift action)", "quick_cast");
            }
            if engine.pack.entities.iter().any(|e| {
                e.name.eq_ignore_ascii_case(&track)
                    && e.metadata
                        .class_magic
                        .as_ref()
                        .is_some_and(|m| m.cloaked_casting)
            }) {
                check(
                    out,
                    "Affected foe is denied its Dexterity bonus (Cloaked Casting)",
                    "target_denied_dex",
                );
            }
            out.push_str("<button>Cast &amp; spend resources</button></form>");
        }
        out.push_str("<details><summary>Saved spell expenditure</summary><ul>");
        for (track, slots) in &d.class_play.resources.slots_spent {
            for (level, spent) in slots {
                let _ = write!(
                    out,
                    "<li>{}: level {} · {} slots spent</li>",
                    escape(track),
                    level,
                    spent
                );
            }
        }
        for (feat, spent) in &d.class_play.resources.sudden_spent {
            let _ = write!(
                out,
                "<li>Sudden {}: {} uses spent today</li>",
                escape(feat),
                spent
            );
        }
        let _ = write!(
            out,
            "<li>Instant Metamagic: {} uses spent today</li></ul></details></section>",
            d.class_play.resources.instant_spent
        );
        if d.level_in_class("Sacred Exorcist") > 0 {
            out.push_str("<section><h2>Sacred Exorcist</h2>");
            begin(out, id, rev, "ability");
            out.push_str("<input type=\"hidden\" name=\"ability\" value=\"exorcism\"><h3>Exorcism</h3><div class=\"grid\">");
            for (label, name, val, kind) in [
                ("Victim ID", "victim", "", "text"),
                ("Possessor ID", "possessor", "", "text"),
                ("Your d20", "roll", "10", "number"),
                ("Possessor Hit Dice", "hd", "1", "number"),
                ("Possessor Charisma score", "cha", "10", "number"),
            ] {
                input(out, label, name, val, kind);
            }
            out.push_str("</div><button>Resolve full-round exorcism</button></form><h3>Protected victims</h3><ul>");
            for (victim, possessors) in &d.class_play.resources.exorcised {
                for (possessor, time) in possessors {
                    let _ = write!(
                        out,
                        "<li>{} protected from {} for {} seconds</li>",
                        escape(victim),
                        escape(possessor),
                        time
                    );
                }
            }
            out.push_str("</ul>");
            begin(out, id, rev, "ability");
            out.push_str("<input type=\"hidden\" name=\"ability\" value=\"defense\"><h3>Situational modifiers</h3>");
            select(
                out,
                "Target kind",
                "target_kind",
                &[
                    ("other", "Other"),
                    ("undead", "Undead"),
                    ("evil_outsider", "Evil outsider"),
                ],
            );
            check(
                out,
                "Consecrated Presence suppressed by Desecrate",
                "desecrated",
            );
            out.push_str("<button>Resolve modifiers</button></form></section>");
        }
        if d.level_in_class("Incantatrix") >= 2 {
            out.push_str("<section><h2>Incantatrix metamagic</h2><p>Each legal attempt spends one use, even on failure. The two abilities have independent daily pools.</p>");
            begin(out, id, rev, "metamagic_use");
            select(
                out,
                "Ability",
                "mode",
                &[
                    ("cooperative", "Cooperative Metamagic"),
                    ("effect", "Metamagic Effect"),
                ],
            );
            input(out, "Spell name", "spell", "", "text");
            input(
                out,
                "Original caster's spell list",
                "spell_list",
                "Wizard",
                "text",
            );
            select(
                out,
                "Apply metamagic",
                "apply",
                &[
                    ("extend", "Extend"),
                    ("empower", "Empower"),
                    ("maximize", "Maximize"),
                    ("enlarge", "Enlarge"),
                    ("widen", "Widen"),
                    ("persistent", "Persistent"),
                ],
            );
            out.push_str("<fieldset><legend>Already applied metamagic</legend>");
            for rule in [
                "extend",
                "empower",
                "maximize",
                "enlarge",
                "widen",
                "persistent",
                "silent",
                "still",
                "quicken",
            ] {
                check(out, rule, &format!("existing_{rule}"));
            }
            out.push_str("</fieldset>");
            input(
                out,
                "Spellcraft d20 (blank = take 10)",
                "roll",
                "",
                "number",
            );
            for (label, key) in [
                ("Threatened", "threatened"),
                ("Distracted", "distracted"),
                ("Adjacent to caster or effect", "adjacent"),
                ("Within the existing effect", "within"),
                ("Willing allied caster", "willing"),
                ("Action readied for ally's casting", "readied"),
                ("Spell effect already ongoing", "ongoing"),
            ] {
                check(out, label, key);
            }
            out.push_str("<button>Resolve &amp; spend an attempt</button></form></section>");
        }
        super::item_forms::render(out, id, d, engine);
    }
    if show_defenses {
        super::defense_forms::render(out, id, d);
    }
    if show_casting && d.level_in_class("War Weaver") > 0 {
        out.push_str("<section><h2>Eldritch Tapestry</h2>");
        begin(out, id, rev, "ability");
        out.push_str("<input type=\"hidden\" name=\"ability\" value=\"tapestry\"><p>A successful new weave replaces the old tapestry and its dormant spells. Failure leaves the existing tapestry intact.</p>");
        input(
            out,
            "Arcane casting track",
            "track",
            d.class_play
                .resources
                .tapestry
                .as_ref()
                .map(|t| t.casting_track.as_str())
                .unwrap_or("Warmage"),
            "text",
        );
        targets(out, "members_", d, &[], None);
        input(
            out,
            "Craft (Weaving) d20 (blank = take 10)",
            "roll",
            "",
            "number",
        );
        for (label, key) in [("Threatened", "threatened"), ("Distracted", "distracted")] {
            check(out, label, key);
        }
        out.push_str("<button>Weave tapestry · 10 minutes</button></form>");
        if let Some(t) = &d.class_play.resources.tapestry {
            let _ = write!(
                out,
                "<p>{} allies connected; {} seconds remain; {} dormant spells.</p>",
                t.members.len(),
                t.remaining_seconds,
                t.stored.len()
            );
            if d.level_in_class("War Weaver") >= 2 {
                begin(out, id, rev, "store_weaving");
                input(out, "Spell to store", "spell", "", "text");
                out.push_str("<label><input type=\"checkbox\" name=\"spellguard_touch\">Spellguard defensive personal arcane spell</label>");
                targets(out, "", d, &t.members, None);
                out.push_str("<p>Recipient range is checked at release. Components and XP are paid now for the selected recipients.</p><button>Cast into quiescent weaving</button></form>");
            }
            if !t.stored.is_empty() {
                begin(out, id, rev, "release_weaving");
                for (i, s) in t.stored.iter().enumerate() {
                    let _=write!(out,"<fieldset><legend>{}: {}</legend><p>CL {}; DC {}; range {:?}, {} feet; duration {}</p>",i+1,escape(&s.spell),s.caster_level,s.save_dc,s.range,s.range_feet.map(|n|n.to_string()).unwrap_or_else(||"unavailable".into()),escape(s.duration.as_deref().unwrap_or("Unavailable")));
                    targets(
                        out,
                        &format!("s{i}_"),
                        d,
                        &s.targets.recipients,
                        s.targets.choice.as_deref(),
                    );
                    out.push_str("</fieldset>");
                }
                out.push_str("<button>Release ALL stored spells · move action</button></form>");
            }
        }
        out.push_str("</section>");
    }
}
pub(super) fn execute(
    engine: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        op if op.starts_with("combat_") => super::combat_forms::execute(engine, d, f),
        op if op.starts_with("tactical_") => super::tactical_forms::execute(engine, d, f),
        op if op.starts_with("necro_") => super::necromancy_forms::execute(engine, d, f),
        "resolve_feature"
        | "combat_facts"
        | "end_rage"
        | "ability_time"
        | "end_ability_encounter" => super::feature_forms::execute(engine, d, f),
        "cast_record" => {
            let mut request = CastRequest {
                quick_cast: f.contains_key("quick_cast"),
                target: f.get("cast_target").filter(|s| !s.is_empty()).cloned(),
                target_denied_dex: f.contains_key("target_denied_dex"),
                class_id: value(f, "track")?.into(),
                spell: value(f, "spell")?.into(),
                ..Default::default()
            };
            for name in [
                "empower", "enlarge", "extend", "maximize", "silent", "still", "widen", "quicken",
            ] {
                if f.contains_key(&format!("apply_{name}"))
                    || f.contains_key(&format!("sudden_{name}"))
                {
                    let rule = serde_json::from_value(serde_json::Value::String(name.into()))
                        .map_err(|_| "Unknown metamagic")?;
                    request.metamagic.push(rule);
                    if f.contains_key(&format!("sudden_{name}")) {
                        request.sudden.push(rule);
                    }
                }
            }
            let instant = value(f, "instant")?;
            if !instant.is_empty() {
                let rule = serde_json::from_value(serde_json::Value::String(instant.into()))
                    .map_err(|_| "Unknown metamagic")?;
                request.instant_metamagic = Some(rule);
                if !request.metamagic.contains(&rule) {
                    request.metamagic.push(rule);
                }
            }
            engine.cast_and_record(d, &request)
        }
        "scene_actor" => super::scene_forms::execute(engine, d, f),
        "refresh" => engine.refresh_class_resources(d, value(f, "period")?),
        "advance_class" => engine.advance_class_time(d, number(f, "seconds")?),
        "ability" => {
            let request = match value(f, "ability")? {
                "wings" => ClassActionRequest::GrowWings {
                    spent_seconds: 0,
                    use_seconds: number(f, "seconds")?,
                    armor_or_restrictive_clothing: f.contains_key("restricted"),
                    grappling: f.contains_key("grappling"),
                },
                "sacred_spell" => ClassActionRequest::SacredSpell {
                    spell: value(f, "spell")?.into(),
                    spent: 0,
                },
                "exorcism" => ClassActionRequest::Exorcism {
                    roll: number(f, "roll")?,
                    possessor_hit_dice: number(f, "hd")?,
                    possessor_charisma: value(f, "cha")?.parse().map_err(|_| "Invalid Charisma")?,
                },
                "defense" => ClassActionRequest::SacredDefense {
                    target_kind: value(f, "target_kind")?.into(),
                    desecrated: f.contains_key("desecrated"),
                },
                "tapestry" => ClassActionRequest::WeaveTapestry {
                    class_id: value(f, "track")?.into(),
                    roll: roll(f)?,
                    threatened: f.contains_key("threatened"),
                    distracted: f.contains_key("distracted"),
                    members: parse_targets(f, "members_")?.recipients,
                },
                _ => return Err("Unknown class ability".into()),
            };
            engine.use_class_ability(
                d,
                &request,
                f.get("victim").map(String::as_str).unwrap_or(""),
                f.get("possessor").map(String::as_str).unwrap_or(""),
            )
        }
        "register_trigger" | "use_trigger" | "register_preset" => {
            super::item_forms::execute(engine, d, f)
        }
        "save_defenses"
        | "resolve_defenses"
        | "save_health"
        | "advance_effect"
        | "dying_turn"
        | "damage_concentration" => super::defense_forms::execute(engine, d, f),
        "metamagic_use" => {
            let rule = |s: &str| {
                serde_json::from_value(serde_json::Value::String(s.into()))
                    .map_err(|_| "Unknown metamagic".to_string())
            };
            let existing = f
                .keys()
                .filter_map(|s| s.strip_prefix("existing_"))
                .map(rule)
                .collect::<Result<Vec<_>, _>>()?;
            engine.use_metamagic_feature(
                d,
                &MetamagicRequest {
                    mode: serde_json::from_value(serde_json::Value::String(
                        value(f, "mode")?.into(),
                    ))
                    .map_err(|_| "Unknown ability")?,
                    spell: value(f, "spell")?.into(),
                    spell_list: value(f, "spell_list")?.into(),
                    existing,
                    apply: rule(value(f, "apply")?)?,
                    inside_sanctum: false,
                    roll: roll(f)?,
                    threatened: f.contains_key("threatened"),
                    distracted: f.contains_key("distracted"),
                    spent: 0,
                    adjacent: f.contains_key("adjacent"),
                    within_effect: f.contains_key("within"),
                    willing_ally: f.contains_key("willing"),
                    readied_action: f.contains_key("readied"),
                    ongoing_effect: f.contains_key("ongoing"),
                },
            )
        }
        "store_weaving" => engine.store_weaving(
            d,
            &CastRequest {
                spellguard_touch: f.contains_key("spellguard_touch"),
                class_id: d
                    .class_play
                    .resources
                    .tapestry
                    .as_ref()
                    .ok_or("No tapestry")?
                    .casting_track
                    .clone(),
                spell: value(f, "spell")?.into(),
                tapestry_targets: Some(parse_targets(f, "")?),
                ..Default::default()
            },
        ),
        "release_weaving" => {
            let n = d
                .class_play
                .resources
                .tapestry
                .as_ref()
                .ok_or("No tapestry")?
                .stored
                .len();
            let targets = (0..n)
                .map(|i| parse_targets(f, &format!("s{i}_")))
                .collect::<Result<Vec<_>, _>>()?;
            engine.release_weaving(d, &targets)
        }
        _ => Err("Unknown operation".into()),
    }
}
