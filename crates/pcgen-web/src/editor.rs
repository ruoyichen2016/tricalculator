//! Ordinary HTML forms; all rule evaluation remains on the server.
use super::ui::{escape, url};
use super::*;
use axum::Form;
use std::collections::BTreeMap;
use std::fmt::Write;
fn render(
    state: &AppState,
    id: &str,
    message: &str,
    submitted: Option<&BTreeMap<String, String>>,
) -> String {
    let mut out = super::ui::start(
        super::ui::Page::Studio,
        id,
        "Character setup",
        "Shape the character. Keep every level and choice visible.",
        &[
            ("characters", "Your characters"),
            ("identity", "Identity & abilities"),
            ("choices", "Levels & choices"),
        ],
    );
    super::ui::notice(&mut out, message, message.starts_with("Not saved:"));
    out.push_str("<section id=\"characters\"><div class=\"section-heading\"><h2>Your characters</h2><a class=\"text-link\" href=\"/editor\">New character +</a></div><nav>");
    match state.repository.list() {
        Ok(items) => {
            for item in items {
                let _ = write!(
                    out,
                    "<a href=\"/editor?id={}\">{}</a>",
                    url(&item.id),
                    escape(&item.name)
                );
            }
        }
        Err(e) => {
            let _ = write!(out, "<p>{}</p>", escape(&e.to_string()));
        }
    }
    out.push_str("</nav></section>");
    let env = if id.is_empty() {
        None
    } else {
        match state.repository.load(id) {
            Ok(e) => Some(e),
            Err(_) if submitted.is_some() => None,
            Err(e) => {
                let _ = write!(
                    out,
                    "<section class=\"error\">{}</section>",
                    escape(&e.to_string())
                );
                super::ui::finish(&mut out, "Open a character to continue");
                return out;
            }
        }
    };
    let draft = env
        .as_ref()
        .map(|e| e.character.clone())
        .unwrap_or_default();
    if draft.build_history.is_some() {
        let _=write!(out,"<section><h2>Acquisition history is active</h2><p><a href=\"/build?id={}\">Edit this character on the Build page</a>. Final choices are projected from recorded acquisitions.</p></section>",url(id));
        super::ui::finish(&mut out,"Acquisition inputs are edited in one place");return out;
    }
    let mut values = BTreeMap::new();
    values.insert(
        "id".into(),
        if id.is_empty() {
            String::new()
        } else {
            id.into()
        },
    );
    values.insert(
        "pcc".into(),
        env.as_ref().and_then(|e| e.pcc.clone()).unwrap_or_else(|| {
            std::path::absolute(
                state
                    .config
                    .cohort_root
                    .join("data/rules/rainbow-warweaver.rules.json"),
            )
            .expect("absolute rule path")
            .to_string_lossy()
            .into_owned()
        }),
    );
    for (key, value) in [
        ("name", &draft.name),
        ("race_id", &draft.race_id),
        ("alignment", &draft.alignment),
    ] {
        values.insert(key.into(), value.clone());
    }
    for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
        values.insert(key.into(), draft.stats.get(key).unwrap_or(&10).to_string());
    }
    let json = serde_json::to_value(&draft).expect("serializable draft");
    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            if !values.contains_key(key) {
                values.insert(key.clone(), serde_json::to_string_pretty(value).unwrap());
            }
        }
    }
    if let Some(submitted) = submitted {
        values.extend(submitted.clone());
    }
    out.push_str("<form method=\"post\" action=\"/editor\"><section id=\"identity\"><h2>Identity &amp; abilities</h2><div class=\"grid\">");
    for (key, label) in [
        ("id", "Character ID"),
        ("name", "Name"),
        ("race_id", "Race"),
        ("alignment", "Alignment"),
        ("pcc", "Rule database"),
    ]
    .into_iter()
    .chain(["STR", "DEX", "CON", "INT", "WIS", "CHA"].map(|k| (k, k)))
    {
        let _ = write!(
            out,
            "<label>{label}<input name=\"{key}\" value=\"{}\" required></label>",
            escape(&values[key])
        );
    }
    out.push_str("</div></section><section id=\"choices\"><h2>Character choices</h2><p class=\"muted\">Edit your levels, feats and resources using ordinary forms. Rules are evaluated when you save.</p>");
    let engine = cached_engine(
        state,
        &resolve_path(&state.config.data_root, &values["pcc"]),
    );
    if let Err(error) = &engine {
        let _ = write!(out, "<p class=\"error\">{}</p>", escape(error));
    }
    super::editor_fields::render(&mut out, &draft, engine.as_deref().ok());
    let _ = write!(
        out,
        "<input type=\"hidden\" name=\"revision\" value=\"{}\">",
        draft.class_play.revision
    );
    out.push_str("<fieldset><legend>Add one class level</legend><label>Class<select name=\"next_class\"><option value=\"\">Do not add a level</option>");
    if let Ok(e) = &engine {
        for class in super::character_context::classes(e) {
            let _ = write!(
                out,
                "<option value=\"{}\">{}</option>",
                escape(&class.id),
                escape(&class.name)
            );
        }
    }
    out.push_str("</select></label><label>Hit die roll (before Constitution)<input type=\"number\" min=\"1\" name=\"next_hp\" value=\"3\"></label><label>Advance casting class (only for advancing levels)<input name=\"next_track\" value=\"\"></label></fieldset>");
    if let Ok(e) = &engine {
        for entry in draft.class_levels() {
            if let Some(m) = e
                .pack
                .entities
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&entry.class_id))
                .and_then(|c| c.metadata.class_magic.as_ref())
            {
                if let Some(learning) = &m.advanced_learning {
                    let _=write!(out,"<details><summary>{} Advanced Learning</summary><p>Choose from {}. Eligible schools: {}.</p>",escape(&entry.class_id),escape(&learning.lists.join(", ")),escape(&learning.schools.join(", ")));
                    for level in learning.levels.iter().filter(|l| **l <= entry.level) {
                        let key = format!("{}.AdvancedLearning.{level}", entry.class_id);
                        let value = draft.choices.get(&key).map(String::as_str).unwrap_or("");
                        let _=write!(out,"<label>Class level {level}<input name=\"choice:{}\" value=\"{}\"></label>",escape(&key),escape(value));
                    }
                    out.push_str("</details>");
                }
            }
        }
    }
    out.push_str("<div class=\"save-bar\"><span>Save your choices and update derived values.</span><button type=\"submit\">Save &amp; evaluate</button></div></section></form>");
    super::ui::finish(&mut out, "Your choices are saved when you submit");
    if let Some(fields) = submitted {
        super::workbench::restore_form(
            out,
            "/editor",
            &serde_json::to_value(fields).expect("form values"),
        )
    } else {
        out
    }
}
pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<super::character_context::CharacterQuery>,
) -> Html<String> {
    Html(render(&state, &q.id, "", None))
}
pub(super) async fn save(
    State(state): State<AppState>,
    Form(fields): Form<BTreeMap<String, String>>,
) -> Html<String> {
    let _guard = state.mutation_lock.lock().await;
    let id = fields.get("id").cloned().unwrap_or_default();
    let result = (|| -> Result<String, String> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        {
            return Err("Use letters, digits, hyphens or underscores for the character ID".into());
        }
        let mut draft = match state.repository.load(&id) {
            Ok(e) => e.character,
            Err(BuilderError::NotFound(_)) => CharacterDraft::default(),
            Err(error) => return Err(error.to_string()),
        };
        let mut value = serde_json::to_value(&draft).map_err(|e| e.to_string())?;
        for key in ["id", "name", "race_id", "alignment"] {
            value[key] =
                serde_json::Value::String(fields.get(key).ok_or(format!("Missing {key}"))?.clone());
        }
        for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
            let n: i32 = fields
                .get(key)
                .ok_or(format!("Missing {key}"))?
                .parse()
                .map_err(|_| format!("{key} requires an integer"))?;
            if n < 1 {
                return Err(format!("{key} must be positive"));
            }
            value["stats"][key] = n.into();
        }
        draft = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let pcc = fields.get("pcc").ok_or("Missing rule database")?;
        let engine = cached_engine(&state, &resolve_path(&state.config.data_root, pcc))?;
        let revision = fields
            .get("revision")
            .ok_or("Missing form revision")?
            .parse::<u64>()
            .map_err(|_| "Invalid revision")?;
        if revision != draft.class_play.revision {
            return Err(
                "The character changed since this form was opened. Reload before saving.".into(),
            );
        }
        if draft.build_history.is_some() { return Err("This character uses acquisition history. Edit levels and choices on the Build page.".into()); }
        super::editor_fields::apply(&fields, &mut draft, &engine)?;

        if let Some(class) = fields.get("next_class").filter(|c| !c.is_empty()) {
            let definition = engine.pack.classes.get(class).ok_or("Unknown class")?;
            let level = draft
                .advancement
                .iter()
                .flat_map(|f| &f.lanes)
                .filter(|l| l.class_id == *class)
                .map(|l| l.class_level)
                .max()
                .unwrap_or(0)
                + 1;
            if level > definition.max_level {
                return Err(format!(
                    "{} has only {} class levels",
                    class, definition.max_level
                ));
            }
            let hp: i32 = fields
                .get("next_hp")
                .ok_or("Missing hit die roll")?
                .parse()
                .map_err(|_| "Invalid hit die roll")?;
            let die: i32 = definition
                .hit_die
                .trim_start_matches('d')
                .parse()
                .map_err(|_| "Hit die unavailable")?;
            if hp < 1 || hp > die {
                return Err(format!("Hit die roll must be 1 through {die}"));
            }
            let mut frame =
                pcgen_core::AdvancementFrame::single(draft.total_level() + 1, class, level, hp, 0);
            if definition.level_spellcaster_advance.contains_key(&level) {
                let track = fields
                    .get("next_track")
                    .filter(|s| !s.is_empty())
                    .ok_or("Select the casting class to advance for this prestige level")?;
                frame.lanes[0].advances.push(track.clone());
            }
            draft.advancement.push(frame);
        }
        for (key, value) in &fields {
            if let Some(key) = key.strip_prefix("choice:") {
                if value.trim().is_empty() {
                    draft.choices.remove(key);
                } else {
                    draft.choices.insert(key.into(), value.trim().into());
                }
            }
        }
        draft.class_play.revision = draft
            .class_play
            .revision
            .checked_add(1)
            .ok_or("Revision overflow")?;
        let snapshot = engine.evaluate(&draft);
        let envelope = NativeSaveEnvelope::new(
            draft,
            engine.pack.header.rules_fingerprint.clone(),
            engine.pack.header.data_ref.clone(),
        )
        .with_pcc(Some(pcc.clone()));
        state
            .repository
            .replace_current(&envelope)
            .map_err(|e| e.to_string())?;
        Ok(if snapshot.is_available() {
            "Saved. Derived values are available in the rules workbench.".into()
        } else {
            format!(
                "Saved inputs. Derived values unavailable: {}",
                snapshot.diagnostics.join("; ")
            )
        })
    })();
    match result {
        Ok(message) => Html(render(&state, &id, &message, None)),
        Err(error) => Html(render(
            &state,
            &id,
            &format!("Not saved: {error}"),
            Some(&fields),
        )),
    }
}
