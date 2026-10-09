//! `PREABILITY:x,CATEGORY=y,z` — count, `TYPE.`, and bracketed exclusions.
//!
//! PCGen's reference:
//!
//! ```text
//! PREABILITY:1,Category=Feat,TYPE.SpellFocus   one SpellFocus feat
//! PREABILITY:3,Category=Feat,TYPE.Metamagic    three Metamagic feats
//! PREABILITY:1,Category=SpecialQuality,[Night Vision]
//!   "Requires that the creature does not have the Night Vision special quality"
//! PREABILITY:1,CATEGORY=Feat,Power Attack,[Weapon Finesse]
//!   "has Power Attack, and does not have Weapon Finesse"
//! ```
//!
//! Only the last bare name was read, so a bracketed exclusion became a
//! requirement (and never matched), `TYPE.` was not understood, and the count
//! was ignored. 924 of the RSRD's 7,183 values use brackets, 1,303 use `TYPE.`,
//! and 2,481 list more than one name.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/preability.lst", 1, "preability.pcc");
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

/// Is the ability gated on `preability` active, given these granted abilities?
///
/// Each granted ability is `(category, name, type tags)`.
fn active(preability: &str, granted: &[(&str, &str, &str)]) -> bool {
    // The catalogue: every ability the gate may refer to.
    let catalogue: &[(&str, &str, &str)] = &[
        ("Feat", "Power Attack", "Fighter"),
        ("Feat", "Weapon Finesse", "Fighter"),
        ("Feat", "Silent Spell", "Metamagic"),
        ("Feat", "Quicken Spell", "Metamagic"),
        ("Feat", "Empower Spell", "Metamagic"),
        ("Feat", "Spell Focus", "SpellFocus"),
        ("Special Quality", "Night Vision", "SpecialQuality"),
    ];
    let mut records: Vec<RawRecord> = catalogue
        .iter()
        .map(|(category, name, types)| {
            record("ABILITY", name, &[("CATEGORY", category), ("TYPE", types)])
        })
        .collect();
    records.push(record(
        "ABILITY",
        "Gated",
        &[
            ("CATEGORY", "Special Ability"),
            ("TYPE", "SpecialQuality"),
            ("PREABILITY", preability),
            ("AUTO", "WEAPONPROF|Canaryblade"),
        ],
    ));

    // Grant the chosen abilities, plus the gate itself, from one template.
    let mut grants: Vec<String> = granted
        .iter()
        .map(|(category, name, _)| format!("{category}|AUTOMATIC|{name}"))
        .collect();
    grants.push("Special Ability|AUTOMATIC|Gated".to_string());
    let grant_tags: Vec<(&str, &str)> = grants
        .iter()
        .map(|grant| ("ABILITY", grant.as_str()))
        .collect();
    records.push(record("TEMPLATE", "Grantor", &grant_tags));

    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let draft = CharacterDraft {
        templates: vec!["Grantor".to_string()],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn a_type_requirement_finds_feats_of_that_type() {
    assert!(active(
        "1,CATEGORY=Feat,TYPE.Metamagic",
        &[("Feat", "Silent Spell", "Metamagic")]
    ));
    assert!(!active(
        "1,CATEGORY=Feat,TYPE.Metamagic",
        &[("Feat", "Power Attack", "Fighter")]
    ));
}

#[test]
fn the_count_is_honoured() {
    // Three Metamagic feats, not one.
    assert!(!active(
        "3,CATEGORY=Feat,TYPE.Metamagic",
        &[("Feat", "Silent Spell", "Metamagic")]
    ));
    assert!(active(
        "3,CATEGORY=Feat,TYPE.Metamagic",
        &[
            ("Feat", "Silent Spell", "Metamagic"),
            ("Feat", "Quicken Spell", "Metamagic"),
            ("Feat", "Empower Spell", "Metamagic"),
        ]
    ));
}

#[test]
fn a_bracketed_name_is_an_exclusion() {
    // `[Night Vision]` means the creature must NOT have it. Taken as a
    // requirement it never matched, so the gate was permanently closed.
    assert!(active("1,CATEGORY=Special Quality,[Night Vision]", &[]));
    assert!(!active(
        "1,CATEGORY=Special Quality,[Night Vision]",
        &[("Special Quality", "Night Vision", "SpecialQuality")]
    ));
}

#[test]
fn wanted_and_excluded_can_be_combined() {
    // "has Power Attack, and does not have Weapon Finesse"
    assert!(active(
        "1,CATEGORY=Feat,Power Attack,[Weapon Finesse]",
        &[("Feat", "Power Attack", "Fighter")]
    ));
    assert!(!active(
        "1,CATEGORY=Feat,Power Attack,[Weapon Finesse]",
        &[
            ("Feat", "Power Attack", "Fighter"),
            ("Feat", "Weapon Finesse", "Fighter"),
        ]
    ));
    assert!(!active(
        "1,CATEGORY=Feat,Power Attack,[Weapon Finesse]",
        &[]
    ));
}

#[test]
fn several_wanted_names_need_only_the_count() {
    assert!(active(
        "1,CATEGORY=Feat,Power Attack,Weapon Finesse",
        &[("Feat", "Power Attack", "Fighter")]
    ));
    assert!(!active(
        "2,CATEGORY=Feat,Power Attack,Weapon Finesse",
        &[("Feat", "Power Attack", "Fighter")]
    ));
}

#[test]
fn both_type_spellings_are_read() {
    // The reference's examples use `TYPE.Metamagic`; the RSRD writes
    // `TYPE=Metamagic`. Handling only the dotted form made four RSRD values
    // unparseable, and they landed in the unsupported-tag report — which is how
    // the pf35e gate caught it.
    assert!(active(
        "1,CATEGORY=Feat,TYPE=Metamagic",
        &[("Feat", "Silent Spell", "Metamagic")]
    ));
    assert!(!active("1,CATEGORY=Feat,TYPE=Metamagic", &[]));
    assert!(active(
        "3,CATEGORY=Feat,TYPE=Metamagic,TYPE=ItemCreation",
        &[
            ("Feat", "Silent Spell", "Metamagic"),
            ("Feat", "Quicken Spell", "Metamagic"),
            ("Feat", "Empower Spell", "Metamagic"),
        ]
    ));
}
