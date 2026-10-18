use super::ui::{escape, url};
use super::*;
use std::fmt::Write;
pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<character_context::CharacterQuery>,
) -> Html<String> {
    let mut out = ui::start(
        ui::Page::Workbench,
        &q.id,
        "Character",
        "Current values, acquisition status and the next useful action.",
        &[],
    );
    out.push_str("<section><h2>Your characters</h2><form method=\"get\" action=\"/\"><label>Character<select name=\"id\"><option value=\"\">Choose a character</option>");
    if let Ok(items) = state.repository.list() {
        for item in items {
            let _ = write!(
                out,
                "<option value=\"{}\" {}>{} · level {}</option>",
                escape(&item.id),
                if item.id == q.id { "selected" } else { "" },
                escape(&item.name),
                item.total_level
            );
        }
    }
    out.push_str("</select></label><button>Open character</button></form><p><a href=\"/editor\">Create a character</a> · <a href=\"/cases\">Explore extreme cases</a></p></section>");
    if !q.id.is_empty() {
        let result = (|| -> Result<(), String> {
            let (env, e) = character_context::load(&state, &q.id)?;
            let d = &env.character;
            let s = e.evaluate(d);
            let audit = e.audit_build(d);
            let _=write!(out,"<section><h2>{}</h2><p>{} · {} · Level {}</p><p>Acquisition status: <strong>{}</strong></p><div class=\"metrics\">",escape(&d.name),escape(&d.race_id),escape(&d.alignment),s.total_level,audit.status().label());
            for (label, value, path) in [
                ("Build", format!("{} levels", d.total_level()), "/build"),
                ("Hit points", s.hit_points.to_string(), "/class-play"),
                (
                    "Base attack",
                    s.base_attack_bonus.to_string(),
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
            out.push_str("<section><h2>Needs attention</h2><ul>");
            for check in audit
                .decisions
                .iter()
                .filter(|c| c.status != pcgen_core::build::DecisionStatus::Satisfied)
                .take(12)
            {
                let _ = write!(
                    out,
                    "<li><a href=\"/build?id={}&level={}\">Level {} · {}</a> — {}: {}</li>",
                    url(&q.id),
                    check.level.max(1),
                    check.level,
                    escape(&check.subject),
                    check.status.label(),
                    escape(&check.explanation)
                );
            }
            for error in &s.diagnostics {
                let _ = write!(out, "<li>{}</li>", escape(error));
            }
            out.push_str("</ul></section>");
            out.push_str("<section><h2>Class levels</h2><table><thead><tr><th>Class</th><th>Actual level</th><th>Caster level (not class level)</th></tr></thead><tbody>");
            for c in d.class_levels() {
                let _ = write!(
                    out,
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape(&c.class_id),
                    c.level,
                    s.caster_levels
                        .get(&c.class_id)
                        .map(ToString::to_string)
                        .unwrap_or("—".into())
                );
            }
            let _=write!(out,"</tbody></table><p><a href=\"/classes?id={}\">Read class abilities and implementation coverage</a> · <a href=\"/workbench?id={}\">Advanced operation tools</a></p></section>",url(&q.id),url(&q.id));
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
