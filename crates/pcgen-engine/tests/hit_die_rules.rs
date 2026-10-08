//! PCGen `HITDIE:` values, and the scope suffix that hid four of them.
//!
//! The raw data spells a Hit Die override three ways — `HITDIE:10`,
//! `HITDIE:%/12` (set) and `HITDIE:%up1` (one step up) — and appends a
//! `|CLASS.TYPE=Monster` scope. The parser stripped neither, so anything with a
//! suffix or a `%` silently returned `None` and the override was ignored:
//! `8|CLASS.TYPE=Monster` is plainly an 8, and it did nothing.

use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/hitdie.lst", 1, "hitdie.pcc");
    RawRecord {
        category: category.to_string(),
        key: name.to_string(),
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

/// Hit points for a 3rd-level d8 monster under the given override.
///
/// HP uses the 3.5e average, `die / 2 + 1` per level: d6 -> 4, d8 -> 5,
/// d10 -> 6, d12 -> 7.
fn hit_points(hitdie: Option<&str>) -> i32 {
    let mut records = vec![
        record(
            "CLASS",
            "Monster",
            &[("HD", "8"), ("TYPE", "Monster"), ("MAXLEVEL", "20")],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    if let Some(value) = hitdie {
        records.push(record("TEMPLATE", "T", &[("HITDIE", value)]));
    }
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let engine = Engine::new(pack);
    let mut draft = CharacterDraft {
        race_id: "Human".to_string(),
        ..CharacterDraft::default()
    };
    if hitdie.is_some() {
        draft.templates = vec!["T".to_string()];
    }
    draft.class_levels.push(DraftClassLevel {
        class_id: "Monster".to_string(),
        level: 3,
        hit_points_gained: 0,
        hit_points_by_level: vec![],
        experience_gained: 0,
    });
    engine.evaluate(&draft).hit_points
}

#[test]
fn the_class_die_is_the_baseline() {
    // The control for everything below.
    assert_eq!(hit_points(None), 15, "3 levels of d8 is 5 each");
}

#[test]
fn a_scope_suffix_does_not_hide_the_die() {
    // `8|CLASS.TYPE=Monster` is an 8. It used to parse as nothing at all.
    assert_eq!(hit_points(Some("10")), 18);
    assert_eq!(
        hit_points(Some("10|CLASS.TYPE=Monster")),
        18,
        "the |CLASS.TYPE= scope must be ignored, not the whole value"
    );
}

#[test]
fn the_percent_forms_are_understood() {
    // `%/N` sets the die outright.
    assert_eq!(hit_points(Some("%/12")), 21, "d12 is 7 per level");
    // `%upN` / `%downN` move it, relative to the class's own die.
    assert_eq!(hit_points(Some("%up1")), 18, "d8 one step up is d10");
    assert_eq!(hit_points(Some("%down1")), 12, "d8 one step down is d6");
}
