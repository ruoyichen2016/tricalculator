use super::{
    forms::{begin, check, input, number, value},
    ui::escape,
};
use pcgen_core::{class_play::*, magic::SpellRule};
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft, engine: &Engine) {
    let track = super::character_context::first_casting_track(engine, d);
    out.push_str("<section id=\"items\"><h2>Spell-trigger inventory</h2><p>One item, one charge pool. Each power has its own base cost. Select a power and any acquired Incantatrix metamagic; normal activation needs no crafting feat.</p>");
    if let Ok(presets) = pcgen_engine::item_casting::item_templates() {
        out.push_str("<details><summary>Add a standard wand or staff</summary>");
        begin(out, id, d.class_play.revision, "register_preset");
        input(out, "Item ID", "item", "", "text");
        input(out, "User casting track", "item_track", &track, "text");
        out.push_str("<label>SRD item<select name=\"preset\">");
        for key in presets.keys() {
            let _ = write!(out, "<option>{}</option>", escape(key));
        }
        out.push_str(
            "</select></label><button>Register SRD item · 50 charges</button></form></details>",
        );
    }
    out.push_str("<details><summary>Custom wand or staff</summary>");
    begin(out, id, d.class_play.revision, "register_trigger");
    input(out, "Item ID", "item", "", "text");
    out.push_str("<label>Item type<select name=\"kind\"><option value=\"staff\">Staff</option><option value=\"wand\">Wand</option></select></label>");
    input(out, "Item caster level", "item_cl", "8", "number");
    input(out, "User casting track", "item_track", &track, "text");
    input(out, "Shared remaining charges", "charges", "50", "number");
    for i in 0..8 {
        let _ = write!(
            out,
            "<fieldset><legend>Power {} · blank spell ignores row</legend>",
            i + 1
        );
        input(out, "Spell", &format!("spell_{i}"), "", "text");
        input(out, "Creation list", &format!("list_{i}"), "Wizard", "text");
        input(out, "Base charges", &format!("cost_{i}"), "1", "number");
        input(
            out,
            "Stored Heighten level (optional)",
            &format!("heighten_{i}"),
            "",
            "number",
        );
        out.push_str("</fieldset>");
    }
    out.push_str("<button>Register item</button></form></details>");
    for (key, item) in &d.class_play.resources.trigger_items {
        begin(out, id, d.class_play.revision, "use_trigger");
        let _=write!(out,"<div class=\"effect-card\"><h3>{}</h3><p>{:?} · CL {} · <strong>{} shared charges</strong></p><input type=\"hidden\" name=\"item\" value=\"{}\"><label>Power<select name=\"power\">",escape(key),item.kind,item.caster_level,item.charges,escape(key));
        for (power, p) in &item.powers {
            let _ = write!(
                out,
                "<option value=\"{}\">{} · {} base charge(s)</option>",
                escape(power),
                escape(&p.spell),
                p.base_charges
            );
        }
        out.push_str("</select></label><details><summary>Incantatrix metamagic</summary><fieldset><legend>Applied together; leave empty for normal activation</legend>");
        for rule in [
            "empower",
            "maximize",
            "extend",
            "enlarge",
            "widen",
            "quicken",
            "silent",
            "still",
            "persistent",
        ] {
            check(out, rule, &format!("meta_{rule}"));
        }
        out.push_str("</fieldset></details><button>Activate selected power</button></div></form>");
    }
    out.push_str("</section>");
}
pub(super) fn execute(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        "register_preset" => {
            let mut item = pcgen_engine::item_casting::item_templates()?
                .remove(value(f, "preset")?)
                .ok_or("Unknown item template")?;
            item.casting_track = value(f, "item_track")?.into();
            e.register_trigger_item(d, value(f, "item")?, item)
        }
        "register_trigger" => {
            let mut powers = BTreeMap::new();
            for i in 0..8 {
                let spell = value(f, &format!("spell_{i}"))?;
                if spell.trim().is_empty() {
                    continue;
                }
                let h = value(f, &format!("heighten_{i}"))?;
                powers.insert(
                    format!("power-{}", i + 1),
                    TriggerPower {
                        spell: spell.into(),
                        spell_list: value(f, &format!("list_{i}"))?.into(),
                        base_charges: number(f, &format!("cost_{i}"))?,
                        heightened_level: if h.is_empty() {
                            None
                        } else {
                            Some(h.parse().map_err(|_| "Invalid stored Heighten level")?)
                        },
                    },
                );
            }
            e.register_trigger_item(
                d,
                value(f, "item")?,
                TriggerItem {
                    powers,
                    kind: serde_json::from_value(serde_json::Value::String(
                        value(f, "kind")?.into(),
                    ))
                    .map_err(|_| "Unknown item kind")?,
                    caster_level: number(f, "item_cl")?,
                    casting_track: value(f, "item_track")?.into(),
                    charges: number(f, "charges")?,
                },
            )
        }
        "use_trigger" => {
            let meta = f
                .keys()
                .filter_map(|k| k.strip_prefix("meta_"))
                .map(|s| {
                    serde_json::from_value::<SpellRule>(serde_json::Value::String(s.into()))
                        .map_err(|_| "Invalid metamagic".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            e.use_trigger_item(d, value(f, "item")?, value(f, "power")?, meta)
        }
        _ => Err("Unknown item action".into()),
    }
}
