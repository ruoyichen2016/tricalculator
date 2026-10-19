//! Casting-track-first ordinary forms. No browser-side rules or caller-owned resource balances.
use super::ui::{escape, url};
use super::*;
use axum::Form;
use pcgen_core::magic::*;
use std::{collections::BTreeMap, fmt::Write};

#[derive(Default, Deserialize)]
pub(super) struct Selection {
    #[serde(default)]
    id: String,
    #[serde(default)]
    track: String,
    #[serde(default)]
    spell: String,
}

fn render(
    state: &AppState,
    q: &Selection,
    message: &str,
    plan: Option<&pcgen_engine::execution::CastPlan>,
) -> String {
    let anchors: &[(&str, &str)] = if q.id.is_empty() {
        &[]
    } else {
        &[
            ("track-select", "Track & spell"),
            ("cast-options", "Cast & prepare"),
            ("access", "Spell access"),
        ]
    };
    let mut out = ui::start(
        ui::Page::Spellcasting,
        &q.id,
        "Spellcasting",
        "Choose a casting track, then a spell. Preview uses saved remaining resources.",
        anchors,
    );
    if !message.is_empty() {
        ui::notice(&mut out, message, message.starts_with("Cannot"));
    }
    ui::character_bar(
        &mut out,
        state,
        "/spellcasting",
        &q.id,
        &[("track", &q.track)],
        &[
            (&format!("/?id={}", url(&q.id)), "Character overview"),
            (&format!("/build?id={}", url(&q.id)), "Build history"),
            (&format!("/class-play?id={}", url(&q.id)), "Encounter desk"),
            (
                &format!("/workbench?id={}#casting", url(&q.id)),
                "Advanced proofs",
            ),
        ],
    );
    let result = (|| -> Result<(), String> {
        if q.id.is_empty() {
            out.push_str(
                "<section><p>Select a character above or on the <a href=\"/\">Character page</a> first.</p></section>",
            );
            return Ok(());
        }
        let (env, e) = character_context::load(state, &q.id)?;
        let d = &env.character;
        let s = e.evaluate(d);
        let track = if q.track.is_empty() {
            character_context::first_casting_track(&e, d)
        } else {
            q.track.clone()
        };
        let _ = write!(
            out,
            "<section id=\"track-select\"><h2>{}</h2><p><a href=\"/build?id={}\">Review acquisition qualifications</a>. This page validates the operation, not the complete build.</p><div class=\"grid\"><form method=\"get\" action=\"/spellcasting\"><input type=\"hidden\" name=\"id\" value=\"{}\"><label>Casting track<select name=\"track\">",
            escape(&d.name),
            url(&q.id),
            escape(&q.id)
        );
        for c in d.class_levels() {
            if e.pack
                .classes
                .get(&c.class_id)
                .is_some_and(|c| c.spellcasting_ability.is_some())
            {
                let _ = write!(
                    out,
                    "<option {}>{}</option>",
                    if c.class_id == track { "selected" } else { "" },
                    escape(&c.class_id)
                );
            }
        }
        out.push_str("</select></label><button>Open casting track</button></form>");
        let list = e.spell_catalog(d, &track)?;
        let _ = write!(
            out,
            "<form method=\"get\" action=\"/spellcasting\"><input type=\"hidden\" name=\"id\" value=\"{}\"><input type=\"hidden\" name=\"track\" value=\"{}\"><label>Known spell<select name=\"spell\"><option value=\"\">Choose a spell</option>",
            escape(&q.id),
            escape(&track)
        );
        for sp in list.iter().filter(|s| s.known && !s.prohibited) {
            let _ = write!(
                out,
                "<option value=\"{}\" {}>{} · level {} · {:?}</option>",
                escape(&sp.spell),
                if sp.spell == q.spell { "selected" } else { "" },
                escape(&sp.spell),
                sp.level,
                sp.power
            );
        }
        out.push_str("</select></label><button>Open spell</button></form></div>");

        // Track summary & prepared copies ledger
        let cl = s
            .caster_levels
            .get(&track)
            .map(ToString::to_string)
            .unwrap_or_else(|| "—".into());
        let known_count = list.iter().filter(|sp| sp.known && !sp.prohibited).count();
        let preps: Vec<_> = d
            .preparations
            .iter()
            .filter(|p| {
                p.class_id
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&track))
                    && p.times_memorized > 0
            })
            .collect();
        let spent_map = d.class_play.resources.slots_spent.get(&track);
        let _ = write!(
            out,
            "<div class=\"combat-strip\"><div class=\"combat-cell\"><span>Active Track</span><strong>{}</strong><small class=\"muted\">Caster level {}</small></div><div class=\"combat-cell\"><span>Known &amp; Accessible</span><strong>{} spells</strong><small class=\"muted\">{} total on list</small></div><div class=\"combat-cell\"><span>Daily Preparations</span><strong>{} entries</strong><small class=\"muted\">{}</small></div></div>",
            escape(&track),
            escape(&cl),
            known_count,
            list.len(),
            preps.len(),
            if preps.is_empty() {
                "None recorded".to_string()
            } else {
                preps
                    .iter()
                    .map(|p| format!("{} ×{}", p.name, p.times_memorized))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
        if let Some(spent) = spent_map.filter(|m| !m.is_empty()) {
            let spent_summary = spent
                .iter()
                .map(|(lvl, cnt)| format!("L{lvl}: {cnt} spent"))
                .collect::<Vec<_>>()
                .join(" · ");
            let _ = write!(
                out,
                "<p class=\"muted\"><strong>Slots spent today ({}):</strong> {}</p>",
                escape(&track),
                escape(&spent_summary)
            );
        }
        out.push_str("</section>");

        if let Some(s) = list
            .iter()
            .find(|s| s.spell == q.spell && s.known && !s.prohibited)
        {
            out.push_str(
                "<section id=\"cast-options\"><h2>Cast options</h2><form method=\"post\" action=\"/spellcasting\">",
            );
            for (key, value) in [
                ("id", q.id.clone()),
                ("track", track.clone()),
                ("spell", s.spell.clone()),
                ("revision", d.class_play.revision.to_string()),
            ] {
                let _ = write!(
                    out,
                    "<input type=\"hidden\" name=\"{key}\" value=\"{}\">",
                    escape(&value)
                );
            }
            let _ = write!(
                out,
                "<p>Selected: <strong>{}</strong> (Base level {}, {:?}, source: {})</p>",
                escape(&s.spell),
                s.level,
                s.power,
                escape(&s.source_list)
            );
            if e.pack
                .classes
                .get(&track)
                .is_some_and(|c| c.casting_type == pcgen_core::CastingType::Prepared)
            {
                super::forms::input(
                    &mut out,
                    "Ordinary prepared copies (before daily expenditure)",
                    "copies",
                    "1",
                    "number",
                );
                out.push_str("<button name=\"op\" value=\"prepare\">Record preparations</button>");
            }
            let mut available = Vec::new();
            let state_eff = pcgen_engine::apply_effects(&e.pack, d);
            for entity in &e.pack.entities {
                if let Some(rule) = entity.metadata.spell_rule {
                    if (pcgen_engine::effects::ability_matches_draft(d, entity)
                        || state_eff.active_abilities.contains(&entity.id.key))
                        && !matches!(
                            rule,
                            SpellRule::Heighten
                                | SpellRule::Earth
                                | SpellRule::Versatile
                                | SpellRule::DivineMetamagic
                        )
                        && !available.contains(&rule)
                    {
                        available.push(rule);
                        super::forms::check(
                            &mut out,
                            &entity.name,
                            &format!(
                                "meta:{}",
                                serde_json::to_value(rule).unwrap().as_str().unwrap()
                            ),
                        );
                    }
                }
            }
            if available.contains(&SpellRule::Sanctum) {
                super::forms::check(&mut out, "Inside the sanctum", "inside");
            }
            if d.level_in_class("Spellguard of Silverymoon") >= 4 {
                super::forms::check(
                    &mut out,
                    "Apply Spellguard (defensive personal arcane spells only)",
                    "spellguard",
                );
            }
            if d.level_in_class("War Weaver") >= 1 {
                super::forms::check(&mut out, "Cast through tapestry", "tapestry");
                super::forms::input(
                    &mut out,
                    "Recipient IDs, comma separated",
                    "recipients",
                    "",
                    "text",
                );
                super::forms::input(
                    &mut out,
                    "Shared spell option, if required",
                    "choice",
                    "",
                    "text",
                );
                if d.level_in_class("War Weaver") >= 2 {
                    super::forms::check(&mut out, "Store in quiescent weaving", "quiescent");
                }
            }
            out.push_str("<button name=\"op\" value=\"preview\">Preview</button> <button name=\"op\" value=\"commit\">Cast and spend resources</button></form><details><summary>Other casting tools</summary>");
            let _ = write!(
                out,
                "<p><a href=\"/workbench?id={}#casting\">Conditional entry proofs, Heighten, Earth Spell and advanced metamagic</a></p>",
                url(&q.id)
            );
            out.push_str("</details></section>");
        }
        if let Some(p) = plan {
            let r = &p.resolution;
            out.push_str(
                "<section id=\"result\"><h2>Operation result</h2><dl class=\"resolution-grid\">",
            );
            for (label, value) in [
                ("Spell type", format!("{:?}", r.power)),
                ("Effective level", r.effective_level.to_string()),
                (
                    "Slot paid",
                    format!("{} × level {}", r.paid_slots, r.paid_slot_level),
                ),
                ("Caster level", r.caster_level.to_string()),
                ("Save DC", r.save_dc.to_string()),
                ("Range", format!("{:?}", r.range)),
                ("Action", r.casting_time_note.clone()),
                ("Turning", r.turning_cost.to_string()),
            ] {
                let _ = write!(
                    out,
                    "<div><dt>{label}</dt><dd>{}</dd></div>",
                    escape(&value)
                );
            }
            out.push_str("</dl><ol>");
            for step in &p.steps {
                let _ = write!(
                    out,
                    "<li><strong>{} · <span class=\"decision {:?}\">{}</span></strong>: {}</li>",
                    escape(&step.subject),
                    step.status,
                    step.status.label(),
                    escape(&step.explanation)
                );
            }
            out.push_str("</ol></section>");
        }
        out.push_str("<section id=\"access\"><h2>Spell access</h2><p>List access is separate from knowledge and preparation. Click any known spell to configure or preview it above.</p><div class=\"table-scroll\"><table><thead><tr><th>Spell</th><th>Level</th><th>Type</th><th>Known</th><th>Source list</th><th>Action</th></tr></thead><tbody>");
        for s in list {
            let action_cell = if s.known && !s.prohibited {
                format!(
                    "<a href=\"/spellcasting?id={}&track={}&spell={}#cast-options\">Select →</a>",
                    url(&q.id),
                    url(&track),
                    url(&s.spell)
                )
            } else {
                "—".to_string()
            };
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{:?}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&s.spell),
                s.level,
                s.power,
                if s.prohibited {
                    "Prohibited"
                } else if s.known {
                    "Yes"
                } else {
                    "No"
                },
                escape(&s.source_list),
                action_cell
            );
        }
        out.push_str("</tbody></table></div></section>");
        Ok(())
    })();
    if let Err(e) = result {
        ui::notice(&mut out, &e, true);
    }
    ui::finish(
        &mut out,
        "Preview does not spend resources. A failed commit leaves resources unchanged.",
    );
    out
}

pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<Selection>,
) -> Html<String> {
    Html(render(&state, &q, "", None))
}

pub(super) async fn action(
    State(state): State<AppState>,
    Form(f): Form<BTreeMap<String, String>>,
) -> Html<String> {
    let _lock = state.mutation_lock.lock().await;
    let q = Selection {
        id: f.get("id").cloned().unwrap_or_default(),
        track: f.get("track").cloned().unwrap_or_default(),
        spell: f.get("spell").cloned().unwrap_or_default(),
    };
    let result = (|| -> Result<(String, Option<pcgen_engine::execution::CastPlan>), String> {
        let (mut env, e) = character_context::load(&state, &q.id)?;
        let revision = f
            .get("revision")
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("Missing revision")?;
        if revision != env.character.class_play.revision {
            return Err("Character changed; reload this form".into());
        }
        if f.get("op").is_some_and(|s| s == "prepare") {
            let copies = f
                .get("copies")
                .and_then(|s| s.parse().ok())
                .ok_or("Invalid preparation count")?;
            env.character = e.prepare_spell(&env.character, &q.track, &q.spell, copies)?;
            state
                .repository
                .replace_current(&env)
                .map_err(|e| e.to_string())?;
            return Ok(("Preparations recorded. No spell was cast.".into(), None));
        }
        let mut r = CastRequest {
            class_id: q.track.clone(),
            spell: q.spell.clone(),
            inside_sanctum: f.contains_key("inside"),
            spellguard_touch: f.contains_key("spellguard"),
            tapestry: f.contains_key("tapestry"),
            quiescent: f.contains_key("quiescent"),
            ..Default::default()
        };
        for key in f.keys() {
            if let Some(rule) = key.strip_prefix("meta:") {
                r.metamagic.push(
                    serde_json::from_value(serde_json::Value::String(rule.into()))
                        .map_err(|_| "Unknown metamagic")?,
                );
            }
        }
        if r.tapestry {
            r.tapestry_targets = Some(TapestryTargets {
                recipients: f
                    .get("recipients")
                    .map(|s| {
                        s.split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                choice: f.get("choice").filter(|s| !s.trim().is_empty()).cloned(),
            });
        }
        let plan = e.plan_cast(&env.character, &r)?;
        let message = if f.get("op").is_some_and(|s| s == "commit") {
            let result = e.execute_cast(&env.character, &r, revision)?;
            env.character.class_play = result.state;
            state
                .repository
                .replace_current(&env)
                .map_err(|e| e.to_string())?;
            result.facts.join(" ")
        } else {
            "Preview only. No resources spent.".into()
        };
        Ok((message, Some(plan)))
    })();
    match result {
        Ok((message, plan)) => Html(render(&state, &q, &message, plan.as_ref())),
        Err(e) => Html(render(&state, &q, &format!("Cannot execute: {e}"), None)),
    }
}
