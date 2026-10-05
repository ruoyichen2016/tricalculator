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

/// The level-row *entities* must be keyed by their class, not by a preceding
/// specialty row.
///
/// `lower_records` names a class level entity `<class>#<level>`, and the engine
/// activates a level's class features by matching that key.  When a
/// `SUBCLASSLEVEL:` row was taken for a class header, the cursor moved onto it
/// and the following rows were keyed `SUBCLASSLEVEL:1#1` — so the RSRD Wizard's
/// grants existed but were attached to something no class level ever matched,
/// and every Wizard class feature stayed inactive.  The class-level rows parsed
/// fine, which is why only checking `level_abilities` missed it.
#[test]
fn level_row_entities_are_keyed_by_their_class() {
    let pack = lower(&wizard_block());

    let level_entities: Vec<&str> = pack
        .entities
        .iter()
        .filter(|entity| format!("{:?}", entity.category).contains("ClassLevel"))
        .map(|entity| entity.id.key.as_str())
        .collect();

    // This fixture keys its class header `CLASS:Wizard` (the real tokenizer
    // strips the tag and yields `Wizard`), so the level entities carry that
    // prefix.  What matters is that they name the class and not the specialty.
    assert!(
        level_entities.iter().any(|key| key.ends_with("Wizard#1")),
        "Wizard level 1 entity is missing; got {level_entities:?}"
    );
    assert!(
        level_entities.iter().any(|key| key.ends_with("Wizard#5")),
        "Wizard level 5 entity is missing; got {level_entities:?}"
    );
    assert!(
        !level_entities
            .iter()
            .any(|key| key.starts_with("subclasslevel")),
        "level rows were keyed by the specialty row; got {level_entities:?}"
    );

    // The grants themselves must hang off those entities.
    let wizard_level_one = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.ends_with("Wizard#1"))
        .expect("the Wizard level 1 entity exists");
    let granted: Vec<String> = wizard_level_one
        .effects
        .iter()
        .filter_map(|effect| match &effect.action {
            pcgen_core::EffectAction::Grant { entity } => Some(entity.key.clone()),
            _ => None,
        })
        .collect();
    assert!(
        granted.iter().any(|key| key.contains("familiar")),
        "the level-1 familiar grant is not on wizard#1: {granted:?}"
    );
}

/// `SUBSTITUTIONCLASS:` rows are the same hazard as `SUBCLASS:` rows: they
/// refine the class in scope and must not become the cursor.
///
/// The RSRD carries none, so this went unnoticed there, but third-party 3.5e
/// books use them (Secrets of Theurgy, the Pathfinder campaign setting), and a
/// level row keyed after the substitution class never matches its own class.
#[test]
fn substitution_rows_do_not_move_the_class_cursor() {
    let records = vec![
        record("CLASS:Theurge", &[("HD", "4"), ("MAXLEVEL", "10")]),
        record(
            "SUBSTITUTIONCLASS:Lesser Elemental Theurge Option",
            &[("COST", "1")],
        ),
        record(
            "1",
            &[("ABILITY", "Special Ability|AUTOMATIC|Elemental Turning")],
        ),
    ];
    let pack = lower(&records);

    let level_entities: Vec<&str> = pack
        .entities
        .iter()
        .filter(|entity| format!("{:?}", entity.category).contains("ClassLevel"))
        .map(|entity| entity.id.key.as_str())
        .collect();
    assert!(
        level_entities.iter().any(|key| key.ends_with("Theurge#1")),
        "level row was keyed after the substitution class; got {level_entities:?}"
    );
    assert!(
        !level_entities
            .iter()
            .any(|key| key.to_lowercase().contains("substitutionclass")),
        "substitution class became the cursor; got {level_entities:?}"
    );
}
