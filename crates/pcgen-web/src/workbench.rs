//! Server-rendered workbench: no scripts, background requests or duplicate rule engine.
use super::ui::{escape, url};
use super::*;
use axum::Form;
use pcgen_core::magic::*;
use std::fmt::Write;

#[derive(Default, Deserialize)]
pub(super) struct PageQuery {
    #[serde(default)]
    id: String,
    #[serde(default)]
    pcc: String,
}
fn page_error(error: String) -> String {
    format!(
        "<section class=\"error\" role=\"alert\"><h2>Unavailable</h2><p>{}</p></section>",
        escape(&error)
    )
}
fn number(value: &str, name: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| format!("{name} must be a nonnegative integer"))
}
fn rule(value: &str) -> Result<SpellRule, String> {
    serde_json::from_value(serde_json::Value::String(value.into()))
        .map_err(|_| format!("Unknown metamagic feat: {value}"))
}
fn field(out: &mut String, label: &str, name: &str, value: &str, kind: &str) {
    let _ = write!(
        out,
        "<label>{}<input name=\"{}\" type=\"{}\" value=\"{}\" {}></label>",
        escape(label),
        name,
        kind,
        escape(value),
        if kind == "number" { "min=\"0\"" } else { "" }
    );
}
fn check(out: &mut String, label: &str, name: &str) {
    let _ = write!(
        out,
        "<label class=\"check\"><input name=\"{name}\" type=\"checkbox\" value=\"yes\">{}</label>",
        escape(label)
    );
}
fn options(out: &mut String, label: &str, name: &str, items: &[(&str, &str)]) {
    let _ = write!(out, "<label>{}<select name=\"{name}\">", escape(label));
    for (value, text) in items {
        let _ = write!(
            out,
            "<option value=\"{}\">{}</option>",
            escape(value),
            escape(text)
        );
    }
    out.push_str("</select></label>");
}
fn render(state: &AppState, query: &PageQuery, result: &str) -> String {
    let mut out = super::ui::start(
        super::ui::Page::Workbench,
        &query.id,
        "Advanced operation tools",
        "Explore a character and preview an action before spending resources.",
        &[
            ("character", "Overview"),
            ("casting", "Spellcasting"),
            ("class-actions", "Class abilities"),
            ("quiescent", "Weaving"),
            ("metamagic", "Metamagic"),
            ("catalog", "Spell catalog"),
            ("rules", "Sources"),
        ],
    );
    out.push_str(result);
    out.push_str("<section id=\"character\"><h2>Character</h2><form method=\"get\" action=\"/workbench\"><label>Saved character<select name=\"id\"><option value=\"\">Choose a character</option>");
    match state.repository.list() {
        Ok(list) => {
            for c in list {
                let _ = write!(
                    out,
                    "<option value=\"{}\" {}>{} (level {})</option>",
                    escape(&c.id),
                    if c.id == query.id { "selected" } else { "" },
                    escape(&c.name),
                    c.total_level
                );
            }
        }
        Err(e) => {
            out.push_str("</select></label></form>");
            out.push_str(&page_error(e.to_string()));
            out.push_str("</section>");
            super::ui::finish(&mut out, "Preview mode · Resources are unchanged");
            return out;
        }
    }
    out.push_str("</select></label><button>Open character</button></form><p class=\"muted\">Preview an action here. To spend resources and apply its results, open Encounter.</p>");
    let selected = if query.id.is_empty() {
        None
    } else {
        Some(super::character_context::load(state, &query.id))
    };
    let default_track = match &selected {
        Some(Ok((env, engine))) => {
            super::character_context::first_casting_track(engine, &env.character)
        }
        _ => String::new(),
    };
    let mut snapshot = None;
    let (engine, pcc) = match &selected {
        Some(Ok((envelope, engine))) => {
            let s = engine.evaluate(&envelope.character);
            for class in envelope.character.class_levels() {
                if let Some(choice) = engine
                    .pack
                    .entities
                    .iter()
                    .find(|e| e.name.eq_ignore_ascii_case(&class.class_id))
                    .and_then(|e| e.metadata.class_magic.as_ref())
                    .and_then(|m| m.school_choice.as_ref())
                    .filter(|c| class.level >= c.at_level)
                {
                    if envelope.character.build_history.is_some() {
                        let _=write!(out,"<p><a href=\"/build?id={}\">Record {} on the Build page</a></p>",url(&query.id),escape(&choice.choice_key));continue;
                    }
                    let _=write!(out,"<form method=\"post\" action=\"/workbench/school\"><input type=\"hidden\" name=\"revision\" value=\"{}\"><input type=\"hidden\" name=\"id\" value=\"{}\"><input type=\"hidden\" name=\"choice_key\" value=\"{}\"><label>Focused Studies: additional prohibited school<select name=\"school\"><option value=\"\">Choose a school</option>",envelope.character.class_play.revision,escape(&query.id),escape(&choice.choice_key));
                    for school in &choice.allowed {
                        let _ = write!(
                            out,
                            "<option value=\"{}\" {}>{}</option>",
                            escape(school),
                            if envelope.character.choices.get(&choice.choice_key) == Some(school) {
                                "selected"
                            } else {
                                ""
                            },
                            escape(school)
                        );
                    }
                    out.push_str("</select></label><button>Save school</button><p class=\"muted\">Replaces this choice in the current character; no backup is created.</p></form>");
                }
            }

            let _ = write!(out, "<h3>{}</h3>", escape(&envelope.character.name));
            if s.is_available() {
                let _=write!(out,"<div class=\"facts\"><p><strong>{}</strong>Level</p><p><strong>{}</strong>HP</p><p><strong>{}</strong>BAB</p></div>",s.total_level,s.hit_points,s.base_attack_bonus);
                let _ = write!(
                    out,
                    "<p>Casting progression: {}</p>",
                    s.caster_levels
                        .iter()
                        .map(|(c, l)| format!("{} {l}", escape(c)))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                if let Some(t) = &s.turning {
                    let _ = write!(
                        out,
                        "<p>Turn undead: level {}, {} attempts per day.</p>",
                        t.level, t.times_per_day
                    );
                }
                let _ = write!(
                    out,
                    "<p>Prohibited schools: {}</p>",
                    escape(
                        &s.prohibited_spells
                            .iter()
                            .filter_map(|p| match &p.target {
                                pcgen_core::ProhibitionTarget::School(s) => Some(s.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                );
            } else {
                out.push_str(&page_error(s.diagnostics.join("; ")));
            }
            if !s.rule_gaps.is_empty() {
                out.push_str("<details><summary>Other rules not yet implemented</summary><ul>");
                for text in s.rule_gaps.values() {
                    let _ = write!(out, "<li>{}</li>", escape(text));
                }
                out.push_str("</ul></details>");
            }
            snapshot = Some(s);
            (
                Some(engine.clone()),
                envelope.pcc.clone().unwrap_or_default(),
            )
        }
        Some(Err(error)) => {
            out.push_str(&page_error(error.clone()));
            (None, String::new())
        }
        None => {
            let pcc = if query.pcc.is_empty() {
                state
                    .config
                    .cohort_root
                    .join("data/rules/rainbow-warweaver.rules.json")
                    .canonicalize()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            } else {
                query.pcc.clone()
            };
            let engine = cached_engine(state, &resolve_path(&state.config.data_root, &pcc));
            match engine {
                Ok(e) => (Some(e), pcc),
                Err(e) => {
                    out.push_str(&page_error(e));
                    (None, pcc)
                }
            }
        }
    };
    out.push_str("</section>");
    if let Some(engine) = &engine {
        if !query.id.is_empty() {
            out.push_str("<section id=\"casting\"><h2>Cast or prove entry</h2><p>For entry, use the character before acquiring the target class.</p><form action=\"/workbench/cast\" method=\"post\">");
            let _ = write!(
                out,
                "<input type=\"hidden\" name=\"id\" value=\"{}\">",
                escape(&query.id)
            );
            let tracks = snapshot
                .as_ref()
                .map(|s| {
                    s.caster_levels
                        .keys()
                        .map(|s| (s.as_str(), s.as_str()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            options(&mut out, "Casting track", "class_id", &tracks);
            out.push_str("<label>Spell<input name=\"spell\" list=\"known-spells\" value=\"Magic Missile\" required></label><datalist id=\"known-spells\">");
            if let Some(s) = &snapshot {
                for spell in s
                    .spell_access
                    .values()
                    .flatten()
                    .filter(|s| s.known && !s.prohibited)
                {
                    let _ = write!(
                        out,
                        "<option value=\"{}\">Level {} ({:?})</option>",
                        escape(&spell.spell),
                        spell.level,
                        spell.power
                    );
                }
            }
            out.push_str("</datalist><div class=\"grid\">");
            field(
                &mut out,
                "Heighten to (blank for none)",
                "heighten_to",
                "",
                "number",
            );
            field(
                &mut out,
                "Entry class (blank for cast preview)",
                "entry_class",
                "",
                "text",
            );
            options(
                &mut out,
                "Sudden feat (also select the feat below)",
                "sudden",
                &[
                    ("", "None"),
                    ("empower", "Empower"),
                    ("enlarge", "Enlarge"),
                    ("widen", "Widen"),
                    ("maximize", "Maximize"),
                    ("extend", "Extend"),
                    ("silent", "Silent"),
                    ("still", "Still"),
                    ("quicken", "Quicken"),
                ],
            );
            field(
                &mut out,
                "Sudden uses spent today",
                "sudden_spent",
                "0",
                "number",
            );
            options(
                &mut out,
                "Instant Metamagic (also select the feat below)",
                "instant",
                &[
                    ("", "None"),
                    ("empower", "Empower"),
                    ("enlarge", "Enlarge"),
                    ("widen", "Widen"),
                    ("maximize", "Maximize"),
                    ("persistent", "Persistent"),
                    ("extend", "Extend"),
                    ("quicken", "Quicken"),
                ],
            );
            field(
                &mut out,
                "Instant uses spent today",
                "instant_spent",
                "0",
                "number",
            );
            out.push_str("</div><fieldset><legend>Metamagic and conditions</legend>");
            for (label, name) in [
                ("Versatile Spellcaster", "versatile"),
                ("Earth Spell", "earth"),
                ("Standing on stone or unworked earth", "ground"),
                (
                    "Accept Earth Spell's virtual-slot interpretation",
                    "virtual_slot",
                ),
                ("Sanctum Spell", "sanctum"),
                ("Inside the sanctum", "inside"),
                ("Persistent Spell", "persistent"),
                ("Extend Spell (after Persistent)", "extend"),
                ("Enlarge Spell", "enlarge"),
                ("Empower Spell", "empower"),
                ("Maximize Spell", "maximize"),
                ("Widen Spell", "widen"),
                ("Quicken Spell", "quicken"),
                ("Still Spell", "still"),
                ("Silent Spell", "silent"),
                ("Divine Metamagic (Persistent Spell)", "dmm"),
                ("Eldritch Tapestry", "tapestry"),
                ("Quiescent Weaving", "quiescent"),
                ("Spellguard defensive personal arcane spell", "spellguard_touch"),
            ] {
                check(&mut out, label, name);
            }
            out.push_str("</fieldset><div class=\"grid\">");
            field(
                &mut out,
                "Turning attempts already spent",
                "turning_spent",
                "0",
                "number",
            );
            field(&mut out, "Quiescent spells stored", "stored", "0", "number");
            field(
                &mut out,
                "Payment slot level already spent",
                "spent_level",
                "1",
                "number",
            );
            field(
                &mut out,
                "Slots already spent at that level",
                "slots_spent",
                "0",
                "number",
            );
            field(
                &mut out,
                "Recipient IDs, comma separated (excluding caster)",
                "allies",
                "",
                "text",
            );
            check(
                &mut out,
                "Include caster in tapestry targets",
                "include_self",
            );
            field(&mut out, "Shared spell choice", "spell_choice", "", "text");
            check(&mut out, "Calculate armor failure", "armor_check");
            options(
                &mut out,
                "Armor",
                "armor",
                &[
                    ("none", "None"),
                    ("light", "Light"),
                    ("medium", "Medium"),
                    ("heavy", "Heavy"),
                ],
            );
            options(
                &mut out,
                "Shield",
                "shield",
                &[
                    ("none", "None"),
                    ("light", "Light"),
                    ("heavy", "Heavy"),
                    ("tower", "Tower"),
                ],
            );
            field(
                &mut out,
                "Armor spell failure percent",
                "armor_failure",
                "0",
                "number",
            );
            field(
                &mut out,
                "Shield spell failure percent",
                "shield_failure",
                "0",
                "number",
            );
            out.push_str("</div><button>Resolve cast or entry</button></form></section>");
            action_forms(&mut out, &query.id, &default_track);
            out.push_str("<section id=\"metamagic\"><h2>Incantatrix metamagic</h2><p>Modify an ally's spell or an existing spell effect. The original spell need not be on your own list. Cooperative and Effect uses are separate daily pools.</p><form method=\"post\" action=\"/workbench/metamagic\">");
            let _ = write!(
                out,
                "<input type=\"hidden\" name=\"id\" value=\"{}\">",
                escape(&query.id)
            );
            options(
                &mut out,
                "Feature",
                "mode",
                &[
                    ("effect", "Metamagic Effect"),
                    ("cooperative", "Cooperative Metamagic"),
                ],
            );
            field(&mut out, "Target spell", "spell", "Divine Power", "text");
            field(
                &mut out,
                "Original caster's spell list",
                "spell_list",
                "Cleric",
                "text",
            );
            options(
                &mut out,
                "Feat to apply",
                "apply",
                &[
                    ("persistent", "Persistent Spell"),
                    ("extend", "Extend Spell"),
                    ("enlarge", "Enlarge Spell"),
                    ("empower", "Empower Spell"),
                    ("maximize", "Maximize Spell"),
                    ("sanctum", "Sanctum Spell"),
                ],
            );
            out.push_str("<fieldset><legend>Metamagic already on the spell</legend>");
            for (label, name) in [
                ("Silent Spell", "silent"),
                ("Still Spell", "still"),
                ("Quicken Spell", "quicken"),
                ("Empower Spell", "empower"),
                ("Maximize Spell", "maximize"),
                ("Persistent Spell", "persistent"),
                ("Extend Spell", "extend"),
                ("Enlarge Spell", "enlarge"),
                ("Sanctum Spell", "sanctum"),
            ] {
                check(&mut out, label, name);
            }
            out.push_str("</fieldset><fieldset><legend>Situation</legend>");
            for (label, name) in [
                ("Adjacent to the caster or effect", "adjacent"),
                ("Within the effect", "within"),
                ("Willing allied caster", "willing"),
                ("Action readied for the ally's casting", "readied"),
                ("An ongoing effect is already in place", "ongoing"),
                ("Threatened", "threatened"),
                ("Distracted", "distracted"),
                ("Inside the sanctum", "inside"),
            ] {
                check(&mut out, label, name);
            }
            out.push_str("</fieldset><div class=\"grid\">");
            field(
                &mut out,
                "D20 roll (blank to take 10)",
                "roll",
                "",
                "number",
            );
            field(
                &mut out,
                "Uses of this feature already spent",
                "spent",
                "0",
                "number",
            );
            out.push_str("</div><button>Resolve metamagic attempt</button></form></section>");
        }
        out.push_str("<section id=\"catalog\"><h2>Spell catalog</h2><p>Use your browser's Find command. The full catalog below is already in this page.</p>");
        if let Some(s) = &snapshot {
            if s.is_available() {
                for (track, spells) in &s.spell_access {
                    let _=write!(out,"<details><summary>{}: {} spells</summary><table><thead><tr><th>Spell</th><th>Level</th><th>Power</th><th>Status</th></tr></thead><tbody>",escape(track),spells.len());
                    for spell in spells {
                        let _ = write!(
                            out,
                            "<tr><td>{}</td><td>{}</td><td>{:?}</td><td>{}</td></tr>",
                            escape(&spell.spell),
                            spell.level,
                            spell.power,
                            if spell.prohibited {
                                "Prohibited"
                            } else if spell.known {
                                "Known"
                            } else {
                                "Not yet known"
                            }
                        );
                    }
                    out.push_str("</tbody></table></details>");
                }
            } else {
                out.push_str("<p>Character spell access is unavailable until the reported errors are resolved.</p>");
            }
        } else {
            let _=write!(out,"<details><summary>All loaded spells ({})</summary><table><thead><tr><th>Spell</th><th>School</th><th>Class levels</th></tr></thead><tbody>",engine.pack.spells.len());
            for spell in engine.pack.spells.values() {
                let _ = write!(
                    out,
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape(&spell.name),
                    escape(&spell.school),
                    escape(
                        &spell
                            .class_levels
                            .iter()
                            .map(|(c, l)| format!("{c} {l}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                );
            }
            out.push_str("</tbody></table></details>");
        }
        out.push_str("</section><section id=\"rules\"><h2>Rules and sources</h2>");
        let _ = write!(
            out,
            "<p><a href=\"/editor?pcc={}\">Open character editor with this rule database</a></p>",
            url(&pcc)
        );
        let path = resolve_path(&state.config.data_root, &pcc);
        if path.extension().is_some_and(|e| e == "json") {
            match pcgen_import::native::read_database(&path) {
                Ok(db) => {
                    out.push_str("<details><summary>Source references</summary><ul>");
                    for source in db.sources.values() {
                        let title = if source.reference.starts_with("https://") {
                            format!(
                                "<a href=\"{}\">{}</a>",
                                escape(&source.reference),
                                escape(&source.title)
                            )
                        } else {
                            escape(&source.title)
                        };
                        let _ = write!(out, "<li>{title}: {}</li>", escape(&source.locator));
                    }
                    out.push_str(
                        "</ul></details><details><summary>Native class features</summary>",
                    );
                    for e in &engine.pack.entities {
                        if let Some(m) = &e.metadata.class_magic {
                            let _ = write!(out, "<h3>{}</h3>", escape(&e.name));
                            if let Some(choice) = &m.school_choice {
                                let _ = write!(
                                    out,
                                    "<p>At level {}, prohibit one additional school: {}.</p>",
                                    choice.at_level,
                                    escape(&choice.allowed.join(", "))
                                );
                            }
                            for f in &m.metamagic_features {
                                let _=write!(out,"<p>{:?}, level {}: Spellcraft DC {} + {} * modified spell level. Daily uses: {} + {} modifier. {}.</p>",f.mode,f.at_level,f.dc_base,f.dc_per_spell_level,f.uses_base,escape(&f.uses_ability),escape(&f.action));
                            }
                        }
                    }
                    out.push_str("</details>");
                }
                Err(e) => out.push_str(&page_error(e)),
            }
        } else {
            out.push_str("<p>This character uses an imported PCC rule pack.</p>");
        }
        out.push_str("</section>");
    }
    super::ui::finish(&mut out, "Preview mode · Resources are unchanged");
    out
}
pub(super) async fn page(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Html<String> {
    let html = tokio::task::spawn_blocking(move || render(&state, &query, ""))
        .await
        .unwrap_or_else(|e| page_error(e.to_string()));
    Html(html)
}

#[derive(Default, Deserialize, Serialize)]
pub(super) struct CastForm {
    spellguard_touch: Option<String>,
    id: String,
    class_id: String,
    spell: String,
    heighten_to: String,
    entry_class: String,
    turning_spent: String,
    stored: String,
    spent_level: String,
    slots_spent: String,
    versatile: Option<String>,
    earth: Option<String>,
    ground: Option<String>,
    virtual_slot: Option<String>,
    sanctum: Option<String>,
    inside: Option<String>,
    persistent: Option<String>,
    extend: Option<String>,
    enlarge: Option<String>,
    dmm: Option<String>,
    empower: Option<String>,
    maximize: Option<String>,
    widen: Option<String>,
    quicken: Option<String>,
    #[serde(default)]
    sudden: String,
    #[serde(default)]
    sudden_spent: String,
    #[serde(default)]
    instant: String,
    #[serde(default)]
    instant_spent: String,
    still: Option<String>,
    silent: Option<String>,
    tapestry: Option<String>,
    quiescent: Option<String>,
    #[serde(default)]
    allies: String,
    include_self: Option<String>,
    #[serde(default)]
    spell_choice: String,
    armor_check: Option<String>,
    #[serde(default)]
    armor: String,
    #[serde(default)]
    shield: String,
    #[serde(default)]
    armor_failure: String,
    #[serde(default)]
    shield_failure: String,
}
fn resolve_cast(state: &AppState, f: &CastForm) -> Result<String, String> {
    let (e, engine) = super::character_context::load(state, &f.id)?;
    let cast = CastRequest {
        spellguard_touch: f.spellguard_touch.is_some(),
        armor: if f.armor_check.is_some() {
            Some(ArmorContext {
                armor: serde_json::from_value(serde_json::Value::String(f.armor.clone()))
                    .map_err(|_| "Unknown armor weight")?,
                shield: serde_json::from_value(serde_json::Value::String(f.shield.clone()))
                    .map_err(|_| "Unknown shield weight")?,
                armor_failure: number(&f.armor_failure, "Armor failure")?,
                shield_failure: number(&f.shield_failure, "Shield failure")?,
            })
        } else {
            None
        },
        tapestry_targets: if f.tapestry.is_some() {
            Some(targets(
                &f.allies,
                f.include_self.is_some(),
                &f.spell_choice,
            )?)
        } else {
            None
        },
        class_id: f.class_id.clone(),
        spell: f.spell.clone(),
        heighten_to: if f.heighten_to.is_empty() {
            None
        } else {
            Some(number(&f.heighten_to, "Heighten level")?)
        },
        sudden: if f.sudden.is_empty() {
            vec![]
        } else {
            vec![rule(&f.sudden)?]
        },
        sudden_spent: if f.sudden.is_empty() {
            Default::default()
        } else {
            std::collections::BTreeMap::from([(
                f.sudden.clone(),
                number(&f.sudden_spent, "Sudden uses spent")?,
            )])
        },
        instant_metamagic: if f.instant.is_empty() {
            None
        } else {
            Some(rule(&f.instant)?)
        },
        instant_spent: if f.instant_spent.is_empty() {
            0
        } else {
            number(&f.instant_spent, "Instant uses spent")?
        },
        metamagic: [
            (f.sanctum.is_some(), SpellRule::Sanctum),
            (f.persistent.is_some(), SpellRule::Persistent),
            (f.extend.is_some(), SpellRule::Extend),
            (f.enlarge.is_some(), SpellRule::Enlarge),
            (f.empower.is_some(), SpellRule::Empower),
            (f.maximize.is_some(), SpellRule::Maximize),
            (f.widen.is_some(), SpellRule::Widen),
            (f.quicken.is_some(), SpellRule::Quicken),
            (f.still.is_some(), SpellRule::Still),
            (f.silent.is_some(), SpellRule::Silent),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, r)| r)
        .collect(),
        versatile: f.versatile.is_some(),
        earth: f.earth.is_some(),
        on_stone_or_unworked_earth: f.ground.is_some(),
        earth_uses_virtual_slot: f.virtual_slot.is_some(),
        inside_sanctum: f.inside.is_some(),
        divine_metamagic: f.dmm.as_ref().map(|_| SpellRule::Persistent),
        turning_spent: number(&f.turning_spent, "Turning attempts")?,
        stored_spells: number(&f.stored, "Stored spells")?,
        slots_spent: std::collections::BTreeMap::from([(
            number(&f.spent_level, "Payment level")?,
            number(&f.slots_spent, "Spent slots")?,
        )]),
        tapestry: f.tapestry.is_some(),
        quiescent: f.quiescent.is_some(),
        ..Default::default()
    };
    let mut out = String::from("<section id=\"result\" class=\"result\"><h2>Resolution</h2>");
    let result = if f.entry_class.trim().is_empty() {
        engine.cast_preview(&e.character, &cast)?
    } else {
        let entry = engine.class_entry(&e.character, &f.entry_class, Some(&cast))?;
        let _ = write!(
            out,
            "<p><strong>{}: {}</strong>{}</p><ul>",
            escape(&entry.class_id),
            if entry.qualified {
                "RAW entry conditions met"
            } else if entry.requirements_met && entry.conditional {
                "Conditional entry; table ruling required"
            } else {
                "Entry conditions not met"
            },
            if entry.conditional {
                " (conditional on the stated interpretation)"
            } else {
                ""
            }
        );
        for c in entry.checks {
            let _ = write!(
                out,
                "<li>{}: {}</li>",
                if c.satisfied == Some(true) {
                    "Met"
                } else {
                    "Not met or unavailable"
                },
                escape(&c.requirement)
            );
        }
        out.push_str("</ul>");
        entry.casting_proof.ok_or("No casting proof")?
    };
    let _=write!(out,"<h3>{}</h3><p>{:?}; base level {}, effective level {}. Pay {} level-{} slot(s). Caster level {}; save DC {}.</p><p>Turning cost: {}; remaining: {}. Duration: {}. Range: {:?}, multiplier {}.</p><p>{}</p>",escape(&result.spell),result.power,result.base_level,result.effective_level,result.paid_slots,result.paid_slot_level,result.caster_level,result.save_dc,result.turning_cost,result.turning_remaining,escape(result.duration.as_deref().unwrap_or("Unspecified")),result.range,result.range_multiplier,escape(&result.casting_time_note));
    let _ = write!(
        out,
        "<p>Applied metamagic: {}.</p>",
        escape(
            &result
                .applied_metamagic
                .iter()
                .map(|r| format!("{r:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    );
    let _=write!(out,"<p>Warmage Edge: {}. Apply once per casting (one chosen missile), to each affected creature for an area spell, and once each damaging round. This is the class bonus, not a total damage roll.</p><p>Armor failure: {}. Material cost: {} gp; XP cost: {}.</p><p>{}</p>",result.warmage_edge.map(|n|n.to_string()).unwrap_or("Unavailable: damage applicability not established".into()),result.arcane_failure_percent.map(|n|format!("{n}%")).unwrap_or("Not requested".into()),result.material_cost_gp.map(|n|n.to_string()).unwrap_or("Unavailable".into()),result.xp_cost.map(|n|n.to_string()).unwrap_or("Unavailable".into()),escape(&result.effect_timing));
    if let Some(feet) = result.range_feet {
        let _ = write!(out, "<p>Resolved range: {feet} feet.</p>");
    }
    if let Some(damage) = &result.damage {
        out.push_str("<h3>Damage range</h3><ol>");
        for packet in &damage.packets {
            let _ = write!(
                out,
                "<li>{}d{} + {} + Edge {}: {} to {} damage{}</li>",
                packet.dice,
                packet.die_sides,
                packet.flat,
                packet.edge,
                packet.minimum,
                packet.maximum,
                if damage.area {
                    " per affected creature"
                } else {
                    ""
                }
            );
        }
        let _ = write!(out, "</ol><p>{}</p>", escape(&damage.note));
    }
    for note in result.adjudications {
        let _ = write!(out, "<p>{}</p>", escape(&note));
    }
    out.push_str("</section>");
    Ok(out)
}
pub(super) async fn cast(
    State(state): State<AppState>,
    Form(form): Form<CastForm>,
) -> Html<String> {
    Html(
        tokio::task::spawn_blocking(move || {
            let result = resolve_cast(&state, &form).unwrap_or_else(page_error);
            let values = serde_json::to_value(&form).expect("form contains only strings");
            restore_form(
                render(
                    &state,
                    &PageQuery {
                        id: form.id,
                        pcc: String::new(),
                    },
                    &result,
                ),
                "/workbench/cast",
                &values,
            )
        })
        .await
        .unwrap_or_else(|e| page_error(e.to_string())),
    )
}
#[derive(Deserialize, Serialize)]
pub(super) struct MetaForm {
    id: String,
    mode: String,
    spell: String,
    spell_list: String,
    apply: String,
    roll: String,
    spent: String,
    silent: Option<String>,
    still: Option<String>,
    quicken: Option<String>,
    empower: Option<String>,
    maximize: Option<String>,
    persistent: Option<String>,
    extend: Option<String>,
    enlarge: Option<String>,
    sanctum: Option<String>,
    adjacent: Option<String>,
    within: Option<String>,
    willing: Option<String>,
    readied: Option<String>,
    ongoing: Option<String>,
    threatened: Option<String>,
    distracted: Option<String>,
    inside: Option<String>,
}
fn resolve_meta(state: &AppState, f: &MetaForm) -> Result<String, String> {
    let (e, engine) = super::character_context::load(state, &f.id)?;
    let request = MetamagicRequest {
        mode: match f.mode.as_str() {
            "effect" => MetamagicMode::Effect,
            "cooperative" => MetamagicMode::Cooperative,
            _ => return Err("Unknown feature".into()),
        },
        spell: f.spell.clone(),
        spell_list: f.spell_list.clone(),
        existing: [
            (&f.silent, SpellRule::Silent),
            (&f.still, SpellRule::Still),
            (&f.quicken, SpellRule::Quicken),
            (&f.empower, SpellRule::Empower),
            (&f.maximize, SpellRule::Maximize),
            (&f.persistent, SpellRule::Persistent),
            (&f.extend, SpellRule::Extend),
            (&f.enlarge, SpellRule::Enlarge),
            (&f.sanctum, SpellRule::Sanctum),
        ]
        .into_iter()
        .filter(|(on, _)| on.is_some())
        .map(|(_, r)| r)
        .collect(),
        apply: rule(&f.apply)?,
        inside_sanctum: f.inside.is_some(),
        roll: if f.roll.is_empty() {
            None
        } else {
            Some(number(&f.roll, "D20 roll")?)
        },
        spent: number(&f.spent, "Uses spent")?,
        adjacent: f.adjacent.is_some(),
        within_effect: f.within.is_some(),
        willing_ally: f.willing.is_some(),
        readied_action: f.readied.is_some(),
        ongoing_effect: f.ongoing.is_some(),
        threatened: f.threatened.is_some(),
        distracted: f.distracted.is_some(),
    };
    let r = engine.metamagic_preview(&e.character, &request)?;
    Ok(format!("<section id=\"result\" class=\"result\"><h2>{:?}: {}</h2><p>{}: modified spell level {}, Spellcraft DC {}.</p><p>{} {} + modifier {} = {}. A natural 20 is not an automatic skill success.</p><p>Uses: {} per day, {} spent by this attempt, {} remaining. Additional spell slots: {}.</p><p>Action: {}; provokes an attack of opportunity.</p><p>Duration: {}. Applied metamagic: {}.</p><p class=\"muted\">{}</p></section>",r.mode,if r.success {"Success"}else{"Check failed"},escape(&r.spell),r.modified_spell_level,r.spellcraft_dc,if r.take_ten{"Take"}else{"Roll"},r.die_result,r.spellcraft_modifier,r.check_total,r.uses_per_day,r.uses_spent_by_attempt,r.uses_remaining_after_attempt,r.additional_spell_slots,escape(&r.action),escape(r.resulting_duration.as_deref().unwrap_or("Unspecified")),escape(&r.applied.iter().map(|f|format!("{f:?}")).collect::<Vec<_>>().join(", ")),escape(&r.source)))
}
pub(super) async fn metamagic(
    State(state): State<AppState>,
    Form(form): Form<MetaForm>,
) -> Html<String> {
    Html(
        tokio::task::spawn_blocking(move || {
            let result = resolve_meta(&state, &form).unwrap_or_else(page_error);
            let values = serde_json::to_value(&form).expect("form contains only strings");
            restore_form(
                render(
                    &state,
                    &PageQuery {
                        id: form.id,
                        pcc: String::new(),
                    },
                    &result,
                ),
                "/workbench/metamagic",
                &values,
            )
        })
        .await
        .unwrap_or_else(|e| page_error(e.to_string())),
    )
}

#[derive(Deserialize)]
pub(super) struct SchoolForm {
    revision: u64,
    id: String,
    choice_key: String,
    school: String,
}
pub(super) async fn school(
    State(state): State<AppState>,
    Form(form): Form<SchoolForm>,
) -> Html<String> {
    let lock=state.mutation_lock.clone();
    let _guard=lock.lock().await;
    Html(tokio::task::spawn_blocking(move||{
        let result=(||->Result<String,String>{
            let (mut envelope,engine)=super::character_context::load(&state,&form.id)?;
            if envelope.character.build_history.is_some() { return Err("Record this class choice on the Build page at its acquisition level".into()); }
            if envelope.character.class_play.revision!=form.revision {return Err("Character changed; reload before saving".into());}
            envelope.character.class_play.revision=envelope.character.class_play.revision.checked_add(1).ok_or("Revision overflow")?;
            let choice=envelope.character.class_levels().iter().find_map(|level|engine.pack.entities.iter().find(|e|e.name.eq_ignore_ascii_case(&level.class_id)).and_then(|e|e.metadata.class_magic.as_ref()).and_then(|m|m.school_choice.as_ref()).filter(|c|level.level>=c.at_level && c.choice_key==form.choice_key)).ok_or("This school choice is not available")?;
            let school=choice.allowed.iter().find(|s|s.eq_ignore_ascii_case(&form.school)).ok_or("Choose one of the permitted schools")?.clone();
            envelope.character.choices.insert(choice.choice_key.clone(),school.clone());
            let snapshot=engine.evaluate(&envelope.character);
            if !snapshot.is_available() { return Err(snapshot.diagnostics.join("; ")); }
            envelope.rules_fingerprint=engine.pack.header.rules_fingerprint.clone();
            state.repository.replace_current(&envelope).map_err(|e|e.to_string())?;
            Ok(format!("<section class=\"result\"><h2>School saved</h2><p>{} is now prohibited.</p></section>",escape(&school)))
        })().unwrap_or_else(page_error);
        render(&state,&PageQuery{id:form.id,pcc:String::new()},&result)
    }).await.unwrap_or_else(|e|page_error(e.to_string())))
}

/// Restore only known form controls; every value is HTML-escaped before insertion.
pub(super) fn restore_form(mut html: String, action: &str, values: &serde_json::Value) -> String {
    let Some(action_at) = html.find(&format!("action=\"{action}\"")) else {
        return html;
    };
    let Some(end_relative) = html[action_at..].find("</form>") else {
        return html;
    };
    let end = action_at + end_relative;
    let mut form = html[action_at..end].to_string();
    if let Some(values) = values.as_object() {
        for (name, value) in values {
            let marker = format!("name=\"{name}\"");
            let Some(at) = form.find(&marker) else {
                continue;
            };
            let Some(close) = form[at..].find('>') else {
                continue;
            };
            let end_tag = at + close;
            let tag = &form[at..end_tag];
            if tag.contains("type=\"checkbox\"") {
                if !value.is_null() {
                    form.insert_str(end_tag, " checked");
                }
            } else if let Some(value) = value.as_str() {
                if form[..at].ends_with("<textarea ") {
                    if let Some(end_text) = form[end_tag + 1..].find("</textarea>") {
                        form.replace_range(end_tag + 1..end_tag + 1 + end_text, &escape(value));
                    }
                    continue;
                }
                if let Some(start) = tag.find("value=\"") {
                    let from = at + start + 7;
                    if let Some(len) = form[from..end_tag].find('"') {
                        form.replace_range(from..from + len, &escape(value));
                    }
                } else if form[..at].ends_with("<select ") {
                    if let Some(end_select) = form[end_tag..].find("</select>") {
                        let end_select = end_tag + end_select;
                        let option = format!("<option value=\"{}\">", escape(value));
                        let selected = option.replace('>', " selected>");
                        let choices = form[end_tag..end_select]
                            .replace(" selected", "")
                            .replace(&option, &selected)
                            .replace(
                                &format!("<option>{}", escape(value)),
                                &format!("<option selected>{}", escape(value)),
                            );
                        form.replace_range(end_tag..end_select, &choices);
                    }
                }
            }
        }
    }
    html.replace_range(action_at..end, &form);
    html
}

fn targets(ids: &str, include_self: bool, choice: &str) -> Result<TapestryTargets, String> {
    let mut recipients: Vec<_> = ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if include_self {
        recipients.push("self".into());
    }
    Ok(TapestryTargets {
        recipients,
        choice: (!choice.trim().is_empty()).then(|| choice.trim().into()),
    })
}

fn action_forms(out: &mut String, id: &str, track: &str) {
    out.push_str("<section id=\"class-actions\"><h2>Class abilities</h2><p>Situational previews use actual class levels. Spell metamagic does not modify supernatural or spell-like abilities.</p><form method=\"post\" action=\"/workbench/action\">");
    let _ = write!(
        out,
        "<input type=\"hidden\" name=\"id\" value=\"{}\">",
        escape(id)
    );
    options(
        out,
        "Ability",
        "ability",
        &[
            ("wings", "Grow Wings"),
            ("exorcism", "Exorcism"),
            ("detect", "Detection"),
            ("weave", "Create Eldritch Tapestry"),
            ("sacred_spell", "Sacred Exorcist spell-like ability"),
        ],
    );
    out.push_str("<div class=\"grid\">");
    field(out, "Spent uses (wings: seconds)", "spent", "0", "number");
    field(out, "Wing seconds to use", "duration", "60", "number");
    check(out, "Wearing armor or restrictive clothing", "restrictive");
    check(out, "Grappling", "grappling");
    field(
        out,
        "D20 roll (blank: take 10 for Craft only)",
        "roll",
        "",
        "number",
    );
    field(out, "Possessor hit dice", "hd", "1", "number");
    field(out, "Possessor Charisma score", "charisma", "10", "number");
    options(
        out,
        "Detection spell",
        "spell",
        &[
            ("Detect Evil", "Detect Evil"),
            ("Detect Chaos", "Detect Chaos"),
            ("Detect Thoughts", "Detect Thoughts"),
            ("Dispel Evil", "Dispel Evil"),
            ("Holy Aura", "Holy Aura"),
        ],
    );
    field(
        out,
        "Arcane casting track for tapestry",
        "track",
        track,
        "text",
    );
    field(
        out,
        "Member IDs, comma separated (caster excluded)",
        "allies",
        "",
        "text",
    );

    check(out, "Threatened", "threatened");
    check(out, "Distracted", "distracted");
    out.push_str("</div><button>Resolve class ability</button></form></section>");
    out.push_str("<section id=\"quiescent\"><h2>Quiescent Weaving</h2><p>Plan a fresh set of stored spells and its release. Enter one spell per line in casting order. All spells release in one move action; target conditions refer to that moment. Existing stored spells are not included in this plan.</p><form method=\"post\" action=\"/workbench/quiescent\">");
    let _ = write!(
        out,
        "<input type=\"hidden\" name=\"id\" value=\"{}\">",
        escape(id)
    );
    field(out, "Casting track", "track", track, "text");
    out.push_str("<label>Spells in casting order<textarea name=\"spells\" rows=\"4\">Bull's Strength\nShield of Faith</textarea></label>");
    check(
        out,
        "Apply Sanctum Spell to every spell (outside sanctum)",
        "sanctum",
    );
    field(
        out,
        "Recipient IDs (excluding caster)",
        "allies",
        "",
        "text",
    );
    check(out, "Include caster", "include_self");
    field(out, "Shared spell choice", "choice", "", "text");
    out.push_str("<p>Slot availability starts from the daily allotment for this plan. Preparation costs are accumulated; release costs no extra slots.</p><button>Resolve storage and release</button></form></section>");
}
#[derive(Deserialize, Serialize)]
pub(super) struct ActionForm {
    id: String,
    ability: String,
    spent: String,
    duration: String,
    roll: String,
    hd: String,
    charisma: String,
    spell: String,
    track: String,
    allies: String,
    restrictive: Option<String>,
    grappling: Option<String>,
    #[serde(default)]
    choice: String,
    threatened: Option<String>,
    distracted: Option<String>,
}
fn resolve_action(state: &AppState, f: &ActionForm) -> Result<String, String> {
    let (e, engine) = super::character_context::load(state, &f.id)?;
    let request = match f.ability.as_str() {
        "wings" => ClassActionRequest::GrowWings {
            spent_seconds: number(&f.spent, "Spent seconds")?,
            use_seconds: number(&f.duration, "Use seconds")?,
            armor_or_restrictive_clothing: f.restrictive.is_some(),
            grappling: f.grappling.is_some(),
        },
        "exorcism" => ClassActionRequest::Exorcism {
            roll: number(&f.roll, "Exorcism d20")?,
            possessor_hit_dice: number(&f.hd, "Possessor HD")?,
            possessor_charisma: f
                .charisma
                .parse()
                .map_err(|_| "Charisma must be an integer")?,
        },
        "sacred_spell" => ClassActionRequest::SacredSpell {
            spell: f.spell.clone(),
            spent: number(&f.spent, "Uses spent")?,
        },
        "detect" => ClassActionRequest::Detect {
            spell: f.spell.clone(),
        },
        "weave" => ClassActionRequest::WeaveTapestry {
            class_id: f.track.clone(),
            roll: if f.roll.is_empty() {
                None
            } else {
                Some(number(&f.roll, "Craft d20")?)
            },
            threatened: f.threatened.is_some(),
            distracted: f.distracted.is_some(),
            members: targets(&f.allies, false, "")?.recipients,
        },
        _ => return Err("Unknown class ability".into()),
    };
    let r = engine.class_action(&e.character, &request)?;
    let mut out = format!(
        "<section id=\"result\" class=\"result\"><h2>{}: {}</h2><p>{}</p><ul>",
        escape(&r.feature),
        if r.success { "Success" } else { "Check failed" },
        escape(&r.action)
    );
    for fact in r.facts {
        let _ = write!(out, "<li>{}</li>", escape(&fact));
    }
    out.push_str("</ul></section>");
    Ok(out)
}
pub(super) async fn action(
    State(state): State<AppState>,
    Form(form): Form<ActionForm>,
) -> Html<String> {
    Html(
        tokio::task::spawn_blocking(move || {
            let result = resolve_action(&state, &form).unwrap_or_else(page_error);
            let values = serde_json::to_value(&form).expect("String form");
            restore_form(
                render(
                    &state,
                    &PageQuery {
                        id: form.id,
                        pcc: String::new(),
                    },
                    &result,
                ),
                "/workbench/action",
                &values,
            )
        })
        .await
        .unwrap_or_else(|e| page_error(e.to_string())),
    )
}
#[derive(Deserialize, Serialize)]
pub(super) struct QuiescentForm {
    id: String,
    track: String,
    spells: String,
    allies: String,
    include_self: Option<String>,
    #[serde(default)]
    choice: String,
    sanctum: Option<String>,
}
fn resolve_quiescent(state: &AppState, f: &QuiescentForm) -> Result<String, String> {
    let (e, engine) = super::character_context::load(state, &f.id)?;
    let target = targets(&f.allies, f.include_self.is_some(), &f.choice)?;
    let requests: Vec<_> = f
        .spells
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| CastRequest {
            class_id: f.track.clone(),
            spell: s.into(),
            tapestry: true,
            quiescent: true,
            tapestry_targets: Some(target.clone()),
            metamagic: if f.sanctum.is_some() {
                vec![SpellRule::Sanctum]
            } else {
                vec![]
            },
            ..Default::default()
        })
        .collect();
    let r = engine.quiescent_preview(&e.character, &requests)?;
    let mut out = format!(
        "<section id=\"result\" class=\"result\"><h2>Quiescent release</h2><p>{}</p><ol>",
        escape(&r.release_action)
    );
    for spell in r.release_order {
        let _=write!(out,"<li>{}: effective level {}; {:?} range; duration {}; preparation uses {} level-{} slots, {} gp, {} XP.</li>",escape(&spell.spell),spell.effective_level,spell.range,escape(spell.duration.as_deref().unwrap_or("Unavailable")),spell.paid_slots,spell.paid_slot_level,spell.material_cost_gp.ok_or("Material cost unavailable")?,spell.xp_cost.ok_or("XP cost unavailable")?);
    }
    out.push_str("</ol></section>");
    Ok(out)
}
pub(super) async fn quiescent(
    State(state): State<AppState>,
    Form(form): Form<QuiescentForm>,
) -> Html<String> {
    Html(
        tokio::task::spawn_blocking(move || {
            let result = resolve_quiescent(&state, &form).unwrap_or_else(page_error);
            let values = serde_json::to_value(&form).expect("String form");
            restore_form(
                render(
                    &state,
                    &PageQuery {
                        id: form.id,
                        pcc: String::new(),
                    },
                    &result,
                ),
                "/workbench/quiescent",
                &values,
            )
        })
        .await
        .unwrap_or_else(|e| page_error(e.to_string())),
    )
}
