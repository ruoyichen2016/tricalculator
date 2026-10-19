//! Shared static form primitives; values are escaped at the HTML boundary.
use super::ui::escape;
use std::{collections::BTreeMap, fmt::Write};
pub(super) fn input(out: &mut String, label: &str, name: &str, value: &str, kind: &str) {
    let _ = write!(
        out,
        "<label>{}<input name=\"{name}\" value=\"{}\" type=\"{kind}\"></label>",
        escape(label),
        escape(value)
    );
}
pub(super) fn input_list(
    out: &mut String,
    label: &str,
    name: &str,
    value: &str,
    kind: &str,
    list_id: &str,
) {
    let _ = write!(
        out,
        "<label>{}<input name=\"{name}\" value=\"{}\" type=\"{kind}\" list=\"{}\"></label>",
        escape(label),
        escape(value),
        escape(list_id)
    );
}
pub(super) fn datalist<'a>(out: &mut String, id: &str, items: impl IntoIterator<Item = &'a str>) {
    let _ = write!(out, "<datalist id=\"{}\">", escape(id));
    for item in items {
        let _ = write!(out, "<option value=\"{}\">", escape(item));
    }
    out.push_str("</datalist>");
}
pub(super) fn check(out: &mut String, label: &str, name: &str) {
    checked(out, label, name, false);
}
pub(super) fn checked(out: &mut String, label: &str, name: &str, selected: bool) {
    let _ = write!(
        out,
        r#"<label class="check"><input name="{}" type="checkbox" {}>{}</label>"#,
        escape(name),
        if selected { "checked" } else { "" },
        escape(label)
    );
}
pub(super) fn begin(out: &mut String, id: &str, rev: u64, op: &str) {
    let _=write!(out,"<form method=\"post\" action=\"/class-play\"><input type=\"hidden\" name=\"id\" value=\"{}\"><input type=\"hidden\" name=\"revision\" value=\"{rev}\"><input type=\"hidden\" name=\"op\" value=\"{op}\">",escape(id));
}
pub(super) fn value<'a>(f: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    f.get(key)
        .map(|s| s.trim())
        .ok_or_else(|| format!("Missing {key}"))
}
pub(super) fn number(f: &BTreeMap<String, String>, key: &str) -> Result<u32, String> {
    value(f, key)?
        .parse()
        .map_err(|_| format!("{key} requires a nonnegative integer"))
}
