//! Saved weapon and DR facts in plain static forms; all arithmetic stays in the engine.
use super::{
    forms::{begin, checked, input, number, value},
    ui::escape,
};
use pcgen_core::{class_play::BodyImmunity, combat::*};
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
const TRAITS: &[(&str, &str)] = &[
    ("bludgeoning", "Bludgeoning"),
    ("piercing", "Piercing"),
    ("slashing", "Slashing"),
    ("magic", "Magic (+1 or higher)"),
    ("epic", "Epic (+6 or higher)"),
    ("silver", "Alchemical silver"),
    ("cold_iron", "Cold iron"),
    ("adamantine", "Adamantine"),
    ("good", "Good"),
    ("evil", "Evil"),
    ("lawful", "Lawful"),
    ("chaotic", "Chaotic"),
];
fn hidden(out: &mut String, name: &str, v: &str) {
    let _ = write!(
        out,
        "<input type=hidden name=\"{}\" value=\"{}\">",
        escape(name),
        escape(v)
    );
}
fn select_trait(out: &mut String, name: &str, selected: Option<WeaponTrait>) {
    let _ = write!(
        out,
        "<label>Bypass property<select name={name}><option value=''>None</option>"
    );
    for (v, label) in TRAITS {
        let chosen = selected.is_some_and(|s| {
            serde_json::to_value(s).ok() == Some(serde_json::Value::String((*v).into()))
        });
        let _ = write!(
            out,
            "<option value={v} {}>{label}</option>",
            if chosen { "selected" } else { "" }
        );
    }
    out.push_str("</select></label>");
}
fn parse<T: serde::de::DeserializeOwned>(v: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(v.into()))
        .map_err(|_| format!("Invalid rule property {v}"))
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft, e: &Engine) {
    out.push_str("<section id=weapon-and-effects><h2>Weapons, reduction &amp; ongoing spells</h2>");
    if d.class_levels().iter().any(|c| c.class_id == "Duskblade") {
        out.push_str("<details><summary>Channeling weapon</summary>");
        begin(out, id, d.class_play.revision, "combat_weapon");
        let setup = d.class_play.combat_state.weapon.as_ref();
        out.push_str("<label>RAW melee weapon<select name=equipment>");
        for (name, w) in &e.pack.equipment {
            if w.damage.is_some() && w.type_tags.iter().any(|t| t.eq_ignore_ascii_case("Melee")) {
                let _ = write!(
                    out,
                    "<option value=\"{}\" {}>{} ({})</option>",
                    escape(name),
                    if setup.is_some_and(|s| s.equipment == *name) {
                        "selected"
                    } else {
                        ""
                    },
                    escape(name),
                    escape(w.damage.as_deref().unwrap_or(""))
                );
            }
        }
        out.push_str("</select></label>");
        input(
            out,
            "Actual magical enhancement (not equivalent item price bonus)",
            "enhancement",
            &setup
                .map(|s| s.enhancement.to_string())
                .unwrap_or_else(|| "0".into()),
            "number",
        );
        out.push_str("<label>Material<select name=material>");
        for (v, label) in [
            ("ordinary", "Ordinary"),
            ("silver", "Alchemical silver"),
            ("cold_iron", "Cold iron"),
            ("adamantine", "Adamantine"),
        ] {
            let chosen = setup.is_some_and(|s| {
                serde_json::to_value(s.material).ok() == Some(serde_json::Value::String(v.into()))
            });
            let _ = write!(
                out,
                "<option value={v} {}>{label}</option>",
                if chosen { "selected" } else { "" }
            );
        }
        out.push_str("</select></label>");
        checked(
            out,
            "Masterwork",
            "masterwork",
            setup.is_some_and(|s| s.masterwork),
        );
        checked(
            out,
            "Wielded in two hands",
            "two_handed",
            setup.is_some_and(|s| s.two_handed),
        );
        for (v, label) in &TRAITS[8..] {
            checked(
                out,
                label,
                v,
                setup.is_some_and(|s| {
                    s.alignments.iter().any(|t| {
                        serde_json::to_value(t).ok() == Some(serde_json::Value::String((*v).into()))
                    })
                }),
            );
        }
        out.push_str("<button>Save channeling weapon</button></form></details>");
    }
    for target in d.class_play.actors.keys() {
        let _ = write!(
            out,
            "<details><summary>{}: physical DR and immunities</summary>",
            escape(target)
        );
        begin(out, id, d.class_play.revision, "combat_dr");
        hidden(out, "target", target);
        out.push_str("<p>Each row is one DR source. Requirements in a row may be AND or OR. Sources do not add together. Leave amounts at 0 to record no DR.</p>");
        let existing = d.class_play.combat_state.reductions.get(target);
        for i in 0..existing.map_or(3, |r| r.len().max(3)) {
            let r = existing.and_then(|r| r.get(i));
            let _ = write!(out, "<fieldset><legend>DR source {}</legend>", i + 1);
            input(
                out,
                "Reduction amount",
                &format!("amount_{i}"),
                &r.map(|r| r.amount.to_string())
                    .unwrap_or_else(|| "0".into()),
                "number",
            );
            let all = r.is_some_and(|r| r.bypass.len() == 1 && r.bypass[0].len() > 1);
            let _=write!(out,"<label>Bypass logic<select name=logic_{i}><option value=any {}>Any selected property (OR)</option><option value=all {}>All selected properties (AND)</option></select></label>",if !all{"selected"}else{""},if all{"selected"}else{""});
            let atoms: Vec<_> = r
                .map(|r| r.bypass.iter().flatten().copied().collect())
                .unwrap_or_default();
            for j in 0..3 {
                select_trait(out, &format!("trait_{i}_{j}"), atoms.get(j).copied());
            }
            out.push_str("</fieldset>");
        }
        for (v, label) in [
            ("poison", "Poison immunity"),
            ("paralysis", "Paralysis immunity"),
            ("fatigue", "Fatigue immunity"),
            ("mind_affecting", "Mind-affecting immunity"),
            ("critical_hits", "Critical-hit immunity"),
        ] {
            checked(
                out,
                label,
                v,
                d.class_play
                    .combat_state
                    .immunities
                    .get(target)
                    .is_some_and(|xs| {
                        xs.iter().any(|x| {
                            serde_json::to_value(x).ok()
                                == Some(serde_json::Value::String(v.into()))
                        })
                    }),
            );
        }
        out.push_str("<button>Save DR and immunity facts</button></form></details>");
    }
    for (key, exposure) in d
        .class_play
        .combat_state
        .exposures
        .iter()
        .filter(|(_, e)| e.saved.is_none())
    {
        let _=write!(out,"<p class=error>{}: Ghoul Touch stench save pending; affected combat values are unavailable.</p>",escape(&exposure.target));
        begin(out, id, d.class_play.revision, "combat_stench");
        hidden(out, "exposure", key);
        input(out, "Stench Fortitude d20", "roll", "", "number");
        out.push_str("<button>Resolve stench save</button></form>");
    }
    for e in d.class_play.combat_state.effects.values() {
        let _ = write!(
            out,
            "<p>{}: {} · {}; expires at {} seconds (CL {}).</p>",
            escape(&e.target),
            escape(&e.name),
            escape(&format!("{:?}", e.condition)),
            e.expires,
            e.caster_level
        );
    }
    out.push_str("</section>");
}
pub(super) fn execute(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        "combat_weapon" => e.save_weapon_setup(
            d,
            WeaponSetup {
                equipment: value(f, "equipment")?.into(),
                enhancement: number(f, "enhancement")?,
                material: parse(value(f, "material")?)?,
                masterwork: f.contains_key("masterwork"),
                two_handed: f.contains_key("two_handed"),
                alignments: TRAITS[8..]
                    .iter()
                    .filter(|(k, _)| f.contains_key(*k))
                    .map(|(k, _)| parse(k))
                    .collect::<Result<_, _>>()?,
            },
        ),
        "combat_dr" => {
            let mut rules = vec![];
            for (k, v) in f.iter().filter(|(k, _)| k.starts_with("amount_")) {
                let n = v.parse::<u32>().map_err(|_| "Invalid DR amount")?;
                if n == 0 {
                    continue;
                }
                let i = &k[7..];
                let mut atoms = vec![];
                for j in 0..3 {
                    if let Some(v) = f.get(&format!("trait_{i}_{j}")).filter(|v| !v.is_empty()) {
                        atoms.push(parse::<WeaponTrait>(v)?);
                    }
                }
                let bypass = if atoms.is_empty() {
                    vec![]
                } else if value(f, &format!("logic_{i}"))? == "all" {
                    vec![atoms]
                } else {
                    atoms.into_iter().map(|x| vec![x]).collect()
                };
                rules.push(PhysicalReduction { amount: n, bypass });
            }
            let mut result = e.save_physical_reduction(d, value(f, "target")?, rules)?;
            let immunities = [
                "poison",
                "paralysis",
                "fatigue",
                "mind_affecting",
                "critical_hits",
            ]
            .iter()
            .filter(|k| f.contains_key(**k))
            .map(|k| parse::<BodyImmunity>(k))
            .collect::<Result<_, _>>()?;
            result
                .state
                .combat_state
                .immunities
                .insert(value(f, "target")?.into(), immunities);
            Ok(result)
        }
        "combat_stench" => e.resolve_stench_exposure(d, value(f, "exposure")?, number(f, "roll")?),
        _ => Err("Unknown combat action".into()),
    }
}
