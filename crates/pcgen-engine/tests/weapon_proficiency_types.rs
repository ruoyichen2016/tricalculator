//! `TYPE=` in `AUTO:WEAPONPROF` and `PREWEAPONPROF`.
//!
//! PCGen's tag reference lists both as taking a proficiency *type*:
//!
//! ```text
//! AUTO:WEAPONPROF|x|x
//!   x: Text (Weapon name)   x: TYPE.Text (Weapon Type)   x: DEITYWEAPONS
//! PREWEAPONPROF:x,y,y
//!   y: Text (Weapon proficiency name)   y: TYPE.Text (Weapon Proficiency type)
//! ```
//!
//! The granting side handled types; the *requirement* side dropped them, which
//! emptied the list and made the parser discard the whole predicate — so
//! `PREWEAPONPROF:1,TYPE=Martial` required nothing.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/weapontype.lst", 1, "weapontype.pcc");
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

fn pack() -> pcgen_core::CompiledRulePack {
    let records = vec![
        record(
            "EQUIPMENT",
            "Longsword",
            &[("TYPE", "Weapon.Martial.Melee")],
        ),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("PREWEAPONPROF", "1,TYPE=Martial"),
            ],
        ),
        record(
            "ABILITY",
            "Source",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("AUTO", "WEAPONPROF|TYPE=Martial"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    pack
}

#[test]
fn a_type_requirement_is_not_discarded() {
    let pack = pack();
    let gated = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.eq_ignore_ascii_case("gated"))
        .expect("Gated");
    assert_eq!(
        gated.requirements.len(),
        1,
        "the predicate must survive: {:?}",
        gated.requirements
    );
    // `TYPE=` and `TYPE.` are both normalised to the marker the granting side
    // writes, so the two agree.
    assert_eq!(
        format!("{:?}", gated.requirements[0]),
        "WeaponProf { names: [\"type.martial\"] }"
    );
}

#[test]
fn a_type_grant_expands_to_the_weapons_of_that_type() {
    let pack = pack();
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        abilities: vec![("Special Ability".to_string(), "Source".to_string())],
        ..CharacterDraft::default()
    };
    let state = apply_effects(&pack, &draft);
    assert!(
        state.weapon_proficiencies.contains("type.martial"),
        "the type marker is what a type requirement matches: {:?}",
        state.weapon_proficiencies
    );
    assert!(
        state.weapon_proficiencies.contains("longsword"),
        "and every weapon of the type is granted: {:?}",
        state.weapon_proficiencies
    );
}

#[test]
fn the_dot_spelling_agrees_with_the_equals_spelling() {
    // The data uses `TYPE=Martial`; the reference's examples use `TYPE.Martial`.
    let records = vec![
        record(
            "EQUIPMENT",
            "Longsword",
            &[("TYPE", "Weapon.Martial.Melee")],
        ),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("PREWEAPONPROF", "1,TYPE.Martial"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let gated = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.eq_ignore_ascii_case("gated"))
        .expect("Gated");
    assert_eq!(
        format!("{:?}", gated.requirements[0]),
        "WeaponProf { names: [\"type.martial\"] }"
    );
}
