use super::ui::{escape, url};
use super::*;
use std::fmt::Write;
#[derive(Default, Deserialize)]
pub(super) struct Selection {
    #[serde(default)]
    case: String,
    #[serde(default)]
    id: String,
}
pub(super) async fn page(
    State(state): State<AppState>,
    Query(q): Query<Selection>,
) -> Html<String> {
    let mut out = ui::start(
        ui::Page::Cases,
        &q.id,
        "Cases",
        "Original claims, acquisition evidence and separate experiments.",
        &[],
    );
    out.push_str("<section><h2>Extreme builds</h2><nav>");
    for (key, name) in [
        ("shadow-pounce", "Shadow Pounce"),
        ("warforged-umbral", "Warforged Umbral Disciple"),
        ("joker", "The Joker"),
    ] {
        let _ = write!(
            out,
            "<a href=\"/cases?case={key}&id={}\">{name}</a>",
            url(&q.id)
        );
    }
    out.push_str("</nav></section>");
    if !q.case.is_empty() {
        let result = (|| -> Result<(), String> {
            let e = cached_engine(
                &state,
                &state
                    .config
                    .cohort_root
                    .join("data/rules/rainbow-warweaver.rules.json"),
            )?;
            let r = e.case_summary(&q.case)?;
            let _=write!(out,"<section><h2>{}</h2><h3>Claims and results</h3><table><thead><tr><th>Claim</th><th>Result</th></tr></thead><tbody>",escape(&r.title));
            for (claim, result) in r.claims {
                let _ = write!(
                    out,
                    "<tr><td>{}</td><td>{}</td></tr>",
                    escape(&claim),
                    escape(&result)
                );
            }
            out.push_str("</tbody></table><h3>Assumptions and missing evidence</h3><ul>");
            for a in r.assumptions {
                let _ = write!(out, "<li>{}</li>", escape(&a));
            }
            let _=write!(out,"</ul><p><a href=\"{}\">Open separate experiment form</a>. Experiment changes do not repair or overwrite the original build.</p></section><section><h2>Original acquisition audit</h2>",escape(&r.probe_path));
            build_forms::decisions(&mut out, &r.acquisition, None);
            out.push_str("</section>");
            Ok(())
        })();
        if let Err(e) = result {
            ui::notice(&mut out, &e, true);
        }
    }
    ui::finish(
        &mut out,
        "A passing operation probe does not certify the original character",
    );
    Html(out)
}
