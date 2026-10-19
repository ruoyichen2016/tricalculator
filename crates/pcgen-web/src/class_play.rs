//! Static class action forms; committed transitions use a revision check and rolling save.
use super::forms::{begin, check, input, number, value};
use super::ui::{escape, url};
use super::*;
use axum::Form;
use pcgen_core::class_play::*;
use pcgen_engine::class_play::PlayResult;
use std::{collections::BTreeMap, fmt::Write};
#[derive(Default, serde::Deserialize)]
pub(super) struct EncounterQuery {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub view: String,
}

fn render(state: &AppState, id: &str, view: &str, message: &str) -> String {
    let mut out = super::ui::start(
        super::ui::Page::Encounter,
        id,
        "The encounter desk",
        "Track the scene, spend resources, and resolve what happens next.",
        &[
            ("overview", "Overview"),
            ("scene", "Scene"),
            ("class-abilities", "Class abilities"),
            ("necromancy", "Afflictions & undead"),
            ("tactical-actions", "Tactical actions"),
            ("weapon-and-effects", "Weapon & DR"),
            ("resources", "Resources"),
            ("cast", "Cast a spell"),
            ("items", "Items"),
            ("defenses", "Damage & defenses"),
            ("air", "Air domain"),
            ("control", "Spell control"),
        ],
    );
    super::ui::notice(&mut out, message, message.starts_with("Not recorded:"));
    let active_view = if view.is_empty() { "all" } else { view };
    let hidden_view = if active_view == "all" {
        ""
    } else {
        active_view
    };
    super::ui::character_bar(
        &mut out,
        state,
        "/class-play",
        id,
        &[("view", hidden_view)],
        &[],
    );
    let loaded = super::character_context::load(state, id);
    let (env, engine) = match loaded {
        Ok(v) => v,
        Err(error) => {
            if !id.is_empty() {
                let _ = write!(out, "<section class=\"error\">{}</section>", escape(&error));
            }
            super::ui::finish(&mut out, "Encounter actions update the current character");
            return out;
        }
    };
    let d = &env.character;
    let rev = d.class_play.revision;
    out.push_str("<nav class=\"sub-nav\" aria-label=\"Encounter desk view\">");
    for (v, label) in [
        ("all", "All sections"),
        ("scene", "Scene & tactical"),
        ("casting", "Resources & casting"),
        ("defenses", "Defenses, Air & control"),
    ] {
        let href = if v == "all" {
            format!("/class-play?id={}", url(id))
        } else {
            format!("/class-play?id={}&view={v}", url(id))
        };
        let _ = write!(
            out,
            "<a href=\"{href}\" {}>{label}</a>",
            if active_view == v {
                "aria-current=\"page\""
            } else {
                ""
            }
        );
    }
    out.push_str("</nav>");
    let classes_summary = d
        .class_levels()
        .into_iter()
        .map(|c| format!("{} {}", c.class_id, c.level))
        .collect::<Vec<_>>()
        .join(" / ");
    let _ = write!(
        out,
        "<section id=\"overview\" class=\"overview\"><div class=\"section-heading\"><div><p class=\"eyebrow\">CURRENT CHARACTER · {}</p><h2>{}</h2></div><div class=\"context-links\"><a class=\"text-link\" href=\"/build?id={}\">Build studio →</a><a class=\"text-link\" href=\"/spellcasting?id={}\">Spellcasting →</a><a class=\"text-link\" href=\"/editor?id={}\">Edit character →</a></div></div><div class=\"metrics\">",
        escape(if classes_summary.is_empty() {
            "No class levels"
        } else {
            &classes_summary
        }),
        escape(&d.name),
        url(id),
        url(id),
        url(id)
    );
    super::ui::metric(
        &mut out,
        "Character level",
        &d.total_level().to_string(),
        &format!("/?id={}", url(id)),
    );
    super::ui::metric(
        &mut out,
        "Scene actors",
        &d.class_play.actors.len().to_string(),
        "#scene",
    );
    super::ui::metric(
        &mut out,
        "HP records",
        &d.class_play.health.len().to_string(),
        "#defenses",
    );
    let pending = d
        .class_play
        .concentration_checks
        .values()
        .filter(|c| {
            c.success.is_none()
                && d.class_play.spells.get(&c.spell).is_some_and(|s| {
                    s.ended.is_none() && s.controller == SpellController::Incantatrix
                })
        })
        .count();
    super::ui::metric(
        &mut out,
        "Pending checks",
        &pending.to_string(),
        "#defenses",
    );
    out.push_str("</div><p class=\"muted\">Set up the scene, record an action, then resolve its targets. Changes are saved with each submission.</p></section>");
    super::class_resource_forms::render(&mut out, id, d, &engine, active_view);
    if active_view == "all" || active_view == "defenses" {
        out.push_str("<section id=\"air\"><h2>Air domain</h2>");
        match engine.air_pool(d) {
            Ok(pool) => {
                let _=write!(out,"<div class=\"facts\"><p><strong>{}</strong>uses remaining</p><p><strong>{}</strong>daily allowance</p><p><strong>{}</strong>effective Cleric level</p></div><p>{}</p>",pool.remaining,pool.total,pool.effective_level.map(|n|n.to_string()).unwrap_or("Unavailable".into()),escape(&pool.basis));
                begin(&mut out, id, rev, "air");
                out.push_str("<label>Action<select name=\"mode\"><option value=\"turn_earth\">Turn or destroy earth creatures</option><option value=\"rebuke_air\">Rebuke or command air creatures</option><option value=\"bolster_air\">Bolster air creatures</option><option value=\"dispel_turning\">Dispel turning on air creatures</option></select></label><div class=\"grid\">");
                input(&mut out, "Turning d20 roll", "roll", "10", "number");
                input(&mut out, "Turning damage d6 #1", "die1", "3", "number");
                input(&mut out, "Turning damage d6 #2", "die2", "4", "number");
                input(
                    &mut out,
                    "Original turning check total (dispel only)",
                    "original_check",
                    "",
                    "number",
                );
                out.push_str("</div><details><summary>Targets · blank rows are ignored</summary>");
                for i in 0..8 {
                    let _ = write!(out, "<fieldset><legend>Target {}</legend>", i + 1);
                    input(&mut out, "Target ID", &format!("target_{i}"), "", "text");
                    input(&mut out, "Hit Dice", &format!("hd_{i}"), "1", "number");
                    input(
                        &mut out,
                        "Turning resistance",
                        &format!("resistance_{i}"),
                        "0",
                        "number",
                    );
                    input(
                        &mut out,
                        "Distance in feet",
                        &format!("distance_{i}"),
                        "30",
                        "number",
                    );
                    check(&mut out, "Air subtype", &format!("air_{i}"));
                    check(&mut out, "Earth subtype", &format!("earth_{i}"));
                    check(
                        &mut out,
                        "Total cover / no line of effect",
                        &format!("cover_{i}"),
                    );
                    out.push_str("</fieldset>");
                }
                out.push_str("</details><button>Resolve &amp; spend one Air use</button></form>");
                begin(&mut out, id, rev, "air_manage");
                out.push_str("<fieldset><legend>Resource and effect management</legend><label>Operation<select name=\"operation\"><option value=\"advance\">Advance timed Air effects</option><option value=\"order\">Give a mental order (standard action)</option><option value=\"release\">Relinquish a commanded creature</option><option value=\"approach\">Approach a turned creature within 10 feet</option><option value=\"new_day\">Start a new day</option></select></label>");
                input(&mut out, "Target ID (when relevant)", "target", "", "text");
                input(&mut out, "Rounds elapsed", "rounds", "1", "number");
                out.push_str("</fieldset><button>Apply operation</button></form><h3>Commanded creatures</h3><ul>");
                for (id, hd) in &d.class_play.air_commands {
                    let _ = write!(out, "<li>{}: {} HD</li>", escape(id), hd);
                }
                out.push_str("</ul><h3>Timed effects</h3><ul>");
                for (id, effect) in &d.class_play.air_effects {
                    let _ = write!(
                        out,
                        "<li>{}: {}, {} rounds remain</li>",
                        escape(id),
                        escape(&effect.outcome),
                        effect.rounds
                    );
                }
                out.push_str("</ul>");
            }
            Err(error) => {
                let _ = write!(out, "<p class=\"muted\">{}</p>", escape(&error));
            }
        }
        out.push_str("<p class=\"muted\">Air uses are independent of undead turning and Divine Metamagic. <a href=\"https://www.d20srd.org/srd/spellLists/clericDomains.htm#airDomain\">Air domain</a> · <a href=\"https://www.d20srd.org/srd/combat/specialAttacks.htm#turnOrRebukeUndead\">Turning rules</a></p></section><section id=\"control\"><h2>Spell control</h2><p>Seize Concentration and Snatch Spell preserve the original caster's variables and remaining duration. A divine caster receives +2 on opposed caster-level checks.</p>");
        for (effect_id, spell) in &d.class_play.spells {
            let _=write!(out,"<article class=\"effect-card\"><h3>{} · {}</h3><p>Controller: {:?} · Status: {} · Remaining rounds: {}</p><p>Original caster: {}, CL {}, spell level {}. Reassertion pending: {}.</p>",escape(effect_id),escape(&spell.spell),spell.controller,escape(spell.ended.as_deref().unwrap_or("Active")),spell.remaining_rounds.map(|n|n.to_string()).unwrap_or("Concentration / no fixed limit".into()),escape(&spell.original_caster),spell.original_caster_level,spell.spell_level,spell.awaiting_reassertion);
            for (key, value) in &spell.variables {
                let _ = write!(out, "<p>{}: {}</p>", escape(key), escape(value));
            }
            if spell.ended.is_none() {
                begin(&mut out, id, rev, "control");
                let _ = write!(
                    out,
                    "<input type=\"hidden\" name=\"effect\" value=\"{}\">",
                    escape(effect_id)
                );
                out.push_str("<div class=\"grid\"><label>Event<select name=\"event\">");
                let mut events = vec![];
                if spell.awaiting_reassertion {
                    events.extend([
                        ("reassert", "Original caster attempts reassertion"),
                        ("decline", "Original caster declines reassertion"),
                    ]);
                } else {
                    match spell.controller {
                        SpellController::Original => {
                            if spell.concentration {
                                events.push(("seize", "Seize Concentration"));
                            } else if spell.controllable {
                                events.push(("snatch", "Snatch Spell"));
                            }
                        }
                        SpellController::Incantatrix => {
                            if spell.control_mode == Some(ControlMode::Seize) {
                                events.extend([
                                    ("maintain", "Maintain concentration"),
                                    ("lapse", "Let concentration lapse"),
                                ]);
                            }
                            if spell.dismissible {
                                events.push(("dismiss", "Dismiss spell"));
                            }
                        }
                        SpellController::None => {}
                    }
                    events.push(("advance", "Advance duration"));
                }
                events.push(("dispel", "Record a successful external dispel"));
                for (value, label) in events {
                    let _ = write!(out, "<option value=\"{value}\">{label}</option>");
                }
                out.push_str("</select></label>");
                input(&mut out, "Your casting track", "track", "Warmage", "text");
                input(
                    &mut out,
                    "Your d20 (or original caster's reassertion roll)",
                    "roll",
                    "10",
                    "number",
                );
                input(
                    &mut out,
                    "Original caster's opposed d20",
                    "original_roll",
                    "10",
                    "number",
                );
                input(
                    &mut out,
                    "Distance: to caster for Seize, to effect for Snatch",
                    "distance",
                    "30",
                    "number",
                );
                input(&mut out, "Rounds elapsed", "rounds", "1", "number");
                check(
                    &mut out,
                    "Original caster willingly transfers concentration",
                    "willing",
                );
                out.push_str("</div><button>Resolve &amp; record transition</button></form>");
            }
            out.push_str("</article>");
        }
        out.push_str("<details><summary>Register an existing spell effect</summary><p>Enter the original casting facts. Registration does not spend your spell slots or change control.</p>");
        begin(&mut out, id, rev, "register");
        out.push_str("<div class=\"grid\">");
        for (label, name, value, kind) in [
            ("Effect ID", "effect", "", "text"),
            ("Spell", "spell", "", "text"),
            ("Original caster", "original_caster", "", "text"),
            ("Original caster level", "original_cl", "1", "number"),
            ("Spell level", "spell_level", "1", "number"),
            (
                "Remaining rounds (blank: no fixed limit)",
                "remaining",
                "",
                "number",
            ),
            (
                "Rounds persisting after concentration ends",
                "tail",
                "0",
                "number",
            ),
        ] {
            input(&mut out, label, name, value, kind);
        }
        check(&mut out, "Original spell is divine", "divine");
        check(&mut out, "Requires concentration", "concentration");
        check(&mut out, "Responds to caster control", "controllable");
        check(&mut out, "Dismissible", "dismissible");
        out.push_str("</div><details><summary>Variables fixed at original casting</summary>");
        for i in 0..4 {
            out.push_str("<div class=\"grid\">");
            input(
                &mut out,
                "Variable (e.g. save DC)",
                &format!("variable_{i}"),
                "",
                "text",
            );
            input(
                &mut out,
                "Original value",
                &format!("value_{i}"),
                "",
                "text",
            );
            out.push_str("</div>");
        }
        out.push_str("</details><button>Register effect</button></form></details><p class=\"muted\"><a href=\"https://arkalseif.info/classes/players-guide-to-faerun--22/incantatrix/index.html\">Player's Guide to Faerun, pp.61–62</a></p></section>");
    }
    super::ui::finish(&mut out, "Encounter actions update the current character");
    out
}
pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<EncounterQuery>,
) -> Html<String> {
    Html(render(&state, &q.id, &q.view, ""))
}

fn execute(
    engine: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        "air" => {
            let mode = serde_json::from_value(serde_json::Value::String(value(f, "mode")?.into()))
                .map_err(|_| "Unknown Air action")?;
            let mut targets = vec![];
            for i in 0..8 {
                let id = value(f, &format!("target_{i}"))?;
                if id.is_empty() {
                    continue;
                }
                targets.push(AirTarget {
                    id: id.into(),
                    hit_dice: number(f, &format!("hd_{i}"))?,
                    turning_resistance: number(f, &format!("resistance_{i}"))?,
                    distance_feet: number(f, &format!("distance_{i}"))?,
                    total_cover: f.contains_key(&format!("cover_{i}")),
                    air: f.contains_key(&format!("air_{i}")),
                    earth: f.contains_key(&format!("earth_{i}")),
                });
            }
            let raw = value(f, "original_check")?;
            engine.air_attempt(
                d,
                &AirRequest {
                    mode,
                    roll: number(f, "roll")?,
                    damage_die_1: number(f, "die1")?,
                    damage_die_2: number(f, "die2")?,
                    targets,
                    original_turning_check: if raw.is_empty() {
                        None
                    } else {
                        Some(raw.parse().map_err(|_| "Invalid original turning check")?)
                    },
                },
            )
        }
        "air_manage" => engine.air_manage(
            d,
            value(f, "operation")?,
            value(f, "target")?,
            number(f, "rounds")?,
        ),
        "register" => {
            let remaining = value(f, "remaining")?;
            let mut variables = BTreeMap::new();
            for i in 0..4 {
                let key = value(f, &format!("variable_{i}"))?;
                if !key.is_empty() {
                    if variables
                        .insert(key.into(), value(f, &format!("value_{i}"))?.into())
                        .is_some()
                    {
                        return Err("Duplicate casting variable".into());
                    }
                }
            }
            engine.register_spell_effect(
                d,
                value(f, "effect")?,
                ControlledSpell {
                    spell: value(f, "spell")?.into(),
                    original_caster: value(f, "original_caster")?.into(),
                    original_caster_level: number(f, "original_cl")?,
                    spell_level: number(f, "spell_level")?,
                    original_divine: f.contains_key("divine"),
                    concentration: f.contains_key("concentration"),
                    controllable: f.contains_key("controllable"),
                    dismissible: f.contains_key("dismissible"),
                    remaining_rounds: if remaining.is_empty() {
                        None
                    } else {
                        Some(
                            remaining
                                .parse()
                                .map_err(|_| "Invalid remaining duration")?,
                        )
                    },
                    concentration_tail_rounds: number(f, "tail")?,
                    tail_remaining: None,
                    variables,
                    controller: SpellController::Original,
                    control_mode: None,
                    awaiting_reassertion: false,
                    ended: None,
                },
            )
        }
        "control" => {
            let event = match value(f, "event")? {
                "seize" | "snatch" => ControlEvent::Acquire {
                    mode: if value(f, "event")? == "seize" {
                        ControlMode::Seize
                    } else {
                        ControlMode::Snatch
                    },
                    track: value(f, "track")?.into(),
                    distance_feet: number(f, "distance")?,
                    willing: f.contains_key("willing"),
                    roll: number(f, "roll")?,
                    original_roll: number(f, "original_roll")?,
                },
                "maintain" => ControlEvent::Maintain,
                "lapse" => ControlEvent::Lapse,
                "reassert" => ControlEvent::Reassert {
                    roll: number(f, "roll")?,
                },
                "decline" => ControlEvent::DeclineReassertion,
                "dismiss" => ControlEvent::Dismiss {
                    track: value(f, "track")?.into(),
                    roll: number(f, "roll")?,
                    original_roll: number(f, "original_roll")?,
                },
                "advance" => ControlEvent::AdvanceTime {
                    rounds: number(f, "rounds")?,
                },
                "dispel" => ControlEvent::Dispel,
                _ => return Err("Unknown control event".into()),
            };
            engine.spell_control(d, value(f, "effect")?, &event)
        }
        _ => super::class_resource_forms::execute(engine, d, f),
    }
}
pub(super) async fn commit(
    State(state): State<AppState>,
    Form(fields): Form<BTreeMap<String, String>>,
) -> Html<String> {
    let _guard = state.mutation_lock.lock().await;
    let id = fields.get("id").cloned().unwrap_or_default();
    let result = (|| -> Result<String, String> {
        let mut env = state.repository.load(&id).map_err(|e| e.to_string())?;
        let revision = value(&fields, "revision")?
            .parse::<u64>()
            .map_err(|_| "Invalid revision")?;
        if revision != env.character.class_play.revision {
            return Err(
                "Stale or already submitted form. Reload before performing another action.".into(),
            );
        }
        let engine = cached_engine(
            &state,
            &resolve_path(
                &state.config.data_root,
                env.pcc.as_deref().ok_or("Missing rules")?,
            ),
        )?;
        let facts = if ["necro_lich", "necro_negative_day"].contains(&value(&fields, "op")?) {
            let (character, facts) =
                super::necromancy_forms::character_change(&engine, &env.character, &fields)?;
            env.character = character;
            facts
        } else {
            let result = execute(&engine, &env.character, &fields)?;
            env.character.class_play = result.state;
            result.facts
        };
        state
            .repository
            .replace_current(&env)
            .map_err(|e| e.to_string())?;
        Ok(format!("Recorded. {}", facts.join(" · ")))
    })();
    let view = fields.get("view").map(String::as_str).unwrap_or("all");
    Html(render(
        &state,
        &id,
        view,
        &result.unwrap_or_else(|e| format!("Not recorded: {e}")),
    ))
}
