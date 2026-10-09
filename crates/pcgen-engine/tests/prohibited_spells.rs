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
        snapshot.prohibited_spells.is_empty(),
        "the prohibition is gated on a variable that is not set yet: {:?}",
        snapshot.prohibited_spells
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
        snapshot.prohibited_spells.len(),
        1,
        "forbidding Abjuration must be reported once: {:?}",
        snapshot.prohibited_spells
    );
    assert_eq!(
        snapshot.prohibited_spells[0].target,
        pcgen_core::ProhibitionTarget::School("Abjuration".to_string())
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
        engine.evaluate(&draft).prohibited_spells
    };

    assert_eq!(
        cleric("Evil God").len(),
        1,
        "a Cleric of an evil deity does not cast Good spells"
    );
    assert!(
        cleric("Good God").is_empty(),
        "the same character serving a good deity has no such prohibition"
    );
}

#[test]
fn every_documented_target_kind_is_kept() {
    // PCGen's tag reference lists ALIGNMENT, DESCRIPTOR, SCHOOL, SUBSCHOOL and
    // SPELL. I first modelled only the first and third, which silently dropped
    // 23 `SPELL.*` and 11 `DESCRIPTOR.*` entries — every one of them in
    // Unearthed Arcana and other non-RSRD data.
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            &[
                ("HD", "8"),
                ("PROHIBITSPELL", "ALIGNMENT.Good"),
                ("PROHIBITSPELL", "DESCRIPTOR.Fear"),
                ("PROHIBITSPELL", "SCHOOL.Illusion"),
                ("PROHIBITSPELL", "SUBSCHOOL.Ritual"),
                ("PROHIBITSPELL", "SPELL.Searing Light,Sunbeam,Sunburst"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let cleric = pack.classes.get("Cleric").expect("Cleric");
    assert_eq!(
        cleric.prohibited_spells.len(),
        5,
        "no target kind may be dropped: {:?}",
        cleric.prohibited_spells
    );
}

#[test]
fn a_multi_name_target_is_a_conjunction() {
    // `ALIGNMENT.Chaotic.Evil` prohibits spells that have *both* descriptors,
    // not either — which is why the snapshot keeps the structured entries
    // instead of flattening them to a set of names.
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            &[("HD", "8"), ("PROHIBITSPELL", "ALIGNMENT.Chaotic.Evil")],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let entry = pack.classes["Cleric"].prohibited_spells[0].clone();
    assert_eq!(
        entry.target,
        pcgen_core::ProhibitionTarget::Alignment(vec!["Chaotic".to_string(), "Evil".to_string()])
    );

    let spell = |descriptors: &[&str]| {
        let mut definition = pcgen_core::domain::SpellDefinition::new("x", "X");
        definition.descriptors = descriptors.iter().map(|d| d.to_string()).collect();
        definition
    };
    assert!(entry.covers(&spell(&["Chaotic", "Evil"])), "both present");
    assert!(!entry.covers(&spell(&["Chaotic"])), "only one present");
    assert!(!entry.covers(&spell(&["Good"])), "neither present");
}

#[test]
fn a_spell_target_is_a_disjunction() {
    // `SPELL.A,B` is a comma-delimited list of names, and any one of them is
    // enough.
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            &[
                ("HD", "8"),
                ("PROHIBITSPELL", "SPELL.Searing Light,Sunbeam"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let entry = pack.classes["Cleric"].prohibited_spells[0].clone();
    let named = |name: &str| pcgen_core::domain::SpellDefinition::new(name, name);
    assert!(entry.covers(&named("Sunbeam")));
    assert!(entry.covers(&named("Searing Light")));
    assert!(!entry.covers(&named("Bless")));
}

#[test]
fn adddomains_splits_on_the_pipe() {
    // `ADDDOMAINS:x|x` is a list of domains, and `[...]` qualifies the one
    // before it. Read as a single value, `Travel|Sun` became a domain literally
    // named `Travel|Sun`, which matched nothing, so neither domain arrived.
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            &[("HD", "8"), ("ADDDOMAINS", "Travel|Sun")],
        ),
        record(
            "CLASS",
            "Vampire Cleric",
            &[
                ("HD", "8"),
                ("ADDDOMAINS", "Sun[PRETEMPLATE:1,Vampire]|Law"),
            ],
        ),
        record("RACE", "Human", &[("SIZE", "M")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);

    let names: Vec<&str> = pack.classes["Cleric"]
        .added_domains
        .iter()
        .map(|entry| entry.domain.as_str())
        .collect();
    assert_eq!(names, vec!["Travel", "Sun"], "one entry per domain");

    // The condition stays attached to the domain it follows, not to the list.
    let vampiric = &pack.classes["Vampire Cleric"].added_domains;
    assert_eq!(vampiric.len(), 2);
    assert_eq!(vampiric[0].domain, "Sun");
    assert!(vampiric[0].condition.is_some(), "Sun keeps its gate");
    assert_eq!(vampiric[1].domain, "Law");
    assert!(
        vampiric[1].condition.is_none(),
        "Law is unconditional, and must not inherit Sun's gate"
    );
}
