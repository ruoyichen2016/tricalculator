//! Read the same compiled class model used by the editor and engine.
use super::ui::{escape, url};
use super::*;
use std::fmt::Write;

#[derive(Default, Deserialize)]
pub(super) struct LibraryQuery {
    #[serde(default)]
    id: String,
    #[serde(default)]
    class: String,
}

pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<LibraryQuery>,
) -> Html<String> {
    let path = state
        .config
        .cohort_root
        .join("data/rules/rainbow-warweaver.rules.json");
    let loaded = if q.id.is_empty() {
        cached_engine(&state, &path)
    } else {
        super::character_context::load(&state, &q.id).map(|(_, e)| e)
    };
    let engine = match loaded {
        Ok(e) => e,
        Err(e) => {
            let mut out = super::ui::start(
                super::ui::Page::Library,
                &q.id,
                "The rule library",
                "Read class progression and sources.",
                &[],
            );
            super::ui::notice(&mut out, &e, true);
            super::ui::finish(&mut out, "Rules unavailable");
            return Html(out);
        }
    };
    let classes = super::character_context::classes(&engine);
    let visible_classes: Vec<_> = classes
        .iter()
        .copied()
        .filter(|c| q.class.is_empty() || c.id.eq_ignore_ascii_case(&q.class))
        .collect();
    let anchors: Vec<_> = visible_classes
        .iter()
        .map(|c| (c.id.to_ascii_lowercase().replace(' ', "-"), c.name.as_str()))
        .collect();
    let links: Vec<_> = anchors.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mut out = super::ui::start(
        super::ui::Page::Library,
        &q.id,
        "The rule library",
        "Core classes, fixed-list casters, and prestige classes. Every level uses the same rules model.",
        &links,
    );
    let id_param = if q.id.is_empty() {
        String::new()
    } else {
        format!("?id={}", url(&q.id))
    };
    let _ = write!(
        out,
        "<section><div class=\"section-heading\"><h2>Class filter &amp; stress cases</h2><div><a href=\"/martial{id_param}\">Run the Shadow Pounce stress case</a> · <a href=\"/incarnum{id_param}\">Run the Warforged incarnum case</a> · <a href=\"/joker{id_param}\">Run the Joker case</a></div></div><nav class=\"sub-nav\" aria-label=\"Filter by class\"><a href=\"/classes{id_param}\" {}>All classes ({})</a>",
        if q.class.is_empty() {
            "aria-current=\"page\""
        } else {
            ""
        },
        classes.len()
    );
    for c in &classes {
        let active = c.id.eq_ignore_ascii_case(&q.class);
        let _ = write!(
            out,
            "<a href=\"/classes?id={}&class={}\" {}>{}</a>",
            url(&q.id),
            url(&c.id),
            if active { "aria-current=\"page\"" } else { "" },
            escape(&c.name)
        );
    }
    out.push_str("</nav></section>");

    for c in visible_classes {
        let e = engine
            .pack
            .entities
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&c.id) && e.metadata.class_magic.is_some())
            .unwrap();
        let m = e.metadata.class_magic.as_ref().unwrap();
        let _ = write!(
            out,
            "<section id=\"{}\"><div class=\"section-heading\"><div><p class=\"eyebrow\">{}</p><h2>{}</h2></div><span class=\"badge\">Max L{} · {}</span></div><p>{} Hit Die · {} skill points per level · {:?} attack progression</p>",
            escape(&c.id.to_ascii_lowercase().replace(' ', "-")),
            if c.is_prestige {
                "PRESTIGE CLASS"
            } else {
                "BASE CLASS"
            },
            escape(&c.name),
            c.max_level,
            escape(&c.hit_die),
            escape(&c.hit_die),
            c.skill_points_per_level,
            c.attack_progression
        );
        if let Some(stat) = &c.spellcasting_ability {
            let _ = write!(
                out,
                "<p>{:?} {:?} · {} · {}</p>",
                c.casting_type,
                c.spell_power,
                escape(stat),
                if m.knows_entire_list {
                    "Automatically knows the complete class list at acquired spell levels"
                } else if m.prepared_entire_list {
                    "Prepares from the complete class list"
                } else if c.requires_spellbook {
                    "Spellbook; recorded preparations"
                } else {
                    "Selected spells known"
                }
            );
        }
        out.push_str("<div class=\"table-scroll\"><table><thead><tr><th>Level</th><th>Class abilities</th><th>Spellcasting</th><th>Martial progression</th><th>Incarnum progression</th></tr></thead><tbody>");
        for level in 1..=c.max_level {
            let _ = write!(out, "<tr><td>{level}</td><td>");
            for f in m.progression.iter().filter(|f| f.level == level) {
                let _ = write!(
                    out,
                    "<p class=\"coverage\">Data: {} · Qualification: {} · Execution: {}</p>",
                    f.coverage.data.label(),
                    f.coverage.qualification.label(),
                    f.coverage.execution.label()
                );
                if !f.coverage.note.is_empty() {
                    let _ = write!(out, "<p>{}</p>", escape(&f.coverage.note));
                }
                let _ = write!(
                    out,
                    "<strong>{}</strong><p>{}</p>",
                    escape(&f.name),
                    escape(&f.summary)
                );
            }
            let casting = if c.spell_slots.contains_key(&level) {
                "Class spell slots"
            } else if c.level_spellcaster_advance.contains_key(&level) {
                "+1 selected casting class"
            } else {
                "—"
            };
            let _ = write!(out, "</td><td>{casting}</td><td>");
            if let Some(martial) = &m.martial {
                if let Some(row) = martial.at_level(level) {
                    let _ = write!(
                        out,
                        "{} known, {} readied, {} stances{}",
                        row.known,
                        row.readied,
                        row.stances,
                        if martial.prestige {
                            " (awards at this level)"
                        } else {
                            " (cumulative)"
                        }
                    );
                    if row.granted > 0 {
                        let _ = write!(out, "; {} randomly granted", row.granted);
                    }
                }
            }
            out.push_str("</td><td>");
            if let Some(incarnum) = &m.incarnum {
                if let Some(row) = incarnum.at_level(level) {
                    let _ = write!(
                        out,
                        "{} essentia; {} soulmelds; {} binds",
                        row.essentia, row.soulmelds, row.binds
                    );
                    if !row.chakras.is_empty() {
                        let _ = write!(
                            out,
                            "; open: {}; Totem capacity +{}",
                            escape(&row.chakras.join(", ")),
                            row.totem_capacity_bonus
                        );
                    }
                }
            }
            out.push_str("</td></tr>");
        }
        out.push_str(
            "</tbody></table></div><details><summary>Source and implementation coverage</summary>",
        );
        let _ = write!(out, "<p>{}</p>", escape(&e.source));
        if let Some(gap) = e.facts.get("NativeRulePending") {
            let _ = write!(out, "<p>{}</p>", escape(gap));
        }
        out.push_str("<p>Progression and access are data. Target-specific actions require their own supported resolver; a feature listed here does not imply every encounter interaction is automated.</p></details></section>");
    }
    super::ui::finish(
        &mut out,
        "Compiled rules shared by the library, editor and calculations",
    );
    Html(out)
}
