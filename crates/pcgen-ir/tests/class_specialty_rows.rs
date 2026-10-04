//! RSRD class blocks put `SUBCLASS:` / `SUBCLASSLEVEL:` rows *between* the
//! class header and the level progression (the eight Wizard schools precede
//! the Wizard level rows).  Those rows refine the current class; if they are
//! parsed as class headers they move the "current class" cursor, and every
//! level row that follows is attached to the specialty instead of the class.
//!
//! Regression: RSRD Wizard and Dragon Disciple lost *all* of their class
//! feature rows this way, while Fighter / Cleric (whose level rows precede any
//! `SUBCLASS:` row) were unaffected.

use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/rsrd_classes.lst", 1, "classes.pcc");
    RawRecord {
        category: "CLASS".to_string(),
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

/// The RSRD Wizard block, reduced: header, two schools, then level rows.
fn wizard_block() -> Vec<RawRecord> {
    vec![
        record("CLASS:Wizard", &[("HD", "4"), ("MAXLEVEL", "20")]),
        record("CLASS:Wizard", &[("STARTSKILLPTS", "2")]),
        record("SUBCLASS:Abjurer", &[("COST", "2")]),
        record(
            "SUBCLASSLEVEL:1",
            &[(
                "ABILITY",
                "Special Ability|AUTOMATIC|Abjurer Learning Bonus",
            )],
        ),
        record("SUBCLASS:Conjurer", &[("COST", "2")]),
        record(
            "SUBCLASSLEVEL:1",
            &[(
                "ABILITY",
                "Special Ability|AUTOMATIC|Conjurer Learning Bonus",
            )],
        ),
        record(
            "1",
            &[(
                "ABILITY",
                "Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar",
            )],
        ),
        record(
            "1",
            &[(
                "ABILITY",
                "Wizard Class Feature|AUTOMATIC|Wizard ~ Spellbooks",
            )],
        ),
        record(
            "5",
            &[(
                "ABILITY",
                "Wizard Class Feature|AUTOMATIC|Wizard ~ Bonus Feats",
            )],
        ),
    ]
}

fn lower(records: &[RawRecord]) -> pcgen_ir::CompiledRulePack {
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["classes.pcc".to_string()],
        records,
    );
    pack
}

#[test]
fn level_rows_after_subclass_rows_stay_on_the_class() {
    let pack = lower(&wizard_block());

    assert!(
        !pack.classes.contains_key("SUBCLASS:Abjurer"),
        "SUBCLASS row was parsed as a class header: {:?}",
        pack.classes.keys().collect::<Vec<_>>()
    );
    assert!(
        !pack.classes.contains_key("SUBCLASSLEVEL:1"),
        "SUBCLASSLEVEL row was parsed as a class header: {:?}",
        pack.classes.keys().collect::<Vec<_>>()
    );

    let wizard = pack
        .classes
        .get("CLASS:Wizard")
        .or_else(|| pack.classes.get("Wizard"))
        .expect("Wizard class should exist");

    assert_eq!(
        wizard.level_abilities.get(&1).map(Vec::len),
        Some(2),
        "level 1 rows were dropped: {:?}",
        wizard.level_abilities
    );
    assert_eq!(wizard.level_abilities.get(&5).map(Vec::len), Some(1));

    let features = wizard
        .level_abilities
        .values()
        .flatten()
        .collect::<Vec<_>>()
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        features.contains("Wizard ~ Familiar"),
        "the replaceable Familiar feature is missing: {features}"
    );
}

#[test]
fn subclass_rows_do_not_leak_features_into_the_class() {
    // The schools' own ABILITY rows belong to the specialty, not the class.
    let pack = lower(&wizard_block());
    let wizard = pack
        .classes
        .get("CLASS:Wizard")
        .or_else(|| pack.classes.get("Wizard"))
        .expect("Wizard class");

    let features = wizard
        .level_abilities
        .values()
        .flatten()
        .collect::<Vec<_>>()
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !features.contains("Learning Bonus"),
        "SUBCLASSLEVEL ability leaked onto the class: {features}"
    );
}

#[test]
fn classes_without_specialty_rows_are_unaffected() {
    // Fighter layout: level rows come before any specialty row.
    let records = vec![
        record("CLASS:Fighter", &[("HD", "10")]),
        record(
            "1",
            &[(
                "ABILITY",
                "Fighter Class Feature|AUTOMATIC|Fighter ~ Bonus Feats",
            )],
        ),
    ];
    let pack = lower(&records);
    let fighter = pack
        .classes
        .get("CLASS:Fighter")
        .or_else(|| pack.classes.get("Fighter"))
        .expect("Fighter class");
    assert_eq!(fighter.level_abilities.get(&1).map(Vec::len), Some(1));
}
