//! PCGen `HITDIE:` values, and the scope suffix that hid four of them.
//!
//! The raw data spells a Hit Die override three ways — `HITDIE:10`,
//! `HITDIE:%/12` (set) and `HITDIE:%up1` (one step up) — and appends a
//! `|CLASS.TYPE=Monster` scope. The parser stripped neither, so anything with a
//! suffix or a `%` silently returned `None` and the override was ignored:
//! `8|CLASS.TYPE=Monster` is plainly an 8, and it did nothing.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_import::PcgClassLevel as DraftClassLevel;
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
    draft.advancement.extend(
        pcgen_import::advancement_from_class_levels(&[DraftClassLevel {
            class_id: "Monster".to_string(),
            level: 3,
            hit_points_gained: 0,
            hit_points_by_level: vec![],
            experience_gained: 0,
        }])
        .into_iter()
        .map({
            let offset = draft.total_level();
            move |mut frame| {
                frame.character_level += offset;
                frame
            }
        }),
    );
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
    // PCGen's tag reference defines these as arithmetic on the class's own die:
    //   %+N added, %-N subtracted, %*N multiplied, %/N divided,
    //   %upN / %downN stepped (and the %Hup / %Hdown spellings).
    //
    // `%/N` is division, not "set to N". The RSRD's `Eighth Hitdie` template is
    // `HITDIE:%/8`, the fractional-Hit-Die mechanic: a d8 monster becomes a d1.
    // Reading it as "set to 8" left the die exactly as it was.

    // HP is `die / 2 + 1` per level, over 3 levels: d4 -> 9, d8 -> 15,
    // d12 -> 21, d16 -> 27, d1 -> 3.
    assert_eq!(hit_points(Some("%+4")), 21, "d8 + 4 is d12");
    assert_eq!(hit_points(Some("%-4")), 9, "d8 - 4 is d4");
    assert_eq!(hit_points(Some("%*2")), 27, "d8 * 2 is d16");
    assert_eq!(hit_points(Some("%/8")), 3, "d8 / 8 is a d1");

    // Stepping moves in twos: d8 one step up is d10, one step down is d6.
    assert_eq!(hit_points(Some("%up1")), 18);
    assert_eq!(hit_points(Some("%down1")), 12);
    assert_eq!(hit_points(Some("%Hup1")), 18, "%Hup is the same as %up");
    assert_eq!(hit_points(Some("%Hdown1")), 12);
}
