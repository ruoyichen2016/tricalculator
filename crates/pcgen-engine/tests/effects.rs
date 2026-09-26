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

#[test]
fn creature_traits_are_exposed_in_snapshot() {
    let records = vec![record(
        "RACE",
        "Trait Race",
        "Trait Race",
        &[
            ("VISION", "Darkvision (60')"),
            ("DR", "5/-"),
            ("SR", "13"),
            ("NATURALATTACKS", "Claw"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["creature-traits".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Trait Race".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.vision, vec!["Darkvision (60')"]);
    assert_eq!(snapshot.damage_reduction, vec!["5/-"]);
    assert_eq!(snapshot.spell_resistance, Some(13));
    assert_eq!(snapshot.natural_attacks, vec!["Claw"]);
}

#[test]
fn typed_bonus_stacking_applies_to_saves_skills_and_attack() {
    let records = vec![
        record(
            "RACE",
            "Save Stack",
            "Save Stack",
            &[
                ("BONUS", "SAVE|Fortitude|2|TYPE=Racial"),
                ("BONUS", "SAVE|Fortitude|3|TYPE=Racial"),
            ],
        ),
        record(
            "RACE",
            "Skill Stack",
            "Skill Stack",
            &[
                ("BONUS", "SKILL|Climb|2|TYPE=Competence"),
                ("BONUS", "SKILL|Climb|3|TYPE=Competence"),
            ],
        ),
        record(
            "RACE",
            "Attack Stack",
            "Attack Stack",
            &[
                ("BONUS", "COMBAT|TOHIT|2|TYPE=Morale"),
                ("BONUS", "COMBAT|TOHIT|3|TYPE=Morale"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["typed-bonus".to_string()],
        &records,
    );
    let save = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Save Stack".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(save.saves.get("Fortitude"), Some(&3));

    let skill = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Skill Stack".to_string(),
        skills: std::collections::BTreeMap::from([("Climb".to_string(), 1.0)]),
        ..CharacterDraft::default()
    });
    assert_eq!(skill.skills.get("Climb"), Some(&4));

    let attack = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Attack Stack".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(attack.attack_bonuses.get("COMBAT.TOHIT"), Some(&3));
}

#[test]
fn formula_typed_bonus_keeps_bonus_type_and_stack_key() {
    let records = vec![
        record(
            "RACE",
            "Formula Nonstack",
            "Formula Nonstack",
            &[
                ("BONUS", "SAVE|Fortitude|1+1|TYPE=Racial"),
                ("BONUS", "SAVE|Fortitude|3|TYPE=Racial"),
            ],
        ),
        record(
            "RACE",
            "Formula Stack",
            "Formula Stack",
            &[
                ("BONUS", "SAVE|Fortitude|1+1|TYPE=Racial|STACK=YES"),
                ("BONUS", "SAVE|Fortitude|3|TYPE=Racial|STACK=YES"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["formula-typed-bonus".to_string()],
        &records,
    );

    let nonstack = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Formula Nonstack".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(nonstack.saves.get("Fortitude"), Some(&3));

    let stack = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Formula Stack".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(stack.saves.get("Fortitude"), Some(&5));
}

#[test]
fn class_level_creature_traits_follow_advancement_when_class_levels_absent() {
    let records = vec![
        record("CLASS", "Test Class", "Test Class", &[]),
        record(
            "CLASS",
            "1",
            "Test Class",
            &[("VISION", "Darkvision (60')"), ("UDAM", "1d6")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["class-level-traits".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        advancement: vec![AdvancementFrame::single(1, "Test Class", 1, 0, 0)],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.vision, vec!["Darkvision (60')"]);
    assert_eq!(snapshot.unarmed_damage, vec!["1d6"]);
}

#[test]
fn temp_bonus_applies_only_when_active_state_is_selected() {
    let records = vec![record(
        "RACE",
        "Temp Race",
        "Temp Race",
        &[
            ("TEMPBONUS", "PC|SAVE|Fortitude|2|TYPE=Morale"),
            ("TEMPVALUE", "MIN=1|MAX=5|TITLE=Test"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["temp-state".to_string()],
        &records,
    );

    let inactive = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Temp Race".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(inactive.saves.get("Fortitude"), Some(&0));
    assert!(inactive.active_states.is_empty());

    let active = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Temp Race".to_string(),
        active_states: vec!["Temp Race".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(active.saves.get("Fortitude"), Some(&2));
    assert_eq!(active.active_states, vec!["Temp Race".to_string()]);

    let race = pack
        .entities
        .iter()
        .find(|entity| entity.id.key == "temp_race")
        .expect("temp race entity");
    assert!(
        matches!(
            race.effects
                .first()
                .and_then(|effect| effect.condition.as_ref()),
            Some(pcgen_core::Predicate::ActiveState { .. })
        ),
        "TEMPBONUS should be gated by an ActiveState predicate"
    );
    let temporary_value = race.temporary_value.as_ref().expect("TEMPVALUE parsed");
    assert_eq!(temporary_value.min, Some(1));
    assert_eq!(temporary_value.max, Some(5));
    assert_eq!(temporary_value.title.as_deref(), Some("Test"));
}

#[test]
fn temp_bonus_from_spell_is_activated_by_state() {
    let records = vec![record(
        "SPELL",
        "Bless",
        "Bless",
        &[("TEMPBONUS", "ANYPC|COMBAT|TOHIT|1|TYPE=Morale")],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["spell-temp-state".to_string()],
        &records,
    );

    let inactive = Engine::new(pack.clone()).evaluate(&CharacterDraft::default());
    assert_eq!(inactive.attack_bonuses.get("COMBAT.TOHIT"), None);

    let active = Engine::new(pack).evaluate(&CharacterDraft {
        active_states: vec!["Bless".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(active.attack_bonuses.get("COMBAT.TOHIT"), Some(&1));
}

#[test]
fn equipment_temp_bonus_requires_equipped_type_and_active_state() {
    let records = vec![record(
        "EQUIPMENT",
        "Temp Sword",
        "Temp Sword",
        &[
            ("TYPE", "Weapon"),
            ("TEMPBONUS", "EQ|Weapon|WEAPON|TOHIT|1|TYPE=Enhancement"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["equipment-temp-state".to_string()],
        &records,
    );

    let active_not_equipped = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        active_states: vec!["Temp Sword".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(active_not_equipped.attack_bonuses.get("WEAPON.TOHIT"), None);

    let active_equipped = Engine::new(pack).evaluate(&CharacterDraft {
        equipment: vec!["Temp Sword".to_string()],
        active_states: vec!["Temp Sword".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(active_equipped.attack_bonuses.get("WEAPON.TOHIT"), Some(&1));
}

#[test]
fn precheckbase_gates_grants_by_class_base_save() {
    // A class with a good Will save and a poor Fortitude save.
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            "Cleric",
            &[
                ("BAB", "medium"),
                (
                    "BONUS",
                    "SAVE|BASE.Will|classlevel(\"APPLIEDAS=NONEPIC\")/2+2",
                ),
                (
                    "BONUS",
                    "SAVE|BASE.Fortitude|classlevel(\"APPLIEDAS=NONEPIC\")/3",
                ),
            ],
        ),
        record(
            "RACE",
            "Test Race",
            "Test Race",
            &[
                ("BONUS", "VAR|WillGated|1|PRECHECKBASE:1,Will=2"),
                ("BONUS", "VAR|FortGated|1|PRECHECKBASE:1,Fortitude=2"),
            ],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["precheckbase".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );

    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 3,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    // Will base save at level 3 is 3 (>= 2); Fortitude base save is 1 (< 2).
    assert_eq!(snapshot.variables.get("WillGated").copied(), Some(1));
    assert!(!snapshot.variables.contains_key("FortGated"));
}
