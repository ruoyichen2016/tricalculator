//! Scene facts entered through static forms; legality is computed by the engine.
use super::forms::{begin, input, number, value};
use super::ui::escape;
use pcgen_core::class_play::*;
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
fn select(out: &mut String, label: &str, key: &str, current: &str, options: &[&str]) {
    let _ = write!(out, "<label>{label}<select name=\"{key}\">");
    for v in options {
        let _ = write!(
            out,
            "<option value=\"{v}\" {}>{}</option>",
            if *v == current { "selected" } else { "" },
            escape(&v.replace('_', " "))
        );
    }
    out.push_str("</select></label>");
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft) {
    out.push_str("<section id=\"scene\"><h2>Scene & recipients</h2><p>Use actor ID <strong>self</strong> for the caster. Positions are five-foot squares on a flat grid; every second diagonal costs ten feet. Footprint is the side length in squares. Path describes the environment from the caster, not a legality verdict. Update it after movement. Saves and spell resistance resolve separately.</p>");
    if let Some(caster) = d.class_play.actors.get("self") {
        out.push_str("<div class=\"table-scroll\"><table><thead><tr><th>Actor</th><th>Type</th><th>Distance</th><th>Path</th><th>Tapestry</th></tr></thead><tbody>");
        for (key, a) in &d.class_play.actors {
            let connected = key == "self"
                || d.class_play
                    .resources
                    .tapestry
                    .as_ref()
                    .is_some_and(|t| t.members.contains(key));
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{} ft</td><td>{:?}</td><td>{}</td></tr>",
                escape(key),
                escape(&a.creature_type),
                pcgen_engine::targeting::distance(caster, a),
                a.path,
                if connected {
                    "Connected"
                } else {
                    "Not connected"
                }
            );
        }
        out.push_str("</tbody></table></div>");
    }
    let blank = SceneActor {
        body: TargetBody::Living,
        creature_type: "humanoid".into(),
        dead: false,
        allied: true,
        willing: true,
        unconscious: false,
        x: 0,
        y: 0,
        space: 1,
        touch_reach_feet: 5,
        path: TargetPath::Clear,
    };
    for (key, a) in d
        .class_play
        .actors
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .chain(std::iter::once((
            if d.class_play.actors.contains_key("self") {
                ""
            } else {
                "self"
            },
            &blank,
        )))
    {
        let _ = write!(
            out,
            "<details><summary>{}</summary>",
            if key.is_empty() {
                "Add actor".into()
            } else {
                escape(key)
            }
        );
        begin(out, id, d.class_play.revision, "scene_actor");
        input(out, "Actor ID", "actor", key, "text");
        let enum_text = |v: serde_json::Value| v.as_str().unwrap_or("").to_owned();
        select(
            out,
            "Body",
            "body",
            &enum_text(serde_json::to_value(a.body).unwrap()),
            &[
                "living",
                "living_construct",
                "construct",
                "undead",
                "object",
            ],
        );
        select(
            out,
            "Creature type",
            "creature_type",
            &a.creature_type,
            &[
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
            ],
        );
        for (label, name, val) in [
            ("X (squares)", "x", a.x.to_string()),
            ("Y (squares)", "y", a.y.to_string()),
            ("Footprint (squares)", "space", a.space.to_string()),
            (
                "Touch reach (feet)",
                "reach",
                a.touch_reach_feet.to_string(),
            ),
        ] {
            input(out, label, name, &val, "number");
        }
        select(
            out,
            "Path from caster",
            "path",
            &enum_text(serde_json::to_value(a.path).unwrap()),
            &["clear", "obscured", "solid", "transparent_solid"],
        );
        for (label, name, checked) in [
            ("Allied", "allied", a.allied),
            ("Willing", "willing", a.willing),
            ("Unconscious", "unconscious", a.unconscious),
            ("Dead", "dead", a.dead),
        ] {
            let _=write!(out,"<label class=\"check\"><input type=\"checkbox\" name=\"{name}\" {}>{label}</label>",if checked{"checked"}else{""});
        }
        out.push_str("<button>Save scene actor</button></form></details>");
    }
    out.push_str("</section>");
}
pub(super) fn execute(
    engine: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    engine.save_scene_actor(
        d,
        value(f, "actor")?,
        SceneActor {
            body: serde_json::from_value(serde_json::Value::String(value(f, "body")?.into()))
                .map_err(|_| "Unknown body")?,
            creature_type: value(f, "creature_type")?.into(),
            path: serde_json::from_value(serde_json::Value::String(value(f, "path")?.into()))
                .map_err(|_| "Unknown path")?,
            x: value(f, "x")?.parse().map_err(|_| "Invalid X coordinate")?,
            y: value(f, "y")?.parse().map_err(|_| "Invalid Y coordinate")?,
            space: number(f, "space")?,
            touch_reach_feet: number(f, "reach")?,
            dead: f.contains_key("dead"),
            allied: f.contains_key("allied"),
            willing: f.contains_key("willing"),
            unconscious: f.contains_key("unconscious"),
        },
    )
}
