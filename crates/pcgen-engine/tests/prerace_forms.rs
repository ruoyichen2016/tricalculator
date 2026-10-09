//! `PRERACE` — five documented spellings, one of which was implemented.
//!
//! PCGen's reference:
//!
//! ```text
//! PRERACE:x,y,y
//!   PRERACE:1,Dwarf,Elf,Human              names
//!   PRERACE:1,Elf%,[Elf (aquatic)]         the % wildcard
//!   PRERACE:1,TYPE=Dire                    a race TYPE tag
//!   PRERACE:1,RACETYPE=Giant               the creature type
//!   PRERACE:1,RACESUBTYPE=Incorporeal      a race subtype
//!   PRERACE:2,RACETYPE=Undead,RACESUBTYPE=Incorporeal
//! ```
//!
//! Only names were handled, and as an exact comparison, so 1,141 of the RSRD's
//! 2,131 values could never match — 929 of them a bare `%`, which the reference
//! defines as matching any race.
//!
//! The gate is observed through a side effect. `active_abilities` holds only
//! *granted* abilities, and an ability merely listed in the draft is not
//! requirement-checked at all (`grant_allowed` is called from one place, the
//! grant path), so asserting on a selected ability would prove nothing.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/prerace.lst", 1, "prerace.pcc");
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

/// Does the ability gated on `prerace` apply to `race`?
///
/// A template grants it automatically, so the requirement is actually enforced;
/// if it holds, the ability's `AUTO:` grants the canary proficiency.
fn active(prerace: &str, race: &str) -> bool {
    let records = vec![
        record(
            "RACE",
            "Elf (High)",
            &[
                ("SIZE", "M"),
                ("TYPE", "Humanoid.PC.Base"),
                ("RACETYPE", "Humanoid"),
                ("RACESUBTYPE", "Elf"),
            ],
        ),
        record(
            "RACE",
            "Dwarf",
            &[
                ("SIZE", "M"),
                ("TYPE", "Humanoid.PC.Base"),
                ("RACETYPE", "Humanoid"),
                ("RACESUBTYPE", "Dwarf"),
            ],
        ),
        record(
            "RACE",
            "Wolf",
            &[("SIZE", "M"), ("TYPE", "Animal"), ("RACETYPE", "Animal")],
        ),
        record(
            "RACE",
            "Dire Wolf",
            &[
                ("SIZE", "L"),
                ("TYPE", "Animal.Dire"),
                ("RACETYPE", "Animal"),
            ],
        ),
        record(
            "RACE",
            "Wraith",
            &[
                ("SIZE", "M"),
                ("TYPE", "Undead"),
                ("RACETYPE", "Undead"),
                ("RACESUBTYPE", "Incorporeal"),
            ],
        ),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("PRERACE", prerace),
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
    let draft = CharacterDraft {
        race_id: race.to_string(),
        templates: vec!["Grantor".to_string()],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn a_bare_percent_matches_any_race() {
    // 929 of the RSRD's values are a bare `%`, which the reference's own example
    // defines as "any race" (`!PRERACE:%` means "has no race"). Compared
    // literally it matched nothing, so those gates never opened.
    assert!(active("1,%", "Elf (High)"));
    assert!(active("1,%", "Wolf"));
    assert!(active("1,%", "Wraith"));
}

#[test]
fn a_prefix_pattern_matches_subraces() {
    // `Elf%` is "any subrace of Elf", and the race is `Elf (High)`.
    assert!(active("1,Elf%", "Elf (High)"));
    assert!(active("1,Dwarf%", "Dwarf"));
    assert!(
        !active("1,Dwarf%", "Elf (High)"),
        "a dwarf pattern is not an elf"
    );
    assert!(!active("1,Elf%", "Wolf"));
}

#[test]
fn the_typed_spellings_are_read() {
    assert!(
        active("1,TYPE=Dire", "Dire Wolf"),
        "TYPE= is a race TYPE tag"
    );
    assert!(!active("1,TYPE=Dire", "Wolf"));
    assert!(
        active("1,RACETYPE=Undead", "Wraith"),
        "RACETYPE= is the creature type"
    );
    assert!(!active("1,RACETYPE=Undead", "Elf (High)"));
    assert!(
        active("1,RACESUBTYPE=Incorporeal", "Wraith"),
        "RACESUBTYPE= is a race subtype"
    );
    assert!(!active("1,RACESUBTYPE=Incorporeal", "Elf (High)"));
}

#[test]
fn the_count_is_the_number_of_matching_specs() {
    // `PRERACE:2,...` needs two of them, not one.
    assert!(active(
        "2,RACETYPE=Undead,RACESUBTYPE=Incorporeal",
        "Wraith"
    ));
    assert!(
        !active("2,RACETYPE=Undead,RACESUBTYPE=Incorporeal", "Dire Wolf"),
        "an animal satisfies neither"
    );
    // With a count of 1 either suffices.
    assert!(active(
        "1,RACETYPE=Undead,RACESUBTYPE=Incorporeal",
        "Wraith"
    ));
    assert!(active("1,RACETYPE=Animal,TYPE=Dire", "Dire Wolf"));
}

#[test]
fn a_plain_name_still_works() {
    assert!(active("1,Elf (High)", "Elf (High)"));
    assert!(!active("1,Elf (High)", "Dwarf"));
    assert!(active("1,Dwarf,Elf (High)", "Dwarf"));
    assert!(!active("1,Dwarf,Elf (High)", "Wolf"));
}

#[test]
fn a_race_gated_selector_entity_is_still_activated() {
    // The engine activates PCGen's racial-defaults entity by asking whether any
    // of its grants is gated on a race (`internal_race_selector`). That test
    // recognised `PRERACE` only while it lowered to a race `HasEntity`; the
    // richer form was not recognised, so the entity was never activated and a
    // Gnome silently lost every racial trait.
    //
    // This asserts the observable end of that chain: the racial default arrives.
    let records = vec![
        record("RACE", "Gnome", &[("SIZE", "S"), ("TYPE", "Humanoid")]),
        record(
            "ABILITY",
            "Select Racial Traits",
            &[
                ("CATEGORY", "Internal"),
                (
                    "ABILITY",
                    "Internal|AUTOMATIC|Gnome Racial Trait Default|PRERACE:1,Gnome",
                ),
            ],
        ),
        record(
            "ABILITY",
            "Gnome Racial Trait Default",
            &[("CATEGORY", "Internal"), ("AUTO", "WEAPONPROF|Canaryblade")],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let draft = CharacterDraft {
        race_id: "Gnome".to_string(),
        ..CharacterDraft::default()
    };
    assert!(
        apply_effects(&pack, &draft)
            .weapon_proficiencies
            .contains("canaryblade"),
        "the race-gated racial default must be granted"
    );
}
