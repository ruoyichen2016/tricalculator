//! Read-only stress replay: ordinary English forms, shared engine rules, no encounter writes.
use super::ui::escape;
use super::*;
use axum::Form;
use pcgen_engine::martial_case::{MartialCase, MartialCaseReport};
use std::{collections::BTreeMap, fmt::Write};
const CASE: &str = include_str!("../../../tests/fixtures/forum-builds/shadow-pounce.json");
fn render(state: &AppState, id: &str, f: &BTreeMap<String, String>) -> String {
    let mut out = super::ui::start(
        super::ui::Page::Cases,
        id,
        "Martial stress replay",
        "Full class progression first; then audit the proposed combination.",
        &[],
    );
    let _ = write!(
        out,
        "<section><h2>Shadow pounce / Ruby Knight Vindicator</h2><p>This is a read-only case study, not a saved character or a certified legal build. <a href=\"/cases?case=shadow-pounce&id={}\">Back to case overview</a> · <a href=\"/classes?id={}\">Read the full class progressions</a>.</p><form method=post action=\"/martial\"><input type=\"hidden\" name=\"id\" value=\"{}\">",
        super::ui::url(id),
        super::ui::url(id),
        escape(id)
    );
    super::forms::checked(
        &mut out,
        "Target denied Dexterity",
        "denied_dex",
        f.is_empty() || f.contains_key("denied_dex"),
    );
    super::forms::checked(
        &mut out,
        "Target flanked",
        "flanking",
        f.contains_key("flanking"),
    );
    super::forms::checked(
        &mut out,
        "Target immune to precision damage",
        "precision_immune",
        f.contains_key("precision_immune"),
    );
    super::forms::checked(
        &mut out,
        "Target immune to fire",
        "fire_immune",
        f.contains_key("fire_immune"),
    );
    super::forms::checked(
        &mut out,
        "Use MIC Deadly Precision (+1d6); otherwise Complete Adventurer (+2d6 sneak only)",
        "mic",
        f.contains_key("mic"),
    );
    super::forms::checked(
        &mut out,
        "Teleport destination occupied",
        "occupied",
        f.contains_key("occupied"),
    );
    super::forms::checked(
        &mut out,
        "Target not visible before teleport",
        "unseen",
        f.contains_key("unseen"),
    );
    super::forms::checked(
        &mut out,
        "No Nightstick",
        "no_nightstick",
        f.contains_key("no_nightstick"),
    );
    super::forms::input(
        &mut out,
        "Fire resistance",
        "resistance",
        f.get("resistance").map(String::as_str).unwrap_or("0"),
        "number",
    );
    super::forms::input(
        &mut out,
        "Distance for each teleport (feet)",
        "distance",
        f.get("distance").map(String::as_str).unwrap_or("50"),
        "number",
    );
    out.push_str("<button>Recalculate case</button></form></section>");
    let result = (|| -> Result<MartialCaseReport, String> {
        let mut c: MartialCase = serde_json::from_str(CASE).map_err(|e| e.to_string())?;
        if !f.is_empty() {
            c.plan.denied_dex = f.contains_key("denied_dex");
        }
        c.plan.flanking = f.contains_key("flanking");
        c.plan.precision_immune = f.contains_key("precision_immune");
        c.plan.fire_immune = f.contains_key("fire_immune");
        c.plan.deadly_precision_mic = f.contains_key("mic");
        c.plan.teleport_destination_clear = !f.contains_key("occupied");
        c.plan.target_visible_before = !f.contains_key("unseen");
        c.plan.nightstick = !f.contains_key("no_nightstick");
        c.plan.fire_resistance = f.get("resistance").map_or(Ok(0), |n| {
            n.parse::<u32>()
                .map_err(|_| "Fire resistance must be a nonnegative integer".to_string())
        })?;
        c.plan.teleport_distance_feet = f.get("distance").map_or(Ok(50), |n| {
            n.parse::<u32>()
                .map_err(|_| "Teleport distance must be a nonnegative integer".to_string())
        })?;
        let engine = cached_engine(
            state,
            &state
                .config
                .cohort_root
                .join("data/rules/rainbow-warweaver.rules.json"),
        )?;
        Ok(engine.audit_martial_case(&c))
    })();
    match result {
        Err(e) => super::ui::notice(&mut out, &e, true),
        Ok(r) => {
            let _ = write!(
                out,
                "<section><h2>Build audit</h2><p>BAB {}. Base saves: {:?}.</p><ul>",
                r.base_attack_bonus, r.base_saves
            );
            for e in r
                .build_errors
                .iter()
                .chain(&r.training.errors)
                .chain(&r.unresolved)
            {
                let _ = write!(out, "<li>{}</li>", escape(e));
            }
            out.push_str("</ul><table><thead><tr><th>Track</th><th>Initiator level</th><th>Known capacity</th><th>Readied</th><th>Stances</th></tr></thead><tbody>");
            for (n, t) in &r.training.tracks {
                let _ = write!(
                    out,
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape(n),
                    t.initiator_level,
                    t.known_capacity,
                    t.readied_capacity,
                    t.stance_capacity
                );
            }
            out.push_str("</tbody></table><h3>Ordinary action rules</h3>");
            for e in &r.action_rules_probe.errors {
                super::ui::notice(&mut out, e, true);
            }
            out.push_str("</section><section><h2>Counterfactual resource probe</h2><p>This separately assumes free Divine Impetus and removes the illegal Greater Divine Surge selection. It does not validate the character.</p><ul>");
            for n in &r.resource_probe_assumptions {
                let _ = write!(out, "<li>{}</li>", escape(n));
            }
            out.push_str("</ul>");
            let p = &r.resource_probe;
            let _=write!(out,"<p>{} full attacks; {} main-hand and {} off-hand attacks; {} turning attempts remain.</p>",p.full_attacks,p.main_attacks,p.off_attacks,p.turning_remaining);
            let _ = write!(
                out,
                "<p>One full attack: main {:?}; off {:?}.</p>",
                p.main_attack_bonuses, p.off_attack_bonuses
            );
            if let Some(avg) = p.all_hit_average {
                let _=write!(out,"<p>Conditional all-hit damage: {}–{}; mean {:.1}. No attack rolls or criticals are simulated.</p>",p.hit_damage_min,p.hit_damage_max,avg);
            }
            for e in &p.errors {
                super::ui::notice(&mut out, e, true);
            }
            for n in &p.damage_notes {
                let _ = write!(out, "<p>{}</p>", escape(n));
            }
            out.push_str("<details><summary>Action ledger</summary><ol>");
            for n in &p.log {
                let _ = write!(out, "<li>{}</li>", escape(n));
            }
            out.push_str("</ol></details></section>");
        }
    }
    super::ui::finish(
        &mut out,
        "Rules and resources are evaluated by the engine; the forum numbers are not inputs",
    );
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
