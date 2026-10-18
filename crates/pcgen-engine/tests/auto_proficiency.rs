//! `AUTO:WEAPONPROF|...|PRE*` — the gate in the row, not just its values.
//!
//! `AUTO:WEAPONPROF|TYPE=Martial|PREHD:MAX=0` (the Outsider house rule) folds a
//! predicate into a list of proficiency values. It used to be treated as three
//! values joined by `|`, so `TYPE=Martial` and `PREHD:MAX=0` were both inserted
//! as weapon proficiency *names* and the gate was never evaluated — a
//! 12th-level character was granted martial proficiency it should not have.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_import::PcgClassLevel as DraftClassLevel;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/auto.lst", 1, "auto.pcc");
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

/// Weapon proficiencies the `AUTO:` row grants at `level`, filtered to the ones
/// this test is about so a change elsewhere does not make it brittle.
fn martial_proficiencies(auto: &str, level: u32) -> Vec<String> {
    let records = vec![
        record(
            "ABILITY",
            "Source",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("AUTO", auto),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M"), ("MOVE", "Walk,30")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let mut draft = CharacterDraft {
        race_id: "Human".to_string(),
        abilities: vec![("Special Ability".to_string(), "Source".to_string())],
        ..CharacterDraft::default()
    };
    draft.advancement.extend(
        pcgen_import::advancement_from_class_levels(&[DraftClassLevel {
            class_id: "Fighter".to_string(),
            level,
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
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .into_iter()
        .filter(|prof| prof.contains("martial") || prof.contains("prehd"))
        .collect()
}

#[test]
fn an_ungated_grant_still_grants() {
    // The control: without this passing, the other assertions prove nothing.
    assert_eq!(
        martial_proficiencies("WEAPONPROF|TYPE=Martial", 12),
        vec!["type.martial"]
    );
}

#[test]
fn a_gate_in_the_row_is_evaluated() {
    let row = "WEAPONPROF|TYPE=Martial|PREHD:MAX=0";
    // The gate is on Hit Dice, so a 12th-level character does not get it…
    assert!(
        martial_proficiencies(row, 12).is_empty(),
        "PREHD:MAX=0 is not satisfied at 12th level"
    );
    // …and an ungated reading of the same row at 0 HD does.
    assert_eq!(martial_proficiencies(row, 0), vec!["type.martial"]);
}

#[test]
fn the_predicate_does_not_become_a_proficiency_name() {
    // `PREHD:MAX=0` used to be inserted as a proficiency alongside the real one.
    let granted = martial_proficiencies("WEAPONPROF|TYPE=Martial|PREHD:MAX=0", 0);
    assert!(
        !granted.iter().any(|prof| prof.contains("prehd")),
        "the gate must not survive as a value: {granted:?}"
    );
}
