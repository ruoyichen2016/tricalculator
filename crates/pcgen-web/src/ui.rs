//! Shared page shell and presentation primitives. No rule calculations live here.
use std::fmt::Write;
pub(super) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub(super) fn url(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub(super) fn id_query(id: &str) -> String {
    if id.is_empty() {
        String::new()
    } else {
        format!("?id={}", url(id))
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Page {
    Workbench,
    Studio,
    Encounter,
    Library,
    Spellcasting,
    Cases,
}
pub(super) fn start(
    page: Page,
    id: &str,
    title: &str,
    subtitle: &str,
    anchors: &[(&str, &str)],
) -> String {
    let mut out = format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{} · Gladius</title><style>{}</style></head><body><a class="skip-link" href="#main">Skip to content</a><header class="site-header"><a class="brand" href="/?id={}"><span class="brand-mark" aria-hidden="true">G</span> GLADIUS <small>Rules workbench</small></a><nav class="primary-nav" aria-label="Main navigation">"##,
        escape(title),
        include_str!("ui/workbench.css"),
        url(id)
    );
    for (kind, path, label) in [
        (Page::Workbench, "/", "Character"),
        (Page::Studio, "/build", "Build"),
        (Page::Spellcasting, "/spellcasting", "Spellcasting"),
        (Page::Encounter, "/class-play", "Encounter"),
        (Page::Library, "/classes", "Rules"),
        (Page::Cases, "/cases", "Cases"),
    ] {
        let _ = write!(
            out,
            r#"<a href="{}?id={}" {}>{}</a>"#,
            path,
            url(id),
            if kind == page {
                r#"aria-current="page""#
            } else {
                ""
            },
            label
        );
    }
    let mode = match page {
        Page::Workbench => "PREVIEW & EXPLORE",
        Page::Studio => "BUILD & REFINE",
        Page::Encounter => "PLAY & RESOLVE",
        Page::Library => "READ & UNDERSTAND",
        Page::Spellcasting => "SELECT & CAST",
        Page::Cases => "CLAIMS & EVIDENCE",
    };
    let _ = write!(
        out,
        r#"</nav></header><div class="page-wrap"><div class="page-heading"><div><p class="eyebrow">{}</p><h1>{}</h1><p class="lede">{}</p></div><span class="edition">D&amp;D <strong>3.5</strong><span>RAW rules</span></span></div>"#,
        escape(mode),
        escape(title),
        escape(subtitle)
    );
    if !anchors.is_empty() {
        out.push_str(r#"<nav class="section-nav" aria-label="On this page">"#);
        for (anchor, label) in anchors {
            let _ = write!(
                out,
                r##"<a href="#{}">{}</a>"##,
                escape(anchor),
                escape(label)
            );
        }
        out.push_str("</nav>");
    }
    out.push_str(r#"<main id="main" tabindex="-1">"#);
    out
}
pub(super) fn finish(out: &mut String, note: &str) {
    let _ = write!(
        out,
        r#"</main><footer><span>GLADIUS <span class="muted">/ Experimental rules studio</span></span><span>{}</span></footer></div></body></html>"#,
        escape(note)
    );
}
pub(super) fn notice(out: &mut String, text: &str, error: bool) {
    if text.is_empty() {
        return;
    }
    let _ = write!(
        out,
        r#"<section id="feedback" class="notice {}" role="{}"><strong>{}</strong><p>{}</p></section>"#,
        if error { "error" } else { "result" },
        if error { "alert" } else { "status" },
        if error {
            "Action needs attention"
        } else {
            "Update recorded"
        },
        escape(text)
    );
}
pub(super) fn metric(out: &mut String, label: &str, value: &str, href: &str) {
    let _ = write!(
        out,
        r#"<a class="metric" href="{}"><span>{}</span><strong>{}</strong><span class="metric-link">View details →</span></a>"#,
        escape(href),
        escape(label),
        escape(value)
    );
}

pub(super) fn character_bar(
    out: &mut String,
    state: &super::AppState,
    action_path: &str,
    current_id: &str,
    hidden_fields: &[(&str, &str)],
    links: &[(&str, &str)],
) {
    let _ = write!(
        out,
        r#"<section class="character-picker"><form method="get" action="{}">"#,
        escape(action_path)
    );
    for (k, v) in hidden_fields {
        if !v.is_empty() {
            let _ = write!(
                out,
                r#"<input type="hidden" name="{}" value="{}">"#,
                escape(k),
                escape(v)
            );
        }
    }
    out.push_str(
        r#"<label>Character<select name="id"><option value="">Choose a character</option>"#,
    );
    if let Ok(items) = state.repository.list() {
        for item in items {
            let classes = if item.class_ids.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.class_ids.join("/"))
            };
            let _ = write!(
                out,
                r#"<option value="{}" {}>{} · level {}{}</option>"#,
                escape(&item.id),
                if item.id == current_id {
                    "selected"
                } else {
                    ""
                },
                escape(&item.name),
                item.total_level,
                escape(&classes)
            );
        }
    }
    out.push_str("</select></label><button>Open character</button></form>");
    if !links.is_empty() {
        out.push_str(r#"<div class="context-links">"#);
        for (i, (href, label)) in links.iter().enumerate() {
            if i > 0 {
                out.push_str(r#"<span class="muted">·</span>"#);
            }
            let _ = write!(out, r#"<a href="{}">{}</a>"#, escape(href), escape(label));
        }
        out.push_str("</div>");
    }
    out.push_str("</section>");
}
