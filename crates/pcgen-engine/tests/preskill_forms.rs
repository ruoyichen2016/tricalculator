//! `PRESKILL:x,y=z,y=z` — the count, and the `TYPE.` spelling.
//!
//! PCGen's reference:
//!
//! ```text
//! PRESKILL:x,y=z,y=z
//!   x: Number (Number of skills)
//!   y: Text (Skill name)   y: TYPE.Text (Skill type)
//!   z: Number (Skill ranks)
//!
//! PRESKILL:1,Spot=10,Listen=10   either, with at least 10 ranks
//! PRESKILL:2,TYPE.Spy=2,TYPE.Spy=2
//!   "two Spy skills with at least two ranks in each of them"
//! ```
//!
//! Only the last `y=z` pair was read, so `PRESKILL:1,Spot=10,Listen=10` tested
//! Listen alone; and `TYPE.` was not understood, so `TYPE.Perform` looked for a
//! skill literally named that. 507 of the RSRD's 1,626 values tested the wrong
//! skill and 204 used a type.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/preskill.lst", 1, "preskill.pcc");
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

/// Is the ability gated on `preskill` active, given these skill ranks?
fn active(preskill: &str, skills: &[(&str, f64)]) -> bool {
    let records = vec![
        record(
            "SKILL",
            "Spot",
            &[("KEYSTAT", "WIS"), ("TYPE", "Wisdom.Standard.Base.Spot")],
        ),
        record(
            "SKILL",
            "Listen",
            &[("KEYSTAT", "WIS"), ("TYPE", "Wisdom.Standard.Base.Listen")],
        ),
        record(
            "SKILL",
            "Perform (Dance)",
            &[
                ("KEYSTAT", "CHA"),
                ("TYPE", "Charisma.Perform.Base.Perform (Dance)"),
            ],
        ),
        record(
            "SKILL",
            "Perform (Oratory)",
            &[
                ("KEYSTAT", "CHA"),
                ("TYPE", "Charisma.Perform.Base.Perform (Oratory)"),
            ],
        ),
        record(
            "SKILL",
            "Spellcraft",
            &[
                ("KEYSTAT", "INT"),
                ("TYPE", "Intelligence.Standard.Base.Spellcraft"),
            ],
        ),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("PRESKILL", preskill),
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
        templates: vec!["Grantor".to_string()],
        skills: skills
            .iter()
            .map(|(name, ranks)| ((*name).to_string(), *ranks))
            .collect::<BTreeMap<_, _>>(),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn a_count_of_one_accepts_either_listed_skill() {
    // `PRESKILL:1,Spot=10,Listen=10` is "either". Reading only the last pair
    // meant a character with Spot but not Listen was wrongly rejected.
    assert!(active("1,Spot=10,Listen=10", &[("Spot", 10.0)]));
    assert!(active("1,Spot=10,Listen=10", &[("Listen", 10.0)]));
    assert!(!active("1,Spot=10,Listen=10", &[("Spot", 9.0)]));
    assert!(!active("1,Spot=10,Listen=10", &[("Spellcraft", 20.0)]));
}

#[test]
fn a_count_of_two_requires_two_of_them() {
    assert!(active(
        "2,Spot=5,Listen=5",
        &[("Spot", 5.0), ("Listen", 5.0)]
    ));
    assert!(!active("2,Spot=5,Listen=5", &[("Spot", 5.0)]));
}

#[test]
fn a_type_requirement_matches_skills_of_that_type() {
    // `TYPE.Perform` must find the Perform skills, not a skill named
    // `TYPE.Perform`. The reference's own example needs two distinct ones.
    assert!(active("1,TYPE.Perform=5", &[("Perform (Dance)", 5.0)]));
    assert!(active(
        "2,TYPE.Perform=5,TYPE.Perform=5",
        &[("Perform (Dance)", 5.0), ("Perform (Oratory)", 5.0)]
    ));
    assert!(!active("1,TYPE.Perform=5", &[("Perform (Dance)", 4.0)]));
    assert!(!active("1,TYPE.Perform=5", &[("Spellcraft", 20.0)]));
}

#[test]
fn the_rank_threshold_is_per_skill() {
    assert!(!active("1,TYPE.Perform=5", &[("Perform (Dance)", 4.9)]));
    assert!(active("1,TYPE.Perform=5", &[("Perform (Dance)", 5.0)]));
}

/// The lowered `PRESKILL` predicate, for asserting on the reading itself.
///
/// The reference splits the part on every `=` and takes the value from the
/// *last* segment, so a name may contain `=`. Splitting on the first `=` left
/// `Knowledge (TYPE=Other)=3` as name `Knowledge (TYPE` plus ranks `Other)=3`,
/// which failed to parse and dropped the requirement.
fn lowered(preskill: &str) -> String {
    let records = vec![record(
        "ABILITY",
        "Gated",
        &[("CATEGORY", "Special Ability"), ("PRESKILL", preskill)],
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

#[test]
fn a_skill_name_may_contain_an_equals_sign() {
    // The reference's own example: three ranks in the skill named
    // `Knowledge (TYPE=Other)`.
    assert_eq!(
        lowered("1,Knowledge (TYPE=Other)=3"),
        "SkillRanks { specs: [SkillRequirement { name: \"Knowledge (TYPE=Other)\", is_type: false, min_ranks: 3 }], min_count: 1 }"
    );
}

#[test]
fn the_equals_spelling_of_a_type_keeps_its_rank() {
    // `TYPE=Perform=8` — the rank is after the last `=`, the key is the `TYPE=`
    // spelling of a skill type.
    assert_eq!(
        lowered("1,TYPE=Perform=8"),
        "SkillRanks { specs: [SkillRequirement { name: \"Perform\", is_type: true, min_ranks: 8 }], min_count: 1 }"
    );
}

#[test]
fn the_equals_spelling_of_a_type_may_omit_the_rank() {
    // `PRESKILL:3,TYPE=Knowledge` — the Draconomicon writes the type with `=`
    // and no rank at all. The reference falls back to the count for the value,
    // so this is three ranks in a Knowledge-type skill.
    assert_eq!(
        lowered("3,TYPE=Knowledge"),
        "SkillRanks { specs: [SkillRequirement { name: \"Knowledge\", is_type: true, min_ranks: 3 }], min_count: 3 }"
    );
}
