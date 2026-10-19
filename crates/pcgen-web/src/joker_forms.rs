//! Ordinary read-only forms; all arithmetic and class rules live in the engine.
use super::*;
use axum::Form;
use pcgen_engine::{joker::JokerCase, spell_features::deity_favor};
use std::{collections::BTreeMap, fmt::Write};
fn render(state: &AppState, id: &str, f: &BTreeMap<String, String>) -> String {
    let mut out = ui::start(
        ui::Page::Cases,
        id,
        "The Joker stress case",
        "Class progression, casting type and temporary hit points.",
        &[],
    );
    let q = ui::id_query(id);
    let _ = write!(
        out,
        "<p class=\"sub-nav\"><a href=\"/cases{q}\">← All cases</a> <a href=\"/classes{q}\">All class levels</a> <a href=\"/incarnum{q}\">Warforged case</a> <a href=\"/martial{q}\">Shadow Pounce case</a></p><p>Read-only analysis of the supplied forum build. Missing history is not treated as satisfied prerequisites.</p>"
    );
    let result = (|| -> Result<(), String> {
        let e = cached_engine(
            state,
            &state
                .config
                .cohort_root
                .join("data/rules/rainbow-warweaver.rules.json"),
        )?;
        let c: JokerCase = serde_json::from_str(include_str!(
            "../../../tests/fixtures/forum-builds/joker.json"
        ))
        .map_err(|e| e.to_string())?;
        let r = e.audit_joker_case(&c);
        for message in &r.errors {
            ui::notice(&mut out, message, true);
        }
        out.push_str("<table><thead><tr><th>Character level</th><th>FS casting contributions</th><th>Rejected advancements</th></tr></thead><tbody>");
        for row in r.milestones {
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                row.level,
                row.favored_soul_casting_level,
                row.rejected_advancements.len()
            );
        }
        out.push_str("</tbody></table><p>Totals retain invalid levels for diagnosis. They do not describe a legal character.</p><ul>");
        for message in r.limits {
            let _ = write!(out, "<li>{}</li>", ui::escape(&message));
        }
        out.push_str("</ul>");
        let _ = write!(
            out,
            "<h2>Deity's Favor boundary probe</h2><p>Assumes the replacement feature is selected and all selected recipients are affected allies. This resolves one use; previous temporary HP pools are not added.</p><form method=\"post\" action=\"/joker\"><input type=\"hidden\" name=\"id\" value=\"{}\">",
            ui::escape(id)
        );
        for (key, label, default, min, max) in [
            ("class_level", "Actual Favored Soul level", 4, 1, 20),
            ("spell_level", "Favored Soul spell level", 6, 0, 9),
            ("allies", "Affected allies", 4, 1, 100),
            ("chosen", "Chosen allies", 1, 1, 100),
            ("spell_hp", "Temporary HP from spell itself", 0, 0, 1000),
            (
                "spell_duration",
                "Spell temporary HP duration (rounds)",
                0,
                0,
                100000,
            ),
        ] {
            let value = f.get(key).map(String::as_str).unwrap_or("");
            let value = if value.is_empty() {
                default.to_string()
            } else {
                value.to_string()
            };
            let _=write!(out,"<label>{label}<input type=\"number\" name=\"{key}\" min=\"{min}\" max=\"{max}\" value=\"{}\" required></label>",ui::escape(&value));
        }
        out.push_str("<button type=\"submit\">Resolve one use</button></form>");
        let value = |key: &str, default: u32| -> Result<u32, String> {
            f.get(key)
                .map(|v| v.parse().map_err(|_| format!("Invalid {key}")))
                .unwrap_or(Ok(default))
        };
        match deity_favor(
            value("class_level", 4)?,
            true,
            "Favored Soul",
            value("spell_level", 6)?,
            value("allies", 4)?,
            value("chosen", 1)?,
            value("spell_hp", 0)?,
            value("spell_duration", 0)?,
        ) {
            Ok(b) => {
                let _ = write!(
                    out,
                    "<p>{} temporary HP; {} rounds; {} recipient(s).</p>",
                    b.temporary_hp, b.duration_rounds, b.recipients
                );
            }
            Err(err) => ui::notice(&mut out, &err, true),
        }
        Ok(())
    })();
    if let Err(err) = result {
        ui::notice(&mut out, &err, true);
    }
    ui::finish(&mut out,"Spellguard requires a defensive personal arcane spell; this rider does not change spell type.");
    out
}
pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<super::character_context::CharacterQuery>,
) -> Html<String> {
    Html(render(&state, &q.id, &BTreeMap::new()))
}
pub(super) async fn replay(
    State(state): State<AppState>,
    Form(f): Form<BTreeMap<String, String>>,
) -> Html<String> {
    let id = f.get("id").map(String::as_str).unwrap_or("");
    Html(render(&state, id, &f))
}
