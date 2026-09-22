use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, key: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/effects.lst", 1, "effects.pcc");
    RawRecord {
        category: category.to_string(),
        key: key.to_string(),
        name: name.to_string(),
        tags: tags
            .iter()
            .map(|(name, value)| RawTag {
                name: (*name).to_string(),
                value: (*value).to_string(),
                source: source.clone(),
            })
            .collect(),
        source,
        raw: String::new(),
    }
}

#[test]
fn effects_execute_variables_grants_and_choices() {
    let records = vec![
        record(
            "RACE",
            "Test Race",
            "Test Race",
            &[
                ("DEFINE", "Power|1"),
                ("BONUS", "VAR|Derived|Power*2"),
                ("CHOOSE", "SKILL|Climb"),
                ("ABILITY", "Feat|AUTOMATIC|Child"),
            ],
        ),
        record("ABILITY", "Child", "Child", &[("BONUS", "VAR|Granted|5")]),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["effects".to_string()],
        &records,
    );
    assert!(report.coverage.formula_effects >= 1);
    assert!(report.coverage.grant_effects >= 1);
    assert!(report.coverage.choose_effects >= 1);
    assert!(report.coverage.variable_effects >= 1);

    let draft = CharacterDraft {
        id: "effects".to_string(),
        name: "Effects".to_string(),
        race_id: "Test Race".to_string(),
        campaign: "3e".to_string(),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.variables.get("Power"), Some(&1));
    assert_eq!(snapshot.variables.get("Derived"), Some(&2));
    assert_eq!(snapshot.variables.get("Granted"), Some(&5));
    assert!(snapshot
        .active_abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("Child")));
    assert!(snapshot.choices.values().any(|choice| choice == "Climb"));
    assert!(snapshot.applied_effects >= 4);
}

#[test]
fn variable_graph_resolves_dependencies_and_reports_cycles() {
    let records = vec![
        record(
            "RACE",
            "Graph Race",
            "Graph Race",
            &[("DEFINE", "A|B+1"), ("DEFINE", "B|1")],
        ),
        record(
            "RACE",
            "Cycle Race",
            "Cycle Race",
            &[("DEFINE", "C|D+1"), ("DEFINE", "D|C+1")],
        ),
    ];
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["graph".to_string()], &records);
    let draft = CharacterDraft {
        race_id: "Graph Race".to_string(),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack.clone()).evaluate(&draft);
    assert_eq!(snapshot.variables.get("A"), Some(&2));
    assert_eq!(snapshot.variables.get("B"), Some(&1));
    let cycle_draft = CharacterDraft {
        race_id: "Cycle Race".to_string(),
        ..CharacterDraft::default()
    };
    let cycle_snapshot = Engine::new(pack).evaluate(&cycle_draft);
    assert!(cycle_snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("cycle")));
}

#[test]
fn grants_substitute_choices_and_check_embedded_prerequisites() {
    let records = vec![
        record(
            "RACE",
            "Choice Race",
            "Choice Race",
            &[
                ("CHOOSE", "SKILL|Climb"),
                ("ABILITY", "Feat|AUTOMATIC|%CHOICE%"),
                ("ABILITY", "Feat|AUTOMATIC|Blocked"),
            ],
        ),
        record("ABILITY", "Climb", "Climb", &[("BONUS", "VAR|Chosen|1")]),
        record("ABILITY", "Blocked", "Blocked", &[("PRESTAT", "1,STR=20")]),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["choices".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Choice Race".to_string(),
        stats: std::collections::BTreeMap::from([("STR".to_string(), 10)]),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.variables.get("Chosen"), Some(&1));
    assert!(snapshot
        .active_abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("Climb")));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("embedded prerequisites")));
}

#[test]
fn substitution_level_bonus_effects_execute() {
    let records = vec![
        record("CLASS", "Cleric", "Cleric", &[("HD", "8")]),
        record(
            "CLASS",
            "SUBSTITUTIONCLASS:Test Sub",
            "SUBSTITUTIONCLASS:Test Sub",
            &[],
        ),
        record(
            "CLASS",
            "SUBSTITUTIONLEVEL:1",
            "SUBSTITUTIONLEVEL:1",
            &[("BONUS", "VAR|SubVar|1")],
        ),
    ];
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["sub".to_string()], &records);
    let variant = pack
        .variants
        .get("Test Sub ~ level 1")
        .expect("substitution variant should exist");
    assert_eq!(variant.effects.len(), 1);
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        advancement: vec![AdvancementFrame::new(
            1,
            vec![AdvancementLane::new("a", "Cleric", 1, 8, 0)
                .with_substitution_level("Test Sub ~ level 1")],
        )],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.variables.get("SubVar"), Some(&1));
}

#[test]
fn prepclevel_excludes_monster_hd() {
    let records = vec![
        record("CLASS", "Wizard", "Wizard", &[]),
        record("CLASS", "Giant", "Giant", &[("TYPE", "Monster")]),
        record(
            "RACE",
            "Test Race",
            "Test Race",
            &[("BONUS", "VAR|ClassGate|5|PREPCLEVEL:MAX=1")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["effects".to_string()],
        &records,
    );
    assert_eq!(report.coverage.unsupported_tags, 0);

    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: vec![
            DraftClassLevel {
                class_id: "Giant".to_string(),
                level: 3,
                hit_points_gained: 0,
                experience_gained: 0,
            },
            DraftClassLevel {
                class_id: "Wizard".to_string(),
                level: 1,
                hit_points_gained: 0,
                experience_gained: 0,
            },
        ],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack.clone()).evaluate(&draft);
    // Giant is Monster (Racial) HD, so character class levels == 1.
    assert_eq!(snapshot.variables.get("ClassGate"), Some(&5));

    let mut higher = draft.clone();
    higher.class_levels[1].level = 2;
    let snapshot = Engine::new(pack).evaluate(&higher);
    assert_eq!(snapshot.variables.get("ClassGate").copied().unwrap_or(0), 0);
}

fn pretotalab_pack(class_name: &str, class_tags: &[(&str, &str)]) -> pcgen_ir::CompiledRulePack {
    let records = vec![
        record("CLASS", class_name, class_name, class_tags),
        record("ABILITY", "Gated", "Gated", &[("PRETOTALAB", "3")]),
        record(
            "RACE",
            "Test Race",
            "Test Race",
            &[("ABILITY", "Feat|AUTOMATIC|Gated")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["pretotalab".to_string()],
        &records,
    );
    assert_eq!(report.coverage.unsupported_tags, 0);
    pack
}

fn pretotalab_draft(class_name: &str, level: u32) -> CharacterDraft {
    CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: class_name.to_string(),
            level,
            hit_points_gained: 0,
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    }
}

#[test]
fn pretotalab_gates_grants_by_class_bab() {
    let pack = pretotalab_pack("Fighter", &[("BAB", "good")]);
    let level_three = Engine::new(pack.clone()).evaluate(&pretotalab_draft("Fighter", 3));
    assert!(level_three
        .active_abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("Gated")));

    let level_two = Engine::new(pack).evaluate(&pretotalab_draft("Fighter", 2));
    assert!(!level_two
        .active_abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("Gated")));
}

#[test]
fn pretotalab_counts_monster_hd() {
    let pack = pretotalab_pack("Giant", &[("TYPE", "Monster"), ("BAB", "good")]);
    let level_three = Engine::new(pack.clone()).evaluate(&pretotalab_draft("Giant", 3));
    assert!(level_three
        .active_abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("Gated")));

    let level_two = Engine::new(pack).evaluate(&pretotalab_draft("Giant", 2));
    assert!(!level_two
        .active_abilities
        .iter()
        .any(|ability| ability.eq_ignore_ascii_case("Gated")));
}

#[test]
fn pretotalab_bab_modifier_cycle_is_diagnosed() {
    let records = vec![
        record("CLASS", "Fighter", "Fighter", &[("BAB", "good")]),
        record(
            "RACE",
            "Cycle Race",
            "Cycle Race",
            &[("BONUS", "COMBAT|BASEAB|1|PRETOTALAB:3")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["pretotalab-cycle".to_string()],
        &records,
    );
    assert_eq!(report.coverage.unsupported_tags, 0);
    let draft = CharacterDraft {
        race_id: "Cycle Race".to_string(),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("PRETOTALAB_BAB_CYCLE")));
}
