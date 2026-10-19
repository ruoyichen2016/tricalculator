//! Acquisition editing through ordinary forms. Audit and projection belong to the engine.
use super::ui::{escape, url};
use super::*;
use axum::Form;
use pcgen_core::{build::*, DraftSpell};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

#[derive(Default, Deserialize)]
pub(super) struct QueryForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    level: u32,
}

pub(super) fn decisions(out: &mut String, a: &pcgen_engine::build::BuildAudit, level: Option<u32>) {
    let tags_html = if a.tags.is_empty() {
        String::new()
    } else {
        format!(
            " · Tags: {}",
            a.tags
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
    let _ = write!(
        out,
        "<p class=\"audit-status\">Build status: <span class=\"decision {:?}\"><strong>{}</strong></span>{}. Derived numbers do not certify legality.</p><div class=\"table-scroll\"><table><thead><tr><th>Level</th><th>Subject</th><th>Tag</th><th>Status</th><th>Evidence</th></tr></thead><tbody>",
        a.status(),
        a.status().label(),
        tags_html
    );
    for d in a
        .decisions
        .iter()
        .filter(|d| level.is_none_or(|n| d.level == n || d.level == 0))
    {
        let decision_tags = if d.tags.is_empty() {
            "—".to_string()
        } else {
            d.tags
                .iter()
                .map(|t| {
                    format!(
                        "<span class=\"badge badge-{}\">{}</span>",
                        t.label().to_lowercase(),
                        escape(t.label())
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        let _ = write!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td><span class=\"decision {:?}\">{}</span></td><td>{}",
            d.level,
            escape(&d.subject),
            decision_tags,
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
    let _ = write!(
        out,
        "<form method=\"post\" action=\"/build\"><input type=\"hidden\" name=\"id\" value=\"{}\"><input type=\"hidden\" name=\"revision\" value=\"{revision}\"><input type=\"hidden\" name=\"level\" value=\"{level}\"><input type=\"hidden\" name=\"op\" value=\"{op}\">",
        escape(id)
    );
}

fn render_create_form(out: &mut String, state: &AppState, open_by_default: bool) {
    let pcc_path = state
        .config
        .cohort_root
        .join("data/rules/rainbow-warweaver.rules.json");
    let engine = cached_engine(state, &pcc_path).ok();
    let _ = write!(
        out,
        "<details {}><summary>Create a new character with acquisition history +</summary><p>Start a character directly with level-1 origin scores and chronological acquisition tracking.</p><form method=\"post\" action=\"/build\"><input type=\"hidden\" name=\"op\" value=\"create\"><input type=\"hidden\" name=\"level\" value=\"1\"><input type=\"hidden\" name=\"revision\" value=\"0\"><div class=\"grid\">",
        if open_by_default { "open" } else { "" }
    );
    super::forms::input(out, "Character ID (slug)", "id", "", "text");
    super::forms::input(out, "Character name", "name", "", "text");
    super::forms::input_list(out, "Race", "race_id", "Human", "text", "create-races");
    super::forms::input_list(
        out,
        "Initial alignment",
        "alignment",
        "NG",
        "text",
        "create-alignments",
    );
    out.push_str("<label>Character type<select name=\"character_type\"><option value=\"PC\" selected>PC (Player Character)</option><option value=\"NPC\">NPC</option></select></label>");
    out.push_str("<label>Level 1 class<select name=\"class\">");
    if let Some(e) = &engine {
        for c in character_context::classes(e) {
            let _ = write!(
                out,
                "<option value=\"{}\">{}</option>",
                escape(&c.id),
                escape(&c.name)
            );
        }
    } else {
        out.push_str("<option value=\"Fighter\">Fighter</option>");
    }
    out.push_str("</select></label>");
    super::forms::input(out, "Level 1 Hit Die roll", "hp", "6", "number");
    for ability in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
        super::forms::input(out, ability, ability, "10", "number");
    }
    out.push_str("</div><button>Create character &amp; start history</button></form>");
    if let Some(e) = &engine {
        let mut races: Vec<&str> = e.pack.races.values().map(|r| r.id.as_str()).collect();
        races.sort_unstable();
        races.dedup();
        super::forms::datalist(out, "create-races", races);
    }
    super::forms::datalist(
        out,
        "create-alignments",
        ["LG", "NG", "CG", "LN", "TN", "CN", "LE", "NE", "CE"],
    );
    out.push_str("</details>");
}

fn render(state: &AppState, q: &QueryForm, message: &str) -> String {
    let anchors: &[(&str, &str)] = if q.id.is_empty() {
        &[]
    } else {
        &[
            ("levels", "Levels"),
            ("history", "Acquisitions"),
            ("audit", "Qualification audit"),
        ]
    };
    let mut out = ui::start(
        ui::Page::Studio,
        &q.id,
        "Build",
        "Record acquisitions in order. Class entry is checked before that level's choices.",
        anchors,
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
                    "<li><a href=\"/build?id={}\">{} · level {}</a></li>",
                    url(&item.id),
                    escape(&item.name),
                    item.total_level
                );
            }
        }
        out.push_str("</ul>");
        render_create_form(&mut out, state, true);
        out.push_str("</section>");
        ui::finish(&mut out, "No history is invented for imported characters");
        return out;
    }

    ui::character_bar(
        &mut out,
        state,
        "/build",
        &q.id,
        &[],
        &[
            (&format!("/?id={}", url(&q.id)), "Character overview"),
            (
                &format!("/spellcasting?id={}", url(&q.id)),
                "Spellcasting tracks",
            ),
            (&format!("/class-play?id={}", url(&q.id)), "Encounter desk"),
            ("/editor", "Flat character studio"),
        ],
    );

    let result = (|| -> Result<(), String> {
        let (env, e) = character_context::load(state, &q.id)?;
        let d = &env.character;
        let audit = e.audit_build(d);
        let rev = d.class_play.revision;
        let n = if q.level == 0 {
            d.total_level().max(1)
        } else {
            q.level
        };
        let _ = write!(
            out,
            "<section id=\"levels\"><div class=\"section-heading\"><h2>{}</h2><span class=\"decision {:?}\">{}</span></div><nav aria-label=\"Build levels\">",
            escape(&d.name),
            audit.status(),
            audit.status().label()
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
            "<a href=\"/build?id={}&level={}\" {}>Add level +</a></nav>",
            url(&q.id),
            d.total_level() + 1,
            if n == d.total_level() + 1 {
                "aria-current=\"step\""
            } else {
                ""
            }
        );
        render_create_form(&mut out, state, false);
        out.push_str("</section>");

        if let Some(h) = &d.build_history {
            out.push_str("<section id=\"history\"><h2>Acquisition history</h2>");
            out.push_str("<details><summary>Edit starting identity and scores</summary>");
            begin(&mut out, &q.id, rev, n, "origin");
            out.push_str("<div class=\"grid\">");
            super::forms::input(&mut out, "Name", "name", &d.name, "text");
            super::forms::input_list(
                &mut out,
                "Initial alignment",
                "alignment",
                &h.initial_alignment,
                "text",
                "build-alignments",
            );
            for ability in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                super::forms::input(
                    &mut out,
                    ability,
                    ability,
                    &h.initial_stats.get(ability).unwrap_or(&10).to_string(),
                    "number",
                );
            }
            out.push_str(
                "</div><button>Save origin and re-evaluate history</button></form></details>",
            );

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
            let mut milestones = Vec::new();
            if n == 1 || n.is_multiple_of(3) {
                milestones.push("General feat slot (levels 1, 3, 6, 9, 12, 15, 18)");
            }
            if n > 0 && n.is_multiple_of(4) {
                milestones.push("+1 ability score increase (every 4 character levels)");
            }
            if n == 1 {
                milestones.push("First-level skill points ×4");
            }
            let _ = write!(
                out,
                "<div class=\"milestone-banner\"><span><strong>Completeness:</strong> recorded through level {} of {}</span>{}</div>",
                h.recorded_through,
                d.total_level(),
                if milestones.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<span><strong>Level {n} milestones:</strong> {}</span>",
                        escape(&milestones.join(" · "))
                    )
                }
            );

            begin(&mut out, &q.id, rev, n, "class");
            out.push_str("<div class=\"grid\"><label>Class<select name=\"class\">");
            for c in character_context::classes(&e) {
                let selected = d
                    .advancement
                    .iter()
                    .find(|f| f.character_level == n)
                    .and_then(|f| f.lanes.first())
                    .is_some_and(|l| l.class_id == c.id);
                let _ = write!(
                    out,
                    "<option value=\"{}\" {}>{} ({}, {} skill pts/lvl)</option>",
                    escape(&c.id),
                    if selected { "selected" } else { "" },
                    escape(&c.name),
                    escape(&c.hit_die),
                    c.skill_points_per_level
                );
            }
            out.push_str("</select></label>");
            let frame_at_n = d.advancement.iter().find(|f| f.character_level == n);
            let lane = frame_at_n.and_then(|f| f.lanes.first());
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
            super::forms::input_list(
                &mut out,
                "Casting track advanced (leave empty if none)",
                "track",
                lane.and_then(|l| l.advances.first())
                    .map(String::as_str)
                    .unwrap_or(""),
                "text",
                "build-tracks",
            );
            out.push_str("</div>");
            let lane2 = frame_at_n.and_then(|f| f.lanes.get(1));
            let has_gestalt = d.advancement.iter().any(|f| f.lanes.len() > 1);
            if has_gestalt {
                let _ = write!(
                    out,
                    "<details open><summary>Secondary gestalt lane (Level {n})</summary><div class=\"grid\"><label>Secondary class<select name=\"lane2_class\"><option value=\"\">None (single lane)</option>"
                );
                for c in character_context::classes(&e) {
                    let selected = lane2.is_some_and(|l| l.class_id == c.id);
                    let _ = write!(
                        out,
                        "<option value=\"{}\" {}>{}</option>",
                        escape(&c.id),
                        if selected { "selected" } else { "" },
                        escape(&c.name)
                    );
                }
                out.push_str("</select></label>");
                super::forms::input(
                    &mut out,
                    "Secondary Hit Die roll",
                    "lane2_hp",
                    &lane2
                        .and_then(|l| l.hit_points_gained)
                        .unwrap_or(1)
                        .to_string(),
                    "number",
                );
                super::forms::input_list(
                    &mut out,
                    "Secondary casting track advanced",
                    "lane2_track",
                    lane2
                        .and_then(|l| l.advances.first())
                        .map(String::as_str)
                        .unwrap_or(""),
                    "text",
                    "build-tracks",
                );
                out.push_str("</div></details>");
            }
            out.push_str("<button>Save level</button></form>");

            out.push_str("<ol class=\"acquisitions\">");
            for (i, event) in h
                .events
                .iter()
                .enumerate()
                .filter(|(_, event)| event.level == n)
            {
                let label = event.action.label();
                out.push_str("<li><div class=\"event-row\"><span>");
                out.push_str(&escape(&label));
                out.push_str("</span><div class=\"event-actions\">");
                for (operation, btn_label, neighbor) in [
                    ("earlier", "Move earlier", i.checked_sub(1)),
                    ("later", "Move later", i.checked_add(1)),
                ] {
                    if neighbor
                        .and_then(|j| h.events.get(j))
                        .is_some_and(|e| e.level == n)
                    {
                        begin(&mut out, &q.id, rev, n, operation);
                        let _ = write!(
                            out,
                            "<input type=\"hidden\" name=\"index\" value=\"{i}\"><button>{btn_label}</button></form>"
                        );
                    }
                }
                begin(&mut out, &q.id, rev, n, "remove");
                let _ = write!(
                    out,
                    "<input type=\"hidden\" name=\"index\" value=\"{i}\"><button>Remove acquisition</button></form></div></div></li>"
                );
            }
            out.push_str("</ol>");

            if n <= d.total_level() {
                for (kind, title, fields) in [
                    (
                        "skill",
                        "Add skill ranks",
                        vec![
                            ("Skill", "name", "text", Some("build-skills")),
                            ("Ranks gained", "amount", "number", None),
                        ],
                    ),
                    (
                        "feat",
                        "Acquire feat",
                        vec![("Feat", "name", "text", Some("build-feats"))],
                    ),
                    (
                        "retrain",
                        "Retrain feat",
                        vec![
                            ("Old feat", "old", "text", Some("build-owned-feats")),
                            ("New feat", "name", "text", Some("build-feats")),
                        ],
                    ),
                    (
                        "spell",
                        "Learn spell",
                        vec![
                            ("Spell", "name", "text", Some("build-spells")),
                            ("Casting class", "track", "text", Some("build-tracks")),
                            ("Spell level", "amount", "number", None),
                        ],
                    ),
                    (
                        "choice",
                        "Record class choice",
                        vec![
                            ("Choice key", "name", "text", Some("build-choices")),
                            ("Selected value", "value", "text", None),
                        ],
                    ),
                    (
                        "evidence",
                        "Record prerequisite evidence",
                        vec![("Evidence statement / rule key", "name", "text", None)],
                    ),
                    (
                        "ability",
                        "Level-up ability increase",
                        vec![(
                            "Ability (STR, DEX, CON, INT, WIS, CHA)",
                            "name",
                            "text",
                            Some("build-abilities"),
                        )],
                    ),
                ] {
                    let _ = write!(out, "<details><summary>{title}</summary>");
                    begin(&mut out, &q.id, rev, n, kind);
                    out.push_str("<div class=\"grid\">");
                    for (label, key, typ, list_id) in fields {
                        if kind == "skill" && key == "amount" {
                            out.push_str("<label>Ranks gained<input type=\"number\" name=\"amount\" step=\"0.5\" min=\"0.5\" required></label>");
                        } else if let Some(list) = list_id {
                            super::forms::input_list(&mut out, label, key, "", typ, list);
                        } else {
                            super::forms::input(&mut out, label, key, "", typ);
                        }
                    }
                    out.push_str(
                        "</div><button>Add acquisition at this level</button></form></details>",
                    );
                }

                // Emit candidate datalists for zero-JS guided input
                let mut skills: Vec<&str> =
                    e.pack.skills.values().map(|s| s.name.as_str()).collect();
                skills.sort_unstable();
                skills.dedup();
                super::forms::datalist(&mut out, "build-skills", skills);

                let mut feats: Vec<&str> = e
                    .pack
                    .abilities
                    .values()
                    .filter(|a| matches!(a.category.as_deref(), Some("FEAT" | "Flaw")))
                    .map(|a| a.name.as_str())
                    .collect();
                feats.sort_unstable();
                feats.dedup();
                super::forms::datalist(&mut out, "build-feats", feats);

                let mut owned_feats: Vec<&str> =
                    h.events
                        .iter()
                        .filter_map(|ev| match &ev.action {
                            Acquisition::Feat { name }
                            | Acquisition::RetrainFeat { new: name, .. } => Some(name.as_str()),
                            _ => None,
                        })
                        .chain(d.abilities.iter().map(|(_, name)| name.as_str()))
                        .collect();
                owned_feats.sort_unstable();
                owned_feats.dedup();
                super::forms::datalist(&mut out, "build-owned-feats", owned_feats);

                let mut spells: Vec<&str> =
                    e.pack.spells.values().map(|s| s.name.as_str()).collect();
                spells.sort_unstable();
                spells.dedup();
                super::forms::datalist(&mut out, "build-spells", spells);

                let mut tracks: Vec<String> =
                    d.class_levels().into_iter().map(|c| c.class_id).collect();
                for c in character_context::classes(&e) {
                    if c.spellcasting_ability.is_some() {
                        tracks.push(c.id.clone());
                    }
                }
                tracks.sort_unstable();
                tracks.dedup();
                super::forms::datalist(&mut out, "build-tracks", tracks.iter().map(String::as_str));

                let mut choice_keys = BTreeSet::new();
                for entity in &e.pack.entities {
                    if let Some(m) = &entity.metadata.class_magic {
                        for rule in &m.choices {
                            choice_keys.insert(rule.key.clone());
                        }
                        if let Some(sc) = &m.school_choice {
                            choice_keys.insert(sc.choice_key.clone());
                        }
                        if let Some(al) = &m.advanced_learning {
                            for lvl in &al.levels {
                                choice_keys
                                    .insert(format!("{}.AdvancedLearning.{lvl}", entity.name));
                            }
                        }
                    }
                }
                choice_keys.insert("Sacred Exorcist.ChosenFoe".to_string());
                super::forms::datalist(
                    &mut out,
                    "build-choices",
                    choice_keys.iter().map(String::as_str),
                );
                super::forms::datalist(
                    &mut out,
                    "build-abilities",
                    ["STR", "DEX", "CON", "INT", "WIS", "CHA"],
                );
                super::forms::datalist(
                    &mut out,
                    "build-alignments",
                    ["LG", "NG", "CG", "LN", "TN", "CN", "LE", "NE", "CE"],
                );
            }
            out.push_str("</section>");
        } else {
            out.push_str("<section id=\"history\"><h2>History missing</h2><p>Current totals are retained until you start a history. Enter original level-1 scores. Starting a history replaces final skill, feat, spell and choice inputs with the acquisitions you record; existing class levels remain. No earlier acquisitions will be inferred.</p>");
            begin(&mut out, &q.id, rev, n, "start");
            out.push_str("<div class=\"grid\">");
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
            out.push_str(
                "</div><button>Start explicit acquisition history</button></form></section>",
            );
        }
        out.push_str("<section id=\"audit\"><h2>Qualification at acquisition</h2>");
        decisions(&mut out, &audit, Some(n));
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
    let get = |key: &str| -> Result<&str, String> {
        f.get(key)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or(format!("Missing {key}"))
    };
    if f.get("op").is_some_and(|op| op == "create") {
        let create_res = (|| -> Result<(), String> {
            let id = get("id")?;
            if !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            {
                return Err(
                    "Use letters, digits, hyphens or underscores for the character ID".into(),
                );
            }
            if state.repository.load(id).is_ok() {
                return Err(format!("Character '{id}' already exists"));
            }
            let pcc_path = std::path::absolute(
                state
                    .config
                    .cohort_root
                    .join("data/rules/rainbow-warweaver.rules.json"),
            )
            .map_err(|e| e.to_string())?;
            let engine = cached_engine(&state, &pcc_path)?;
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
            let alignment = get("alignment")?.to_string();
            let character_type = f
                .get("character_type")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("PC")
                .to_string();
            let mut draft = CharacterDraft {
                id: id.into(),
                name: get("name")?.into(),
                race_id: get("race_id")?.into(),
                alignment: alignment.clone(),
                character_type,
                stats: stats.clone(),
                advancement: vec![pcgen_core::AdvancementFrame::single(1, class, 1, hp, 0)],
                build_history: Some(BuildHistory {
                    initial_stats: stats,
                    initial_alignment: alignment,
                    recorded_through: 1,
                    events: vec![],
                }),
                ..CharacterDraft::default()
            };
            draft.refresh_build_projection()?;
            draft.class_play.revision = 1;
            let pcc_str = pcc_path.to_string_lossy().into_owned();
            let env = NativeSaveEnvelope::new(
                draft,
                engine.pack.header.rules_fingerprint.clone(),
                engine.pack.header.data_ref.clone(),
            )
            .with_pcc(Some(pcc_str));
            state.repository.save(&env).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let message = match create_res {
            Ok(()) => "Created character with explicit acquisition history.".into(),
            Err(e) => format!("Not saved: {e}"),
        };
        return Html(render(&state, &q, &message));
    }

    let result = (|| -> Result<(), String> {
        let (mut env, engine) = character_context::load(&state, &q.id)?;
        let d = &mut env.character;
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
                let lane2_class = f
                    .get("lane2_class")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                if d.advancement.iter().any(|f| f.lanes.len() != 1) && lane2_class.is_none() {
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
                if let Some(c2) = lane2_class {
                    let def2 = engine
                        .pack
                        .classes
                        .get(c2)
                        .ok_or("Unknown secondary class")?;
                    let hp2 = f
                        .get("lane2_hp")
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .unwrap_or(1);
                    let die2 = def2
                        .hit_die
                        .trim_start_matches('d')
                        .parse::<i32>()
                        .map_err(|_| "Unknown secondary hit die")?;
                    if hp2 < 1 || hp2 > die2 {
                        return Err("Secondary HP roll is outside the hit die".into());
                    }
                    let mut lane2 = pcgen_core::AdvancementLane::new("secondary", c2, 1, hp2, 0);
                    if let Some(t2) = f.get("lane2_track").filter(|s| !s.trim().is_empty()) {
                        lane2.advances.push(t2.trim().into());
                    }
                    frame.lanes.push(lane2);
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
                    for lane in &mut frame.lanes {
                        let count = counts.entry(lane.class_id.clone()).or_insert(0);
                        *count += 1;
                        lane.class_level = *count;
                    }
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
