//! `.MOD / .COPY= / .FORGET` ordering, from PCGen's file-format notes.
//!
//! ```text
//! The order of operations is: Native line, COPY, MOD, then FORGET, but the
//! order of operation is also controlled by the RANK tag in the PCC file.
//!
//! Given:  1.MOD  1.FORGET  2.COPY  2.Blank  2.MOD  3.MOD
//! Order:  2.Blank  2.COPY  1.MOD  2.MOD  3.MOD  1.FORGET
//! ```
//!
//! `.COPY=` looked the *whole* record name up (`Base.COPY=Copy`) rather than the
//! base (`Base`), so it found nothing: the copy was emitted under its own
//! `Base.COPY=Copy` name and inherited none of the base's tags.

use pcgen_ir::patched::apply_record_patches;
use pcgen_ir::source::{RawRecord, RawTag, SourceRef};

/// A record as the tokenizer would build it:  strips  and
/// everything from  on, so the patch pass can find the base.
fn record(name: &str, category: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/patch.lst", 1, "patch.pcc");
    RawRecord {
        category: category.to_string(),
        key: name
            .trim_end_matches(".MOD")
            .trim_end_matches(".FORGET")
            .split(".COPY=")
            .next()
            .unwrap_or(name)
            .to_string(),
        name: name.to_string(),
        tags: tags
            .iter()
            .map(|(n, v)| RawTag {
                name: (*n).to_string(),
                value: (*v).to_string(),
                source: source.clone(),
            })
            .collect(),
        source,
        raw: String::new(),
    }
}

fn names(records: &[RawRecord]) -> Vec<String> {
    let mut out: Vec<String> = apply_record_patches(records)
        .into_iter()
        .map(|record| record.name.clone())
        .collect();
    out.sort();
    out
}

fn tags_of(records: &[RawRecord], name: &str) -> Vec<String> {
    let mut out: Vec<String> = apply_record_patches(records)
        .into_iter()
        .find(|record| record.name.eq_ignore_ascii_case(name))
        .map(|record| {
            record
                .tags
                .iter()
                .map(|tag| format!("{}:{}", tag.name, tag.value))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[test]
fn a_copy_takes_the_new_name_and_the_base_tags() {
    let records = vec![
        record("Base", "ABILITY", &[("DESC", "base")]),
        record("Base.COPY=Copy", "ABILITY", &[("DESC", "copied")]),
    ];
    let names = names(&records);
    assert!(names.contains(&"Copy".to_string()), "got {names:?}");
    assert!(
        !names.iter().any(|name| name.contains(".COPY=")),
        "the copy must not keep the source spelling: {names:?}"
    );
    // "Any tags that follow the .COPY tag are added to the list of tags already
    //  in the object."
    assert_eq!(
        tags_of(&records, "Copy"),
        vec!["DESC:base".to_string(), "DESC:copied".to_string()]
    );
}

#[test]
fn the_copy_target_is_addressable() {
    // The new object can itself be patched by the name it was given.
    let records = vec![
        record("Base", "ABILITY", &[("DESC", "base")]),
        record("Base.COPY=Copy", "ABILITY", &[("DESC", "copied")]),
        record("Copy.MOD", "ABILITY", &[("DESC", "then modified")]),
    ];
    assert_eq!(
        tags_of(&records, "Copy"),
        vec![
            "DESC:base".to_string(),
            "DESC:copied".to_string(),
            "DESC:then modified".to_string(),
        ]
    );
}

#[test]
fn an_ability_category_prefix_is_stripped_from_the_base() {
    // `CATEGORY=Feat|Brew Potion.COPY=...` — the base follows the last pipe.
    let records = vec![
        record("CATEGORY=Feat|Brew Potion", "ABILITY", &[("DESC", "base")]),
        record(
            "CATEGORY=Feat|Brew Potion.COPY=Brew Potion (Abebi)",
            "ABILITY",
            &[("DESC", "copied")],
        ),
    ];
    let names = names(&records);
    assert!(
        names.contains(&"Brew Potion (Abebi)".to_string()),
        "got {names:?}"
    );
}

#[test]
fn mod_and_forget_do_not_depend_on_line_order() {
    // "The order of operations is: Native line, COPY, MOD, then FORGET."
    let before = vec![
        record("Thing.MOD", "ABILITY", &[("DESC", "modified")]),
        record("Thing", "ABILITY", &[("DESC", "native")]),
    ];
    let after = vec![
        record("Thing", "ABILITY", &[("DESC", "native")]),
        record("Thing.MOD", "ABILITY", &[("DESC", "modified")]),
    ];
    assert_eq!(tags_of(&before, "Thing"), tags_of(&after, "Thing"));
    assert_eq!(
        tags_of(&before, "Thing"),
        vec!["DESC:modified".to_string(), "DESC:native".to_string()]
    );
}

#[test]
fn forget_removes_the_object() {
    let records = vec![
        record("Thing", "ABILITY", &[("DESC", "native")]),
        record("Thing.FORGET", "ABILITY", &[]),
    ];
    assert!(
        !names(&records).contains(&"Thing".to_string()),
        "FORGET must remove it: {:?}",
        names(&records)
    );
}
