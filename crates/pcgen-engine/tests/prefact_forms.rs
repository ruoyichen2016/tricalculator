//! `PREFACT:x,location,Fact=Value` — facts on the character's objects.
//!
//! PCGen's reference documents `PREFACT` only through the parser and tester:
//!
//! ```java
//! String location = prereq.getCategoryName();
//! Iterable<Reducible> objModel =
//!         (Iterable<Reducible>) OutputDB.getIterable(aPC.getCharID(), locationElements);
//! String[] factinfo = prereq.getKey().split("=");
//! FactKey<?> fk = FactKey.valueOf(factinfo[0]);
//! Object targetVal = fk.getFormatManager().convertIndirect(factinfo[1]);
//! int runningTotal = 0;
//! for (Reducible r : objModel)
//!     if (targetVal.equals(r.getCDOMObject().get(fk))) runningTotal++;
//! runningTotal = prereq.getOperator().compare(runningTotal, number);
//! ```
//!
//! So it counts the character's objects *in one location* whose named fact has
//! the given value, and the list wrapper sums those counts. `PreFactParser`
//! validates the location against `OutputDB` (`CaseInsensitiveMap`, so
//! `ABILITIES` and `Abilities` are the same) and throws when the formula contains
//! `|`, which is why the malformed third-party form stays unsupported.
//!
//! Only `abilities` and `race` are modelled: they are the two locations the
//! third-party data uses, and lowering refuses any other rather than producing a
//! prerequisite that could only ever be false.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/fact.lst", 1, "fact.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied on a feat, given a race and an ability
/// that carry facts?
fn gated(gate: &str, race_facts: &[(&str, &str)], ability_facts: &[(&str, &str)]) -> bool {
    let mut race_tags: Vec<(&str, &str)> = vec![("TYPE", "Humanoid")];
    race_tags.extend(race_facts.iter().copied());
    let mut ability_tags: Vec<(&str, &str)> = vec![("CATEGORY", "Special Ability")];
    ability_tags.extend(ability_facts.iter().copied());

    let records = vec![
        record("RACE", "Test Race", &race_tags),
        record("ABILITY", "Test Ability", &ability_tags),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) = lower_records(Some("5e".to_string()), vec!["fact".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        abilities: vec![
            ("Special Ability".to_string(), "Test Ability".to_string()),
            ("FEAT".to_string(), "Gated Feat".to_string()),
        ],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_fact_on_an_ability_is_found_in_the_abilities_location() {
    assert!(gated(
        "PREFACT:1,ABILITIES,TestFact=true",
        &[],
        &[("FACT", "TestFact|true")]
    ));
    assert!(!gated("PREFACT:1,ABILITIES,TestFact=true", &[], &[]));
    assert!(!gated(
        "PREFACT:1,ABILITIES,OtherFact=true",
        &[],
        &[("FACT", "TestFact|true")]
    ));
    // The value has to match, not just the name.
    assert!(!gated(
        "PREFACT:1,ABILITIES,TestFact=false",
        &[],
        &[("FACT", "TestFact|true")]
    ));
}

#[test]
fn the_race_location_looks_at_the_race_object() {
    assert!(gated(
        "PREFACT:1,RACE,HasFirstLevelFeat=True",
        &[("FACT", "HasFirstLevelFeat|True")],
        &[]
    ));
    // An ability's fact is not visible in the race location, nor the reverse.
    assert!(!gated(
        "PREFACT:1,RACE,TestFact=true",
        &[],
        &[("FACT", "TestFact|true")]
    ));
    assert!(!gated(
        "PREFACT:1,ABILITIES,HasFirstLevelFeat=True",
        &[("FACT", "HasFirstLevelFeat|True")],
        &[]
    ));
}

#[test]
fn the_value_comparison_ignores_case() {
    // Every fact the third-party data tests is declared `DATAFORMAT:BOOLEAN`,
    // where PCGen's typed parse makes `true` and `True` the same value.
    assert!(gated(
        "PREFACT:1,ABILITIES,TestFact=TRUE",
        &[],
        &[("FACT", "TestFact|true")]
    ));
    assert!(gated(
        "PREFACT:1,ABILITIES,TestFact=True",
        &[],
        &[("FACT", "TestFact|true")]
    ));
}

#[test]
fn the_count_sums_across_the_listed_facts() {
    let facts = [("FACT", "A|true"), ("FACT", "B|true")];
    assert!(gated("PREFACT:1,ABILITIES,A=true", &[], &facts));
    assert!(gated("PREFACT:1,ABILITIES,A=true,B=true", &[], &facts));
    assert!(!gated("PREFACT:3,ABILITIES,A=true,B=true", &[], &facts));
    // `PREFACT:2,...` is met by two matching objects, one per fact.
    assert!(gated("PREFACT:2,ABILITIES,A=true,B=true", &[], &facts));
    // One object matching both facts still counts twice, because the wrapper sums
    // per-fact counts rather than distinct objects.
    assert!(gated("PREFACT:2,ABILITIES,A=true,A=true", &[], &facts));
}

#[test]
fn a_negated_fact_gate_holds_when_the_fact_is_absent() {
    assert!(gated("!PREFACT:1,ABILITIES,TestFact=true", &[], &[]));
    assert!(!gated(
        "!PREFACT:1,ABILITIES,TestFact=true",
        &[],
        &[("FACT", "TestFact|true")]
    ));
}
