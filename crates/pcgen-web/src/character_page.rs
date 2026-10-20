use super::ui::{escape, url};
use super::*;
use std::fmt::Write;

pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<character_context::CharacterQuery>,
) -> Html<String> {
    let anchors: &[(&str, &str)] = if q.id.is_empty() {
        &[]
    } else {
        &[
            ("summary", "Overview"),
            ("attention", "Needs attention"),
            ("sheet", "Abilities & combat"),
            ("classes", "Class levels"),
            ("skills", "Feats & skills"),
        ]
    };
    let mut out = ui::start(
        ui::Page::Workbench,
        &q.id,
        "Character",
        "Current values, acquisition status and the next useful action.",
        anchors,
    );
    out.push_str("<section><h2>Your characters</h2><form method=\"get\" action=\"/\"><label>Character<select name=\"id\"><option value=\"\">Choose a character</option>");
    if let Ok(items) = state.repository.list() {
        for item in items {
            let classes = if item.class_ids.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.class_ids.join("/"))
            };
            let _ = write!(
                out,
                "<option value=\"{}\" {}>{} · level {}{}</option>",
                escape(&item.id),
                if item.id == q.id { "selected" } else { "" },
                escape(&item.name),
                item.total_level,
                escape(&classes)
            );
        }
    }
    out.push_str("</select></label><button>Open character</button></form><p><a href=\"/build\">Create a character with build history</a> · <a href=\"/editor\">Create a character</a> · <a href=\"/cases\">Explore extreme cases</a></p></section>");
    if !q.id.is_empty() {
        let result = (|| -> Result<(), String> {
            let (env, e) = character_context::load(&state, &q.id)?;
            let d = &env.character;
            let s = e.evaluate(d);
            let audit = e.audit_build(d);
            let tags_badge = if audit.tags.is_empty() {
                String::new()
            } else {
                format!(
                    " · {}",
                    audit
                        .tags
                        .iter()
                        .map(|t| format!(
                            "<span class=\"badge badge-{}\">{}</span>",
                            t.label().to_lowercase(),
                            escape(t.label())
                        ))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            let status_class = format!("{:?}", audit.status());
            let _ = write!(
                out,
                "<section id=\"summary\" class=\"overview\"><div class=\"section-heading\"><div><p class=\"eyebrow\">ACTIVE CHARACTER</p><h2>{}</h2></div><div class=\"heading-actions\"><a class=\"text-link\" href=\"/build?id={}\">Edit build history →</a><a class=\"text-link\" href=\"/class-play?id={}\">Open encounter desk →</a></div></div><p>{} · {} · Level {}{}</p><p class=\"status-summary\"><span class=\"status-item\">Acquisition status: <span class=\"decision {}\"><strong>{}</strong></span></span><span class=\"status-item\">Derived snapshot: <strong>{}</strong></span></p><div class=\"metrics\">",
                escape(&d.name),
                url(&q.id),
                url(&q.id),
                escape(&d.race_id),
                escape(&d.alignment),
                s.total_level,
                tags_badge,
                escape(&status_class),
                audit.status().label(),
                if s.is_available() {
                    "Available"
                } else {
                    "Unavailable (blocked by diagnostics)"
                }
            );
            for (label, value, path) in [
                ("Build", format!("{} levels", d.total_level()), "/build"),
                (
                    "Hit points",
                    if s.is_available() {
                        s.hit_points.to_string()
                    } else {
                        "Unavailable".into()
                    },
                    "/class-play",
                ),
                (
                    "Base attack",
                    if s.is_available() {
                        format!("{:+}", s.base_attack_bonus)
                    } else {
                        "Unavailable".into()
                    },
                    "/class-play",
                ),
                (
                    "Spellcasting",
                    format!("{} tracks", s.caster_levels.len()),
                    "/spellcasting",
                ),
            ] {
                ui::metric(
                    &mut out,
                    label,
                    &value,
                    &format!("{path}?id={}", url(&q.id)),
                );
            }
            out.push_str("</div></section>");

            out.push_str("<section id=\"attention\"><h2>Needs attention</h2>");
            let non_satisfied: Vec<_> = audit
                .decisions
                .iter()
                .filter(|c| c.status != pcgen_core::build::DecisionStatus::Satisfied)
                .take(12)
                .collect();
            if non_satisfied.is_empty() && s.diagnostics.is_empty() {
                out.push_str("<p class=\"muted\">All checked acquisition steps are satisfied and no engine diagnostics were reported.</p>");
            } else {
                out.push_str("<ul>");
                for check in non_satisfied {
                    let tag_prefix = if check.tags.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " [{}]",
                            check
                                .tags
                                .iter()
                                .map(|t| t.label())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    let _ = write!(
                        out,
                        "<li><a href=\"/build?id={}&level={}\">Level {} · {}</a>: <span class=\"decision {:?}\">{}</span>{} <span class=\"decision-explanation\">{}</span></li>",
                        url(&q.id),
                        check.level.max(1),
                        check.level,
                        escape(&check.subject),
                        check.status,
                        check.status.label(),
                        escape(&tag_prefix),
                        escape(&check.explanation)
                    );
                }
                for error in &s.diagnostics {
                    let _ = write!(
                        out,
                        "<li><strong>Diagnostic</strong>: {}</li>",
                        escape(error)
                    );
                }
                out.push_str("</ul>");
            }
            out.push_str("</section>");

            out.push_str("<section id=\"sheet\"><h2>Abilities &amp; combat statistics</h2>");
            if s.is_available() {
                out.push_str("<div class=\"stat-grid\">");
                for stat in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                    let score = s
                        .stats
                        .get(stat)
                        .or_else(|| d.stats.get(stat))
                        .copied()
                        .unwrap_or(10);
                    let modifier = (score - 10).div_euclid(2);
                    let base_note = d
                        .build_history
                        .as_ref()
                        .and_then(|h| h.initial_stats.get(stat))
                        .filter(|&&init| init != score)
                        .map(|init| format!("<span class=\"stat-base\">Origin {init}</span>"))
                        .unwrap_or_default();
                    let _ = write!(
                        out,
                        "<div class=\"stat-card\"><span class=\"stat-label\">{stat}</span><strong class=\"stat-score\">{score}</strong><span class=\"stat-mod\">{modifier:+}</span>{base_note}</div>"
                    );
                }
                out.push_str("</div><div class=\"combat-strip\">");
                let fort = s.saves.get("Fortitude").copied().unwrap_or(0);
                let reflex = s.saves.get("Reflex").copied().unwrap_or(0);
                let will = s.saves.get("Will").copied().unwrap_or(0);
                for (label, primary, sub) in [
                    (
                        "Armor Class",
                        format!("{}", s.armor_class),
                        format!("Touch {} · Flat-footed {}", s.touch_ac, s.flat_footed_ac),
                    ),
                    (
                        "Saving Throws",
                        format!("Fort {fort:+} · Ref {reflex:+} · Will {will:+}"),
                        "Total save modifiers".to_string(),
                    ),
                    (
                        "Initiative",
                        format!("{:+}", s.initiative),
                        "DEX + misc modifiers".to_string(),
                    ),
                    (
                        "Attack Totals",
                        format!("Melee {:+} · Ranged {:+}", s.melee_attack, s.ranged_attack),
                        format!(
                            "BAB {:+} · Grapple {:+}",
                            s.base_attack_bonus, s.grapple_attack
                        ),
                    ),
                    (
                        "Budgets & Pools",
                        format!("{} skill pts · {} feats", s.skill_points, s.feat_slots),
                        format!("{} active abilities", s.active_abilities.len()),
                    ),
                ] {
                    let _ = write!(
                        out,
                        "<div class=\"combat-cell\"><span>{}</span><strong>{}</strong><small class=\"muted\">{}</small></div>",
                        escape(label),
                        escape(&primary),
                        escape(&sub)
                    );
                }
                if s.spell_resistance.is_some()
                    || !s.damage_reduction.is_empty()
                    || !s.vision.is_empty()
                {
                    let sr = s
                        .spell_resistance
                        .map(|v| format!("SR {v}"))
                        .unwrap_or_else(|| "SR —".into());
                    let dr = if s.damage_reduction.is_empty() {
                        "DR —".into()
                    } else {
                        format!("DR {}", s.damage_reduction.join(", "))
                    };
                    let vis = if s.vision.is_empty() {
                        "Normal vision".into()
                    } else {
                        s.vision.join(", ")
                    };
                    let _ = write!(
                        out,
                        "<div class=\"combat-cell\"><span>Traits &amp; Senses</span><strong>{} · {}</strong><small class=\"muted\">{}</small></div>",
                        escape(&sr),
                        escape(&dr),
                        escape(&vis)
                    );
                }
                out.push_str("</div>");
            } else {
                out.push_str("<p class=\"error\">Derived combat and ability totals are currently unavailable because the engine reported unresolved diagnostics. Resolve the items in Needs attention above.</p>");
            }
            out.push_str("</section>");

            out.push_str("<section id=\"classes\"><h2>Class levels</h2><div class=\"table-scroll\"><table><thead><tr><th>Class</th><th>Actual level</th><th>Caster level (not class level)</th><th>Recorded HP rolls</th></tr></thead><tbody>");
            for c in d.class_levels() {
                let hp_rolls = c
                    .hit_points_by_level
                    .iter()
                    .map(|r| r.map(|n| n.to_string()).unwrap_or_else(|| "—".into()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = write!(
                    out,
                    "<tr><td><a href=\"/classes?id={}&class={}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    url(&q.id),
                    url(&c.class_id),
                    escape(&c.class_id),
                    c.level,
                    s.caster_levels
                        .get(&c.class_id)
                        .map(ToString::to_string)
                        .unwrap_or("—".into()),
                    escape(&hp_rolls)
                );
            }
            let _ = write!(
                out,
                "</tbody></table></div><p><a href=\"/classes?id={}\">Read class abilities and implementation coverage</a> · <a href=\"/workbench?id={}\">Advanced operation tools</a></p></section>",
                url(&q.id),
                url(&q.id)
            );

            out.push_str("<section id=\"skills\"><h2>Feats, abilities &amp; skills</h2><div class=\"grid\"><div><h3>Recorded feats &amp; choices</h3>");
            if d.abilities.is_empty() {
                out.push_str("<p class=\"muted\">No feats recorded on this draft.</p>");
            } else {
                out.push_str("<ul class=\"pill-list\">");
                for (pool, feat) in &d.abilities {
                    let lvl_note = d
                        .feat_acquired_at
                        .get(feat.as_str())
                        .map(|l| format!(" (L{l})"))
                        .unwrap_or_default();
                    let _ = write!(
                        out,
                        "<li><strong>{}</strong><span class=\"muted\"> · {}{}</span></li>",
                        escape(feat),
                        escape(pool),
                        escape(&lvl_note)
                    );
                }
                out.push_str("</ul>");
            }
            if !d.choices.is_empty() {
                out.push_str("<h3>Recorded choices</h3><ul>");
                for (k, v) in &d.choices {
                    let _ = write!(out, "<li><code>{}</code>: {}</li>", escape(k), escape(v));
                }
                out.push_str("</ul>");
            }
            out.push_str("</div><div><h3>Trained skills</h3>");
            if d.skills.is_empty() {
                out.push_str("<p class=\"muted\">No skill ranks recorded.</p>");
            } else {
                out.push_str("<div class=\"table-scroll\"><table><thead><tr><th>Skill</th><th>Ranks</th><th>Total</th></tr></thead><tbody>");
                for (skill, ranks) in &d.skills {
                    let total = if s.is_available() {
                        s.skills
                            .get(skill)
                            .map(|t| format!("{t:+}"))
                            .unwrap_or_else(|| "—".into())
                    } else {
                        "—".into()
                    };
                    let _ = write!(
                        out,
                        "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                        escape(skill),
                        ranks,
                        escape(&total)
                    );
                }
                out.push_str("</tbody></table></div>");
            }
            out.push_str("</div></div></section>");
            Ok(())
        })();
        if let Err(e) = result {
            ui::notice(&mut out, &e, true);
        }
    }
    ui::finish(
        &mut out,
        "Rules, build history and encounter resources are separate",
    );
    Html(out)
}
