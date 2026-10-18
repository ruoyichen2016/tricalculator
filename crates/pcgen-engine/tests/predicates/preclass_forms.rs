//! `PRECLASS:x,y=z,y=z` — the count, and the four ways of naming a class.
//!
//! PCGen's reference:
//!
//! ```text
//! PRECLASS:x,y=z,y=z
//!   x: Number (The number of classes that must be equal to or greater than
//!              the numbers specified for the check to succeed)
//!   y: Text (Class Name)   y: TYPE.Text (Class Type)
//!   y: SPELLCASTER   y: SPELLCASTER.Type (Spellcaster Type)   y: ANY
//!   z: Number (Class Level)
//!
//! PRECLASS:2,Wizard=5,Sorcerer=6,Cleric=7
//!   "Multi-classed character must be at least Wiz5/Sor6, Wiz5/Clr7 or Sor6/Clr7"
//! PRECLASS:1,SPELLCASTER=2
//!   "Character must have 2 levels in any spellcasting class"
//! PRECLASS:1,SPELLCASTER.Arcane=2
//! ```
//!
//! Only the first `y=z` pair was read and the count was ignored, so 140 of the
//! RSRD's 1,735 values tested a single class; `SPELLCASTER` (332 values),
//! `TYPE.` (51) and `ANY` (2) looked for a class by that literal name and so
//! could never match.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_import::PcgClassLevel as DraftClassLevel;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/preclass.lst", 1, "preclass.pcc");
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

/// Is the ability gated on `preclass` active for a character with these levels?
fn active(preclass: &str, levels: &[(&str, u32)]) -> bool {
    let records = vec![
        record(
            "CLASS",
            "Wizard",
            &[
                ("HD", "4"),
                ("MAXLEVEL", "20"),
                ("TYPE", "PC.Base"),
                ("FACT", "SpellType|Arcane"),
                ("SPELLSTAT", "INT"),
            ],
        ),
        record(
            "CLASS",
            "Sorcerer",
            &[
                ("HD", "4"),
                ("MAXLEVEL", "20"),
                ("TYPE", "PC.Base"),
                ("FACT", "SpellType|Arcane"),
                ("SPELLSTAT", "CHA"),
            ],
        ),
        record(
            "CLASS",
            "Cleric",
            &[
                ("HD", "8"),
                ("MAXLEVEL", "20"),
                ("TYPE", "PC.Base"),
                ("FACT", "SpellType|Divine"),
                ("SPELLSTAT", "WIS"),
            ],
        ),
        record(
            "CLASS",
            "Fighter",
            &[("HD", "10"), ("MAXLEVEL", "20"), ("TYPE", "PC.Base")],
        ),
        record(
            "CLASS",
            "Archmage",
            &[
                ("HD", "4"),
                ("MAXLEVEL", "10"),
                ("TYPE", "PC.Prestige"),
                ("FACT", "SpellType|Arcane"),
                ("SPELLSTAT", "INT"),
            ],
        ),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("PRECLASS", preclass),
                ("AUTO", "WEAPONPROF|Canaryblade"),
            ],
        ),
        record(
            "TEMPLATE",
            "Grantor",
            &[("ABILITY", "Special Ability|AUTOMATIC|Gated")],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let mut draft = CharacterDraft {
        templates: vec!["Grantor".to_string()],
        ..CharacterDraft::default()
    };
    for (class_id, level) in levels {
        draft.advancement.extend(
            pcgen_import::advancement_from_class_levels(&[DraftClassLevel {
                class_id: (*class_id).to_string(),
                level: *level,
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
    }
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn the_count_is_the_number_of_satisfied_requirements() {
    // "at least Wiz5/Sor6, Wiz5/Clr7 or Sor6/Clr7"
    let gate = "2,Wizard=5,Sorcerer=6,Cleric=7";
    assert!(active(gate, &[("Wizard", 5), ("Sorcerer", 6)]));
    assert!(active(gate, &[("Wizard", 5), ("Cleric", 7)]));
    assert!(!active(gate, &[("Wizard", 5)]), "one of three is not two");
    assert!(
        !active(gate, &[("Wizard", 4), ("Sorcerer", 6)]),
        "Wizard is one short"
    );
}

#[test]
fn spellcaster_sums_the_levels_of_matching_classes() {
    // "Character must have 2 levels in any spellcasting class."
    assert!(active("1,SPELLCASTER=2", &[("Wizard", 2)]));
    assert!(active("1,SPELLCASTER=2", &[("Cleric", 2)]));
    assert!(
        active("1,SPELLCASTER=2", &[("Wizard", 1), ("Sorcerer", 1)]),
        "multiclass casters sum"
    );
    assert!(
        !active("1,SPELLCASTER=2", &[("Fighter", 10)]),
        "a fighter is not a caster"
    );
}

#[test]
fn spellcaster_can_be_restricted_by_power_source() {
    assert!(active("1,SPELLCASTER.Arcane=2", &[("Wizard", 2)]));
    assert!(!active("1,SPELLCASTER.Arcane=2", &[("Cleric", 6)]));
    assert!(active("1,SPELLCASTER.Divine=6", &[("Cleric", 6)]));
    assert!(!active("1,SPELLCASTER.Divine=6", &[("Wizard", 6)]));
    // The reference's combined example.
    assert!(active(
        "1,SPELLCASTER.Divine=6,SPELLCASTER.Psionic=3",
        &[("Cleric", 6)]
    ));
}

#[test]
fn a_class_type_can_be_required() {
    assert!(active("1,TYPE.Prestige=1", &[("Archmage", 1)]));
    assert!(!active("1,TYPE.Prestige=1", &[("Wizard", 5)]));
    assert!(active("1,TYPE=Base=5", &[("Wizard", 5)]));
}

#[test]
fn any_accepts_any_class() {
    assert!(active("1,ANY=3", &[("Fighter", 3)]));
    assert!(!active("1,ANY=3", &[("Fighter", 2)]));
}

#[test]
fn prespelltype_accepts_any_power() {
    // PCGen's reference lists `Arcane`, `Divine`, `Psionic` and `ANY` for this
    // parameter. `ANY` used to fail `SpellPowerType::from_pcgen`, which made the
    // whole `PRESPELLTYPE` tag unsupported.
    use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
    let source = SourceRef::new("/tmp/any.lst", 1, "any.pcc");
    let record = RawRecord {
        category: "ABILITY".to_string(),
        key: "Gated".to_string(),
        name: "Gated".to_string(),
        tags: vec![
            RawTag {
                name: "CATEGORY".to_string(),
                value: "Special Ability".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "PRESPELLTYPE".to_string(),
                value: "1,ANY=3".to_string(),
                source: source.clone(),
            },
        ],
        source,
        raw: String::new(),
    };
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &[record]);
    let entity = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.eq_ignore_ascii_case("gated"))
        .expect("Gated");
    assert!(
        entity.unsupported_tags.is_empty(),
        "ANY must parse: {:?}",
        entity.unsupported_tags
    );
    assert_eq!(
        format!("{:?}", entity.requirements[0]),
        "SpellType { specs: [SpellTypeSpec { spell_type: None, min_level: 3 }], count: 1 }"
    );
}

/// The lowered `PRECLASS` predicate, for asserting on the reading itself.
fn lowered(preclass: &str) -> String {
    let records = vec![record(
        "ABILITY",
        "Gated",
        &[("CATEGORY", "Special Ability"), ("PRECLASS", preclass)],
    )];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let entity = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.eq_ignore_ascii_case("gated"))
        .expect("Gated");
    assert!(
        entity.unsupported_tags.is_empty(),
        "must parse: {:?}",
        entity.unsupported_tags
    );
    format!("{:?}", entity.requirements[0])
}

/// Was the `PRECLASS` value reported as unsupported rather than transcribed?
fn unsupported(preclass: &str) -> bool {
    let records = vec![record(
        "ABILITY",
        "Gated",
        &[("CATEGORY", "Special Ability"), ("PRECLASS", preclass)],
    )];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let entity = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.eq_ignore_ascii_case("gated"))
        .expect("Gated");
    !entity.unsupported_tags.is_empty()
}

#[test]
fn a_class_name_may_omit_the_level() {
    // `PRECLASS:1,Bard` — the Babylon 5 and Star Wars templates are written this
    // way. The reference reads the whole part as the key and falls back to the
    // count for the level, so this is "a Bard of at least 1st". Requiring
    // `=level` dropped all 21 of them.
    assert_eq!(
        lowered("1,Bard"),
        "ClassRequirement { specs: [Class { class_id: \"Bard\", min_level: 1 }], min_count: 1 }"
    );
}

#[test]
fn a_non_numeric_level_is_reported_rather_than_transcribed() {
    // `SPELLCASTER.Divine=9th`: the reference reads the whole part as the key and
    // the level as the count, producing the key `SPELLCASTER.Divine=9th`, which
    // can never name a class. Transcribing that would carry nonsense forward, so
    // the predicate is left unsupported and the value is reported instead.
    assert!(unsupported("1,SPELLCASTER.Divine=9th"));
    assert!(unsupported("1,SPELLCASTER.Arcane=1st+ level."));
}
