//! `NATURALATTACKS` — a PCGen row that was reaching the character sheet.
//!
//! The sheet printed the joined raw strings, so a wolf's attack routine read
//! `Bite,Weapon.Natural.Weapon Group Natural.Melee.Finesseable.Bludgeoning.
//! Piercing.Slashing,*1,1d6`. The row also packs several attacks into one tag
//! with `|`, and reading the whole value as a single attack let the second
//! attack's fields overwrite the first's.

use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/natural.lst", 1, "natural.pcc");
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

fn attacks(race: &str) -> Vec<String> {
    let records = vec![
        record(
            "RACE",
            "Wolf",
            &[
                ("SIZE", "M"),
                ("NATURALATTACKS", "Bite,Weapon.Natural.Melee,*1,1d6"),
            ],
        ),
        record(
            "RACE",
            "Bear",
            &[
                ("SIZE", "L"),
                (
                    "NATURALATTACKS",
                    "Claw,Weapon.Natural.Melee,*2,1d8|Bite,Weapon.Natural.Melee,*1,1d8",
                ),
            ],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let engine = Engine::new(pack);
    let mut draft = CharacterDraft {
        race_id: race.to_string(),
        ..CharacterDraft::default()
    };
    draft.class_levels.push(DraftClassLevel {
        class_id: "Fighter".to_string(),
        level: 1,
        hit_points_gained: 0,
        hit_points_by_level: vec![],
        experience_gained: 0,
    });
    engine.evaluate(&draft).natural_attacks
}

#[test]
fn an_attack_reads_as_an_attack() {
    assert_eq!(attacks("Wolf"), vec!["Bite 1d6"]);
}

#[test]
fn every_attack_in_the_tag_survives() {
    // `|` separates attacks. Reading the value as one attack lost the claw.
    let bear = attacks("Bear");
    assert!(
        bear.iter().any(|line| line.starts_with("Claw")),
        "the claw must survive: {bear:?}"
    );
    assert!(
        bear.iter().any(|line| line.starts_with("Bite")),
        "the bite must survive: {bear:?}"
    );
    assert_eq!(bear.len(), 2, "a bear has two attacks, not one: {bear:?}");
}

#[test]
fn the_count_is_kept() {
    let bear = attacks("Bear");
    assert!(
        bear.iter()
            .any(|line| line.contains('×') && line.contains('2')),
        "two claws must say so: {bear:?}"
    );
}

#[test]
fn no_pcgen_encoding_reaches_the_sheet() {
    for race in ["Wolf", "Bear"] {
        for line in attacks(race) {
            assert!(
                !line.contains("Weapon.Natural") && !line.contains(','),
                "raw PCGen text on the sheet: {line:?}"
            );
        }
    }
}
