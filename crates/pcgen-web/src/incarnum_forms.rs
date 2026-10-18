//! English ordinary forms for the bounded Warforged case; all calculations live in the engine.
use super::ui::escape;
use super::*;
use axum::Form;
use pcgen_engine::{incarnum::*, incarnum_case::IncarnumCase};
use std::{collections::BTreeMap, fmt::Write};
const CASE: &str = include_str!("../../../tests/fixtures/forum-builds/warforged-umbral.json");
const INVESTMENTS: [(&str, &str); 11] = [
    ("kiss", "Kiss of the Shadows"),
    ("shadow", "Embrace of Shadow"),
    ("sight", "Sight of the Eyeless"),
    ("chill", "Soulchilling Strike"),
    ("strike", "Incandescent Strike"),
    ("healing", "Fast Healing"),
    ("step", "Step of the Bodiless"),
    ("blink", "Blink Shirt"),
    ("worm", "Wormtail Belt"),
    ("avatar", "Totem Avatar"),
    ("girallon", "Girallon Arms"),
];
fn select(out: &mut String, name: &str, label: &str, value: &str, options: &[(&str, &str)]) {
    let _ = write!(
        out,
        "<label>{}<select name=\"{}\">",
        escape(label),
        escape(name)
    );
    for (key, text) in options {
        let _ = write!(
            out,
            "<option value=\"{}\" {}>{}</option>",
            escape(key),
            if *key == value { "selected" } else { "" },
            escape(text)
        );
    }
    out.push_str("</select></label>");
}
fn render(state: &AppState, f: &BTreeMap<String, String>) -> String {
    let mut out = super::ui::start(
        super::ui::Page::Cases,
        "",
        "Incarnum stress replay",
        "Complete class progressions first; original build errors and a separate repair probe.",
        &[],
    );
    out.push_str("<p><a href=\"/classes\">Full class tables</a> · <a href=\"/martial\">Shadow Pounce case</a></p><p>Read-only case study. The original post omits abilities, alignment and skill history. This probe assumes NG, CON 14, medium size, speed 30 and two light weapons. It removes the duplicate Bonus Essentia and replaces Dancing Mongoose with Wolf Fang Strike. These changes do not certify the original character.</p>");
    let result = (|| -> Result<(), String> {
        let mut c: IncarnumCase = serde_json::from_str(CASE).map_err(|e| e.to_string())?;
        let val = |key: &str, default: &str| f.get(key).cloned().unwrap_or(default.into());
        out.push_str("<section><h2>Allocation and target</h2><form method=\"post\" action=\"/incarnum\"><input type=\"hidden\" name=\"submitted\" value=\"1\">");
        select(
            &mut out,
            "bind",
            "Totem bind",
            &val("bind", "Blink Shirt"),
            &[
                ("Blink Shirt", "Blink Shirt: move-action teleport"),
                ("Totem Avatar", "Totem Avatar: natural damage"),
                ("Wormtail Belt", "Wormtail Belt: exclusive stinger"),
                (
                    "Girallon Arms",
                    "Girallon Arms: two free claws with two held weapons",
                ),
            ],
        );
        for (key, name) in INVESTMENTS {
            let default = c
                .loadout
                .investments
                .get(name)
                .copied()
                .unwrap_or(0)
                .to_string();
            super::forms::input(&mut out, name, key, &val(key, &default), "number");
        }
        for (key, label, default) in [
            ("distance", "Target distance at attack (feet)", "20"),
            ("spent", "Fast Healing rounds already spent today", "0"),
        ] {
            super::forms::input(&mut out, label, key, &val(key, default), "number");
        }
        for (key, label, default) in [
            ("denied", "Target denied Dexterity", true),
            (
                "flank",
                "Target flanked under Kiss adjacent-square rule",
                false,
            ),
            ("immune", "Target immune to sneak attack", false),
            ("concealment", "Target has concealment", false),
            ("gloves", "Gloves of the Balanced Hand equipped", true),
            (
                "slam",
                "Allow body slam with both weapon hands occupied (explicit assumption)",
                true,
            ),
            (
                "chill_used",
                "Soulchilling already triggered this round",
                false,
            ),
        ] {
            super::forms::checked(
                &mut out,
                label,
                key,
                if f.is_empty() {
                    default
                } else {
                    f.contains_key(key)
                },
            );
        }
        select(
            &mut out,
            "action",
            "Action sequence",
            &val("action", "charge"),
            &[
                ("charge", "Pouncing Charge"),
                ("full", "Ordinary full attack"),
                ("blink_attack", "Blink Shirt then full attack"),
                ("leap_attack", "Sudden Leap then full attack"),
                (
                    "mongoose",
                    "Dancing Mongoose then full attack (not in repaired training)",
                ),
                ("reallocate_leap", "Reallocate then Sudden Leap"),
                ("heal", "Fast Healing tick at turn start (already active)"),
                ("wrt", "White Raven Tactics on another ally at 10 feet"),
            ],
        );
        out.push_str("<button>Recalculate case</button></form></section>");
        if !f.is_empty() {
            c.loadout.investments.clear();
            for (key, name) in INVESTMENTS {
                let n = val(key, "0")
                    .parse::<u32>()
                    .map_err(|_| format!("{name}: enter a nonnegative integer"))?;
                if n > 0 {
                    c.loadout.investments.insert(name.into(), n);
                }
            }
            let bind = val("bind", "Blink Shirt");
            if !CASE_SOULMELDS.contains(&bind.as_str()) {
                return Err("Unknown totem bind".into());
            }
            c.loadout.binds = BTreeMap::from([(bind.clone(), "Totem".into())]);
            if bind == "Girallon Arms" {
                c.loadout.shaped[2] = bind;
            }
            c.turn.target_feet = val("distance", "20")
                .parse()
                .map_err(|_| "Invalid target distance")?;
            c.turn.healing_rounds_spent = val("spent", "0")
                .parse()
                .map_err(|_| "Invalid spent rounds")?;
            c.turn.denied_dex = f.contains_key("denied");
            c.turn.flanking = f.contains_key("flank");
            c.turn.precision_immune = f.contains_key("immune");
            c.turn.target_concealment = f.contains_key("concealment");
            c.turn.gloves = f.contains_key("gloves");
            c.turn.slam_available = f.contains_key("slam");
            c.turn.soulchilling_used_this_round = f.contains_key("chill_used");
            c.turn.actions = match val("action", "charge").as_str() {
                "charge" => vec![IncarnumAction::PouncingCharge {
                    feet: 20,
                    path_clear: true,
                }],
                "full" => vec![IncarnumAction::FullAttack],
                "blink_attack" => vec![
                    IncarnumAction::Blink { feet: 10 },
                    IncarnumAction::FullAttack,
                ],
                "leap_attack" => vec![
                    IncarnumAction::SuddenLeap {
                        feet: 5,
                        jump_check: 10,
                        running_start: false,
                    },
                    IncarnumAction::FullAttack,
                ],
                "mongoose" => vec![IncarnumAction::DancingMongoose, IncarnumAction::FullAttack],
                "reallocate_leap" => vec![
                    IncarnumAction::Reallocate {
                        investments: c.loadout.investments.clone(),
                    },
                    IncarnumAction::SuddenLeap {
                        feet: 5,
                        jump_check: 10,
                        running_start: false,
                    },
                ],
                "heal" => vec![IncarnumAction::FastHealingTick],
                "wrt" => vec![IncarnumAction::WhiteRavenTactics {
                    target_is_self: false,
                    feet: 10,
                    actor_initiative: 15,
                    ally_initiative: 20,
                }],
                _ => return Err("Unknown action sequence".into()),
            };
        }
        let engine = cached_engine(
            state,
            &state
                .config
                .cohort_root
                .join("data/rules/rainbow-warweaver.rules.json"),
        )?;
        let r = engine.audit_incarnum_case(&c);
        out.push_str("<section><h2>Original build audit</h2>");
        for e in &r.build_errors {
            super::ui::notice(&mut out, e, true);
        }
        out.push_str("<p>Standard multiclass BAB is 15, not 16. Bonus Essentia contributes once: the corrected pool is 14. Warblade IL 11 does not grant a new stance at Warblade 2.</p><table><thead><tr><th>Level</th><th>BAB</th><th>Essentia (one Bonus Essentia)</th><th>Sneak dice</th><th>IL</th><th>Ordinary attacks</th></tr></thead><tbody>");
        for row in &r.milestones {
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}d6</td><td>{}</td><td>{}</td></tr>",
                row.level,
                row.bab,
                row.essentia,
                row.sneak_dice,
                row.warblade_il.map(|n| n.to_string()).unwrap_or("—".into()),
                row.ordinary_attacks
            );
        }
        out.push_str("</tbody></table><p>Milestones assume the stated gloves from level 12 and an available body slam; no soulmeld attacks or boosts included.</p></section><section><h2>Repair probe</h2>");
        for e in &r.repaired_training.errors {
            super::ui::notice(&mut out, e, true);
        }
        let s = &r.allocation_probe;
        let _=write!(out,"<p>Pool {}; allocated {}; capacity {} per ordinary target. Meldshaper level {}; {} soulmelds; {} chakra bind.</p>",s.pool,s.allocated,s.capacity,s.meldshaper_level,s.soulmeld_limit,s.bind_limit);
        for e in &s.errors {
            super::ui::notice(&mut out, e, true);
        }
        if s.errors.is_empty() {
            let _=write!(out,"<p>Own-turn reach: {} feet; threatened reach outside your turn: {} feet. Miss chance: {}%; blindsight: {} feet. Fast Healing {} for at most {} rounds/day.</p>",s.effects.own_turn_reach_feet,s.effects.threatened_reach_feet,s.effects.miss_chance,s.effects.blindsight_feet,s.effects.fast_healing,s.effects.healing_rounds_per_day);
        }
        let p = &r.turn_probe;
        for e in &p.errors {
            super::ui::notice(&mut out, e, true);
        }
        let _=write!(out,"<p>{} attack opportunities; at most {} Soulchilling trigger this round. Healing this turn: {}; healing rounds remaining: {}.</p>",p.attacks.len(),p.soulchilling_triggers_max,p.healing_this_turn,p.healing_rounds_remaining);
        out.push_str("<p>Bonuses below exclude ability, equipment and situational bonuses except charge and Girallon claw enhancement. Damage columns are additions per qualifying hit; no attack or damage rolls are assumed.</p><table><thead><tr><th>Attack</th><th>Base bonus</th><th>Sneak</th><th>Craven</th><th>Incarnum damage</th></tr></thead><tbody>");
        for a in &p.attacks {
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{:+}</td><td>{}d6</td><td>{}</td><td>{}</td></tr>",
                escape(&a.weapon),
                a.base_bonus,
                a.sneak_dice,
                a.craven_damage,
                a.incarnum_damage
            );
        }
        out.push_str(
            "</tbody></table><details><summary>Action ledger and assumptions</summary><ul>",
        );
        for n in p.log.iter().chain(r.assumptions.iter()) {
            let _ = write!(out, "<li>{}</li>", escape(n));
        }
        out.push_str("</ul></details></section>");
        Ok(())
    })();
    if let Err(e) = result {
        super::ui::notice(&mut out, &e, true);
    }
    super::ui::finish(
        &mut out,
        "ECS supernatural healing is halved for Warforged. Read-only probe; saved resources and HP are not changed",
    );
    out
}
pub(super) async fn page(State(state): State<AppState>) -> Html<String> {
    Html(render(&state, &BTreeMap::new()))
}
pub(super) async fn replay(
    State(state): State<AppState>,
    Form(f): Form<BTreeMap<String, String>>,
) -> Html<String> {
    Html(render(&state, &f))
}
