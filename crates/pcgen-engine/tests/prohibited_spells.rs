//! `PROHIBITSPELL` — the half of the feature that was never wired up.
//!
//! The raw data sets the variable from a `TYPE:WizardProhibitSchool` ability and
//! gates the class on it. The ability half worked; the prohibition half was
//! parsed and never consulted, so the spell picker offered schools the character
//! had forbidden.

use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/prohibited.lst", 1, "prohibited.pcc");
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

fn wizard_records() -> Vec<RawRecord> {
    vec![
        record(
            "CLASS",
            "Wizard",
            &[
                ("HD", "4"),
                (
                    "PROHIBITSPELL",
                    "SCHOOL.Abjuration|PREVARGTEQ:ProhibitAbjuration,1",
                ),
                (
                    "PROHIBITSPELL",
                    "SCHOOL.Evocation|PREVARGTEQ:ProhibitEvocation,1",
                ),
            ],
        ),
        // "Forbid Abjuration" sets the variable the class gates on.
        record(
            "ABILITY",
            "Forbid Abjuration",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "WizardProhibitSchool"),
                ("DEFINE", "ProhibitAbjuration|0"),
                ("BONUS", "VAR|ProhibitAbjuration|1"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M"), ("MOVE", "Walk,30")]),
    ]
}

fn wizard(draft: &mut CharacterDraft) {
    draft.race_id = "Human".to_string();
    draft.class_levels.push(DraftClassLevel {
        class_id: "Wizard".to_string(),
        level: 1,
        hit_points_gained: 0,
        hit_points_by_level: vec![],
        experience_gained: 0,
    });
}

#[test]
fn a_forbidden_school_is_reported() {
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &wizard_records());
    let engine = Engine::new(pack);

    // Nothing forbidden until the ability is taken: the gate is the variable.
    let mut draft = CharacterDraft::default();
    wizard(&mut draft);
    let snapshot = engine.evaluate(&draft);
    assert!(
        snapshot.prohibited_schools.is_empty(),
        "the prohibition is gated on a variable that is not set yet: {:?}",
        snapshot.prohibited_schools
    );

    // Taking the ability sets it, and the school is reported.
    let mut draft = CharacterDraft::default();
    wizard(&mut draft);
    draft.abilities = vec![(
        "Special Ability".to_string(),
        "Forbid Abjuration".to_string(),
    )];
    let snapshot = engine.evaluate(&draft);
    assert_eq!(
        snapshot.prohibited_schools.iter().collect::<Vec<_>>(),
        vec!["Abjuration"],
        "forbidding Abjuration must be reported"
    );
    assert!(
        !snapshot.prohibited_schools.contains("Evocation"),
        "taking one prohibition must not apply the other"
    );
}

#[test]
fn an_alignment_prohibition_uses_the_deity_alignment() {
    // `ALIGNMENT.Good|PREDEITYALIGN:LE,NE,CE` — a Cleric of an evil deity does
    // not cast Good spells. The character's own alignment is not the question:
    // a Cleric may be one step from their deity, which is why `PREDEITYALIGN`
    // needed its own predicate.
    let mut records = vec![
        record(
            "CLASS",
            "Cleric",
            &[
                ("HD", "8"),
                ("PROHIBITSPELL", "ALIGNMENT.Good|PREDEITYALIGN:LE,NE,CE"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M"), ("MOVE", "Walk,30")]),
        record("DEITY", "Evil God", &[("ALIGN", "LE")]),
        record("DEITY", "Good God", &[("ALIGN", "LG")]),
    ];
    records.push(record(
        "SPELL",
        "Holy Smite",
        &[
            ("SCHOOL", "Evocation"),
            ("DESCRIPTOR", "Good"),
            ("LEVEL", "4"),
        ],
    ));
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let engine = Engine::new(pack);

    let cleric = |deity: &str| {
        let mut draft = CharacterDraft {
            alignment: "LE".to_string(),
            deity: Some(deity.to_string()),
            ..CharacterDraft::default()
        };
        draft.race_id = "Human".to_string();
        draft.class_levels.push(DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: vec![],
            experience_gained: 0,
        });
        engine.evaluate(&draft).prohibited_alignments
    };

    assert_eq!(
        cleric("Evil God").iter().collect::<Vec<_>>(),
        vec!["Good"],
        "a Cleric of an evil deity does not cast Good spells"
    );
    assert!(
        cleric("Good God").is_empty(),
        "the same character serving a good deity has no such prohibition"
    );
}
