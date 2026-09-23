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
                hit_points_by_level: Vec::new(),
                experience_gained: 0,
            },
            DraftClassLevel {
                class_id: "Wizard".to_string(),
                level: 1,
                hit_points_gained: 0,
                hit_points_by_level: Vec::new(),
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
            hit_points_by_level: Vec::new(),
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

#[test]
fn class_without_explicit_baseab_has_zero_bab() {
    let records = vec![
        record("CLASS", "No Bab", "No Bab", &[("HD", "8")]),
        record("RACE", "Test Race", "Test Race", &[]),
    ];
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["no-bab".to_string()], &records);
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "No Bab".to_string(),
            level: 5,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.base_attack_bonus, 0);
    // No explicit BONUS:SAVE means no base save progression.
    assert_eq!(snapshot.saves.get("Fortitude"), Some(&0));
    assert_eq!(snapshot.saves.get("Reflex"), Some(&0));
    assert_eq!(snapshot.saves.get("Will"), Some(&0));
    // A class with no HD and no rolled HP stays at 0 rather than being
    // forced to the generic level-1 minimum.
    assert_eq!(snapshot.hit_points, 0);
}

#[test]
fn normalized_class_entity_activates_level_bonuses_once() {
    let records = vec![
        record(
            "CLASS",
            "Charlie Class",
            "Charlie Class",
            &[("HD", "10"), ("BONUS", "COMBAT|AC|22|TYPE=Deflection")],
        ),
        record("CLASS", "1", "1", &[("BONUS", "HP|CURRENTMAX|5")]),
        record("RACE", "Test Race", "Test Race", &[]),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["class-activation".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        stats: std::collections::BTreeMap::from([("DEX".to_string(), 10)]),
        class_levels: vec![DraftClassLevel {
            class_id: "Charlie Class".to_string(),
            level: 1,
            hit_points_gained: 10,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    // Header `COMBAT.AC` and level-row `HP.CURRENTMAX` must both be active
    // even though the stable EntityId key is `charlie_class`.
    assert_eq!(snapshot.armor_class, 32);
    assert_eq!(snapshot.touch_ac, 32);
    assert_eq!(snapshot.flat_footed_ac, 32);
    assert_eq!(snapshot.hit_points, 15);
    assert_eq!(snapshot.external_modifiers.get("HP.CURRENTMAX"), Some(&5));
}

#[test]
fn typed_progression_prevents_raw_baseab_and_save_double_counting() {
    let records = vec![
        record(
            "CLASS",
            "Fighter",
            "Fighter",
            &[
                ("HD", "10"),
                ("BONUS", "COMBAT|BASEAB|TL|TYPE=Base.REPLACE"),
                ("BONUS", "SAVE|BASE.Fortitude|2"),
            ],
        ),
        record("RACE", "Test Race", "Test Race", &[]),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["double-count".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 4,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.base_attack_bonus, 4);
    assert_eq!(snapshot.saves.get("Fortitude"), Some(&2));
    assert!(!snapshot
        .external_modifiers
        .keys()
        .any(|target| target.to_ascii_uppercase().contains("BASEAB")));
}

#[test]
fn typed_shield_bonus_does_not_apply_to_touch_ac() {
    let records = vec![record(
        "RACE",
        "Shield Race",
        "Shield Race",
        &[("BONUS", "COMBAT|AC|4|TYPE=Shield")],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["shield-ac".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Shield Race".to_string(),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.armor_class, 14);
    assert_eq!(snapshot.flat_footed_ac, 14);
    assert_eq!(snapshot.touch_ac, 10);
}

#[test]
fn flat_footed_ac_keeps_negative_dex_penalty() {
    let records = vec![record("RACE", "Slow Race", "Slow Race", &[])];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["slow-race".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Slow Race".to_string(),
        stats: std::collections::BTreeMap::from([("DEX".to_string(), 6)]),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.armor_class, 8);
    assert_eq!(snapshot.touch_ac, 8);
    assert_eq!(snapshot.flat_footed_ac, 8);
}

#[test]
fn ac_bonus_stacking_policy_applies_same_type_rules() {
    let records = vec![
        record(
            "RACE",
            "Armor Race",
            "Armor Race",
            &[
                ("BONUS", "COMBAT|AC|2|TYPE=Armor"),
                ("BONUS", "COMBAT|AC|3|TYPE=Armor"),
            ],
        ),
        record(
            "RACE",
            "Dodge Race",
            "Dodge Race",
            &[
                ("BONUS", "COMBAT|AC|2|TYPE=Dodge"),
                ("BONUS", "COMBAT|AC|3|TYPE=Dodge"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["ac-stacking".to_string()],
        &records,
    );
    let armor = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Armor Race".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(armor.armor_class, 13);
    assert_eq!(armor.touch_ac, 10);
    assert_eq!(armor.flat_footed_ac, 13);

    let dodge = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Dodge Race".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(dodge.armor_class, 15);
    assert_eq!(dodge.touch_ac, 15);
    assert_eq!(dodge.flat_footed_ac, 10);
}
