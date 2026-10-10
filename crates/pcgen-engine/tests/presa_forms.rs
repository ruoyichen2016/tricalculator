//! `PRESA:x,y,y` — the character's special abilities.
//!
//! PCGen's reference:
//!
//! ```text
//! PRESA:x,y,y
//!   x: Number (The number of Special Abilities a character must have).
//!   y: Text (The name of an Special Abilities).
//!
//! PRESA:1,Turn undead,Rebuke undead,Smite Evil
//! ```
//!
//! "Checks for the specified special abilities as a prerequisite. Special
//! abilities applied by the SAB tag will satisfy this prerequisite."
//!
//! `PreSpecialAbilityTester` matches with `startsWith`, not equality:
//!
//! ```java
//! final String aString = prereq.getKey().toUpperCase();
//! for (SpecialAbility sa : character.getSpecialAbilityList())
//!     if (sa.getKeyName().toUpperCase().startsWith(aString)) runningTotal++;
//! ```
//!
//! That is load-bearing, because an `SAB`'s key name is only its first
//! `|`-separated token — `SAB:Sneak Attack +%d%|Sneak Attack|Sneak Attack Die`
//! has the key name `Sneak Attack +%d%`, which `PRESA:1,Sneak Attack` finds.
//!
//! The list wrapper sums the counts, so `PRESA:1,A,B` is "one of A or B" and
//! `PRESA:2,A,B` needs two matches among them.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/sa.lst", 1, "sa.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied, given a race that grants `abilities`?
fn gated(gate: &str, abilities: &[(&str, &str)]) -> bool {
    let mut records = vec![record("RACE", "Test Race", abilities)];
    records.push(record(
        "FEAT",
        "Gated Feat",
        &[
            ("TYPE", "General"),
            ("BONUS", &format!("VAR|Gated|1|{gate}")),
        ],
    ));
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["sa".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    // The feat has to be taken for its effects to apply.
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn an_sa_tag_grants_an_ability_that_presa_finds() {
    assert!(gated("PRESA:1,Stonecunning", &[("SAB", "Stonecunning")]));
    assert!(!gated("PRESA:1,Stonecunning", &[("SAB", "Evasion")]));
    assert!(!gated("PRESA:1,Stonecunning", &[]));
    // `SA:` and `SAB:` are the same family.
    assert!(gated("PRESA:1,Stonecunning", &[("SA", "Stonecunning")]));
}

#[test]
fn the_match_is_a_prefix_of_the_key_name() {
    // The key name is the first `|` token, so the display text and the `%`
    // placeholders are not part of it — and the prefix is what makes the tag
    // usable against the rogue's level-scaled ability.
    assert!(gated(
        "PRESA:1,Sneak Attack",
        &[("SAB", "Sneak Attack +%d%|Sneak Attack|Sneak Attack Die")]
    ));
    assert!(!gated(
        "PRESA:1,Attack",
        &[("SAB", "Sneak Attack +%d%|Sneak Attack")]
    ));
    // Case-insensitively.
    assert!(gated(
        "PRESA:1,sneak attack",
        &[("SAB", "Sneak Attack +1d6")]
    ));
}

#[test]
fn a_list_accepts_any_of_its_names_and_the_count_is_a_total() {
    let abilities = [("SAB", "Turn Undead")];
    assert!(gated(
        "PRESA:1,Turn undead,Rebuke undead,Smite Evil",
        &abilities
    ));
    assert!(!gated("PRESA:1,Rebuke undead,Smite Evil", &abilities));
    // One ability matches one listed name, so a count of two needs two abilities.
    assert!(!gated("PRESA:2,Turn undead,Rebuke undead", &abilities));
    assert!(gated(
        "PRESA:2,Turn undead,Rebuke undead",
        &[("SAB", "Turn Undead"), ("SAB", "Rebuke Undead")]
    ));
}

#[test]
fn a_qualified_sa_is_only_granted_when_its_prerequisite_holds() {
    // `SAB tags can be qualified with PRE tags`, and a failed one is not granted.
    let qualified = [("SAB", "Create Skeletal Warrior (Su)|PRELEVEL:MIN=15")];
    assert!(!gated("PRESA:1,Create Skeletal Warrior", &qualified));

    let mut records = vec![record("RACE", "Test Race", &qualified)];
    records.push(record(
        "FEAT",
        "Gated Feat",
        &[
            ("TYPE", "General"),
            ("BONUS", "VAR|Gated|1|PRESA:1,Create Skeletal Warrior"),
        ],
    ));
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["sa".to_string()], &records);
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        // 15 character levels: `PRELEVEL:MIN=15` is a total-level gate.
        class_levels: vec![pcgen_engine::DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 15,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    };
    assert!(apply_effects(&pack, &draft).variables.contains_key("Gated"));
}

#[test]
fn a_negated_presa_holds_when_the_ability_is_absent() {
    assert!(gated("!PRESA:1,Stonecunning", &[("SAB", "Evasion")]));
    assert!(!gated("!PRESA:1,Stonecunning", &[("SAB", "Stonecunning")]));
}
