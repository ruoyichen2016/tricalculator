//! The four `BONUS` types, from PCGen's bonus documentation.
//!
//! ```text
//! Typeless    no TYPE=            always added
//! Standard    TYPE=name           only the highest of the same type counts
//! .REPLACE    TYPE=name.REPLACE   counted together, then compared with the
//!                                 standards; the highest wins
//! .STACK      TYPE=name.STACK     stacks with itself and is added on top
//! ```
//!
//! `.REPLACE` was wrong twice over: it was grouped apart from the standard type
//! of the same name, so the two were *added* rather than compared, and it took
//! the highest single value rather than the sum. Both of the reference's own
//! worked examples failed.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_import::PcgClassLevel as DraftClassLevel;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/bonustype.lst", 1, "bonustype.pcc");
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

/// Armour class for a character carrying these `BONUS:COMBAT|AC` rows.
fn armor_class(rows: &[(&str, &str)]) -> i32 {
    let mut records = vec![record("RACE", "Human", &[("SIZE", "M")])];
    if !rows.is_empty() {
        records.push(record("TEMPLATE", "T", rows));
    }
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let engine = Engine::new(pack);
    let mut draft = CharacterDraft {
        race_id: "Human".to_string(),
        ..CharacterDraft::default()
    };
    if !rows.is_empty() {
        draft.templates = vec!["T".to_string()];
    }
    draft.advancement.extend(
        pcgen_import::advancement_from_class_levels(&[DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 1,
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
    engine.evaluate(&draft).armor_class
}

fn bonus(value: i32, kind: &str) -> Vec<(&'static str, &'static str)> {
    // Leaked so the returned slice can borrow for the call; the test is short.
    let value: &'static str = Box::leak(value.to_string().into_boxed_str());
    let kind: &'static str = Box::leak(kind.to_string().into_boxed_str());
    let tag: &'static str = Box::leak(format!("COMBAT|AC|{value}|TYPE={kind}").into_boxed_str());
    vec![("BONUS", tag)]
}

fn with(rows: Vec<Vec<(&'static str, &'static str)>>) -> i32 {
    let mut flat: Vec<(&'static str, &'static str)> = Vec::new();
    for row in rows {
        flat.extend(row);
    }
    armor_class(&flat)
}

#[test]
fn standard_bonuses_of_one_type_keep_the_highest() {
    // "these two would result in a final value of 3, since 3 is the largest"
    assert_eq!(with(vec![bonus(2, "Luck"), bonus(3, "Luck")]), 13);
}

#[test]
fn different_types_add_together() {
    assert_eq!(with(vec![bonus(4, "Luck"), bonus(3, "Divine")]), 17);
}

#[test]
fn replace_bonuses_are_summed_then_compared() {
    // "take the 2 and 3 with the .REPLACE (total 5) and compare it to the
    //  standards, which are 2 and 4, and take the highest value, for 5."
    assert_eq!(
        with(vec![
            bonus(3, "Luck.REPLACE"),
            bonus(2, "Luck.REPLACE"),
            bonus(4, "Luck"),
            bonus(2, "Luck"),
        ]),
        15
    );
    // "the .REPLACE (total 4), compared to the standards 2 and 6, highest 6."
    assert_eq!(
        with(vec![
            bonus(1, "Luck.REPLACE"),
            bonus(1, "Luck.REPLACE"),
            bonus(2, "Luck.REPLACE"),
            bonus(2, "Luck"),
            bonus(6, "Luck"),
        ]),
        16
    );
}

#[test]
fn stack_bonuses_are_added_on_top() {
    // "take the 4 of type Luck, and then add 2 more from the Stack."
    assert_eq!(with(vec![bonus(4, "Luck"), bonus(2, "Luck.STACK")]), 16);
    // ".STACK stacks together with itself like .REPLACE, but is also added to
    //  normal types."
    assert_eq!(
        with(vec![bonus(4, "Luck.REPLACE"), bonus(4, "Luck.STACK")]),
        18
    );
}
