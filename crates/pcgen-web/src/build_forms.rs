//! Acquisition editing through ordinary forms. Audit and projection belong to the engine.
use super::ui::{escape, url};
use super::*;
use axum::Form;
use pcgen_core::{build::*, DraftSpell};
use std::{collections::BTreeMap, fmt::Write};
#[derive(Default, Deserialize)]
pub(super) struct QueryForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    level: u32,
}
pub(super) fn decisions(out: &mut String, a: &pcgen_engine::build::BuildAudit, level: Option<u32>) {
    let _=write!(out,"<p class=\"audit-status\">Build status: <strong>{}</strong>. Derived numbers do not certify legality.</p><div class=\"table-scroll\"><table><thead><tr><th>Level</th><th>Subject</th><th>Status</th><th>Evidence</th></tr></thead><tbody>",a.status().label());
    for d in a
        .decisions
        .iter()
        .filter(|d| level.is_none_or(|n| d.level == n || d.level == 0))
    {
        let _ = write!(
            out,
            "<tr><td>{}</td><td>{}</td><td><span class=\"decision {:?}\">{}</span></td><td>{}",
            d.level,
            escape(&d.subject),
            d.status,
            d.status.label(),
            escape(&d.explanation)
        );
        if let Some(source) = &d.source {
            let _ = write!(
                out,
                "<details><summary>RAW source</summary><p>{}</p></details>",
                escape(source)
            );
        }
        out.push_str("</td></tr>");
    }
    out.push_str("</tbody></table></div>");
}
fn begin(out: &mut String, id: &str, revision: u64, level: u32, op: &str) {
    let _=write!(out,"<form method=\"post\" action=\"/build\"><input type=\"hidden\" name=\"id\" value=\"{}\"><input type=\"hidden\" name=\"revision\" value=\"{revision}\"><input type=\"hidden\" name=\"level\" value=\"{level}\"><input type=\"hidden\" name=\"op\" value=\"{op}\">",escape(id));
}
fn render(state: &AppState, q: &QueryForm, message: &str) -> String {
    let mut out = ui::start(
        ui::Page::Studio,
        &q.id,
        "Build",
        "Record acquisitions in order. Class entry is checked before that level's choices.",
        &[],
    );
    if !message.is_empty() {
        ui::notice(&mut out, message, message.starts_with("Not saved"));
    }
    if q.id.is_empty() {
        out.push_str("<section><h2>Select a character</h2><p><a href=\"/editor\">Create a character</a></p><ul>");
        if let Ok(items) = state.repository.list() {
            for item in items {
                let _ = write!(
                    out,
                    "<li><a href=\"/build?id={}\">{}</a></li>",
                    url(&item.id),
                    escape(&item.name)
                );
            }
        }
        out.push_str("</ul></section>");
        ui::finish(&mut out, "No history is invented for imported characters");
        return out;
    }
    let result = (|| -> Result<(), String> {
        let (env, e) = character_context::load(state, &q.id)?;
        let d = &env.character;
        let rev = d.class_play.revision;
        let n = if q.level == 0 {
            d.total_level().max(1)
        } else {
            q.level
        };
        let _ = write!(
            out,
            "<section><h2>{}</h2><nav aria-label=\"Build levels\">",
            escape(&d.name)
        );
        for f in &d.advancement {
            let label = f
                .lanes
                .iter()
                .map(|l| format!("{} {}", l.class_id, l.class_level))
                .collect::<Vec<_>>()
                .join(" / ");
            let _ = write!(
                out,
                "<a href=\"/build?id={}&level={}\" {}>Level {} · {}</a>",
                url(&q.id),
                f.character_level,
                if f.character_level == n {
                    "aria-current=\"step\""
                } else {
                    ""
                },
                f.character_level,
                escape(&label)
            );
        }
        let _ = write!(
            out,
            "<a href=\"/build?id={}&level={}\">Add level +</a></nav></section>",
            url(&q.id),
            d.total_level() + 1
        );
        if let Some(h) = &d.build_history {
            out.push_str("<section><h2>Acquisition history</h2>");
            out.push_str("<details><summary>Edit starting identity and scores</summary>");
            begin(&mut out, &q.id, rev, n, "origin");
            super::forms::input(&mut out, "Name", "name", &d.name, "text");
            for ability in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                super::forms::input(
                    &mut out,
                    ability,
                    ability,
                    &h.initial_stats.get(ability).unwrap_or(&10).to_string(),
                    "number",
                );
            }
            super::forms::input(
                &mut out,
                "Initial alignment",
                "alignment",
                &h.initial_alignment,
                "text",
            );
            out.push_str("<button>Save origin and re-evaluate history</button></form></details>");
            begin(&mut out, &q.id, rev, n, "completeness");
            super::forms::input(
                &mut out,
                "Inputs recorded through level (not a legality claim)",
                "through",
                &h.recorded_through.to_string(),
                "number",
            );
            out.push_str("<button>Update completeness</button></form>");
            let _ = write!(out, "<h3>Level {n}</h3>");
            begin(&mut out, &q.id, rev, n, "class");
            out.push_str("<label>Class<select name=\"class\">");
            for c in character_context::classes(&e) {
                let selected = d
                    .advancement
                    .iter()
                    .find(|f| f.character_level == n)
                    .is_some_and(|f| f.lanes.iter().any(|l| l.class_id == c.id));
                let _ = write!(
                    out,
                    "<option value=\"{}\" {}>{}</option>",
                    escape(&c.id),
                    if selected { "selected" } else { "" },
                    escape(&c.name)
                );
            }
            out.push_str("</select></label>");
            let lane = d
                .advancement
                .iter()
                .find(|f| f.character_level == n)
                .and_then(|f| f.lanes.first());
            super::forms::input(
                &mut out,
                "Hit die roll",
                "hp",
                &lane
                    .and_then(|l| l.hit_points_gained)
                    .unwrap_or(1)
                    .to_string(),
                "number",
            );
            super::forms::input(
                &mut out,
                "Casting track advanced (leave empty if none)",
                "track",
                lane.and_then(|l| l.advances.first())
                    .map(String::as_str)
                    .unwrap_or(""),
                "text",
            );
            out.push_str("<button>Save level</button></form>");
            out.push_str("<ol class=\"acquisitions\">");
            for (i, event) in h
                .events
                .iter()
                .enumerate()
                .filter(|(_, event)| event.level == n)
            {
                let _ = write!(out, "<li>{}", escape(&event.action.label()));
                begin(&mut out, &q.id, rev, n, "remove");
                let _=write!(out,"<input type=\"hidden\" name=\"index\" value=\"{i}\"><button>Remove acquisition</button></form>");
                for (operation, label, neighbor) in [
                    ("earlier", "Move earlier", i.checked_sub(1)),
                    ("later", "Move later", i.checked_add(1)),
                ] {
                    if neighbor
                        .and_then(|j| h.events.get(j))
                        .is_some_and(|e| e.level == n)
                    {
                        begin(&mut out, &q.id, rev, n, operation);
                        let _=write!(out,"<input type=\"hidden\" name=\"index\" value=\"{i}\"><button>{label}</button></form>");
                    }
                }
                out.push_str("</li>");
            }
            out.push_str("</ol>");
            if n <= d.total_level() {
                for (kind, title, fields) in [
                    (
                        "skill",
                        "Add skill ranks",
                        vec![
                            ("Skill", "name", "text"),
                            ("Ranks gained", "amount", "number"),
                        ],
                    ),
                    ("feat", "Acquire feat", vec![("Feat", "name", "text")]),
                    (
                        "retrain",
                        "Retrain feat",
                        vec![("Old feat", "old", "text"), ("New feat", "name", "text")],
                    ),
                    (
                        "spell",
                        "Learn spell",
                        vec![
                            ("Spell", "name", "text"),
                            ("Casting class", "track", "text"),
                            ("Spell level", "amount", "number"),
                        ],
                    ),
                    (
                        "choice",
                        "Record class choice",
                        vec![
                            ("Choice key", "name", "text"),
                            ("Selected value", "value", "text"),
                        ],
                    ),
                    (
                        "evidence",
                        "Record prerequisite evidence",
                        vec![("Evidence statement / rule key", "name", "text")],
                    ),
                    (
                        "ability",
                        "Level-up ability increase",
                        vec![("Ability (STR, DEX, CON, INT, WIS, CHA)", "name", "text")],
                    ),
                ] {
                    let _ = write!(out, "<details><summary>{title}</summary>");
                    begin(&mut out, &q.id, rev, n, kind);
                    for (label, key, typ) in fields {
                        if kind == "skill" && key == "amount" {
                            out.push_str("<label>Ranks gained<input type=\"number\" name=\"amount\" step=\"0.5\" min=\"0.5\" required></label>");
                        } else {
                            super::forms::input(&mut out, label, key, "", typ);
                        }
                    }
                    out.push_str("<button>Add acquisition at this level</button></form></details>");
                }
            }
            out.push_str("</section>");
        } else {
            out.push_str("<section><h2>History missing</h2><p>Current totals are retained until you start a history. Enter original level-1 scores. Starting a history replaces final skill, feat, spell and choice inputs with the acquisitions you record; existing class levels remain. No earlier acquisitions will be inferred.</p>");
            begin(&mut out, &q.id, rev, n, "start");
            for ability in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                super::forms::input(&mut out, ability, ability, "10", "number");
            }
            super::forms::input(
                &mut out,
                "Initial alignment",
                "alignment",
                &d.alignment,
                "text",
            );
            out.push_str("<button>Start explicit acquisition history</button></form></section>");
        }
        out.push_str("<section><h2>Qualification at acquisition</h2>");
        decisions(&mut out, &e.audit_build(d), Some(n));
        out.push_str("</section>");
        Ok(())
    })();
    if let Err(err) = result {
        ui::notice(&mut out, &err, true);
    }
    ui::finish(
        &mut out,
        "Invalid drafts may be saved. Qualification and implementation coverage remain visible.",
    );
    out
}
pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<QueryForm>,
) -> Html<String> {
    Html(render(&state, &q, ""))
}
pub(super) async fn save(
    State(state): State<AppState>,
    Form(f): Form<BTreeMap<String, String>>,
) -> Html<String> {
    let _lock = state.mutation_lock.lock().await;
    let q = QueryForm {
        id: f.get("id").cloned().unwrap_or_default(),
        level: f.get("level").and_then(|x| x.parse().ok()).unwrap_or(1),
    };
    let result = (|| -> Result<(), String> {
        let (mut env, engine) = character_context::load(&state, &q.id)?;
        let d = &mut env.character;
        let get = |key: &str| -> Result<&str, String> {
            f.get(key)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or(format!("Missing {key}"))
        };
        let revision = get("revision")?
            .parse::<u64>()
            .map_err(|_| "Invalid revision")?;
        if revision != d.class_play.revision {
            return Err("Character changed; reload before saving".into());
        }
        match get("op")? {
            "origin" => {
                let mut stats = BTreeMap::new();
                for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                    let score = get(key)?
                        .parse::<i32>()
                        .map_err(|_| "Invalid starting score")?;
                    if !(1..=100).contains(&score) {
                        return Err("Starting score must be 1 through 100".into());
                    }
                    stats.insert(key.into(), score);
                }
                d.name = get("name")?.into();
                let history = d.build_history.as_mut().ok_or("Start a history first")?;
                history.initial_stats = stats;
                history.initial_alignment = get("alignment")?.into();
            }
            "start" => {
                if d.build_history.is_some() {
                    return Err("History already exists".into());
                }
                let mut stats = BTreeMap::new();
                for key in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                    let value = get(key)?
                        .parse::<i32>()
                        .map_err(|_| "Invalid starting score")?;
                    if !(1..=100).contains(&value) {
                        return Err("Starting score must be 1 through 100".into());
                    }
                    stats.insert(key.into(), value);
                }
                d.build_history = Some(BuildHistory {
                    initial_stats: stats,
                    initial_alignment: get("alignment")?.into(),
                    recorded_through: 0,
                    events: vec![],
                });
            }
            "class" => {
                if q.level == 0 || q.level > d.total_level() + 1 {
                    return Err("Add consecutive levels only".into());
                }
                if d.advancement.iter().any(|f| f.lanes.len() != 1) {
                    return Err("This form edits single-lane builds; gestalt history requires a separate lane editor".into());
                }
                let class = get("class")?;
                let def = engine.pack.classes.get(class).ok_or("Unknown class")?;
                let hp = get("hp")?.parse::<i32>().map_err(|_| "Invalid HP roll")?;
                let die = def
                    .hit_die
                    .trim_start_matches('d')
                    .parse::<i32>()
                    .map_err(|_| "Unknown hit die")?;
                if hp < 1 || hp > die {
                    return Err("HP roll is outside the hit die".into());
                }
                let mut frame = pcgen_core::AdvancementFrame::single(q.level, class, 1, hp, 0);
                if let Some(track) = f.get("track").filter(|s| !s.trim().is_empty()) {
                    frame.lanes[0].advances.push(track.trim().into());
                }
                if let Some(old) = d
                    .advancement
                    .iter_mut()
                    .find(|f| f.character_level == q.level)
                {
                    *old = frame;
                } else {
                    d.advancement.push(frame);
                }
                let mut counts = BTreeMap::new();
                for frame in &mut d.advancement {
                    let lane = &mut frame.lanes[0];
                    let count = counts.entry(lane.class_id.clone()).or_insert(0);
                    *count += 1;
                    lane.class_level = *count;
                }
            }
            "completeness" => {
                let n = get("through")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid completeness level")?;
                if n > d.total_level() {
                    return Err("Completeness exceeds recorded levels".into());
                }
                d.build_history
                    .as_mut()
                    .ok_or("Start a history first")?
                    .recorded_through = n;
            }
            "earlier" | "later" => {
                let i = get("index")?
                    .parse::<usize>()
                    .map_err(|_| "Invalid acquisition index")?;
                let neighbor = if get("op")? == "earlier" {
                    i.checked_sub(1)
                } else {
                    i.checked_add(1)
                }
                .ok_or("No neighboring acquisition")?;
                let h = d.build_history.as_mut().ok_or("Start a history first")?;
                if [i, neighbor]
                    .iter()
                    .any(|j| h.events.get(*j).is_none_or(|e| e.level != q.level))
                {
                    return Err("Reorder acquisitions within the same level only".into());
                }
                h.events.swap(i, neighbor);
            }
            "remove" => {
                let i = get("index")?
                    .parse::<usize>()
                    .map_err(|_| "Invalid event index")?;
                let h = d.build_history.as_mut().ok_or("Start a history first")?;
                if h.events.get(i).is_none_or(|e| e.level != q.level) {
                    return Err("Acquisition no longer exists at this level".into());
                }
                h.events.remove(i);
            }
            op => {
                if q.level == 0 || q.level > d.total_level() {
                    return Err("Record the class level before acquisitions".into());
                }
                let name = get("name")?.to_string();
                let action = match op {
                    "skill" => Acquisition::Skill {
                        name,
                        ranks: get("amount")?
                            .parse()
                            .map_err(|_| "Invalid rank increment")?,
                    },
                    "feat" => Acquisition::Feat { name },
                    "retrain" => Acquisition::RetrainFeat {
                        old: get("old")?.into(),
                        new: name,
                    },
                    "spell" => Acquisition::Spell {
                        spell: DraftSpell {
                            name,
                            class_id: Some(get("track")?.into()),
                            level: get("amount")?.parse().map_err(|_| "Invalid spell level")?,
                            times_memorized: 0,
                            book: None,
                        },
                    },
                    "choice" => Acquisition::Choice {
                        key: name,
                        value: get("value")?.into(),
                    },
                    "evidence" => Acquisition::Evidence { statement: name },
                    "ability" => Acquisition::AbilityIncrease { ability: name },
                    _ => return Err("Unknown acquisition action".into()),
                };
                let h = d.build_history.as_mut().ok_or("Start a history first")?;
                let index = h.events.partition_point(|e| e.level <= q.level);
                h.events.insert(
                    index,
                    BuildEvent {
                        level: q.level,
                        action,
                    },
                );
            }
        }
        d.refresh_build_projection()?;
        d.class_play.revision = d
            .class_play
            .revision
            .checked_add(1)
            .ok_or("Revision overflow")?;
        state
            .repository
            .replace_current(&env)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    let message = match result {
        Ok(()) => "Saved acquisition inputs. Review the audit below.".into(),
        Err(e) => format!("Not saved: {e}"),
    };
    Html(render(&state, &q, &message))
}
