//! Final P4 closure: residual traits, language/subclass state, pools and
//! bounded numeric tails.

use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record_key(category: &str, key: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/p4-close.lst", 1, "p4.pcc");
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

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    record_key(category, name, name, tags)
}

#[test]
fn residual_trait_tags_are_consumed_and_visible() {
    let records = vec![record(
        "RACE",
        "Dragon",
        &[
            ("SIZE", "L"),
            ("MOVE", "Walk,30"),
            ("REACH", "10"),
            ("MOVECLONE", "Walk,Fly,*2"),
            ("ATTACKCYCLE", "BAB,+5"),
        ],
    )];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["p4-residual".to_string()],
        &records,
    );
    for tag in ["REACH", "MOVECLONE", "ATTACKCYCLE"] {
        assert!(
            !report.coverage.legacy_pending_by_tag.contains_key(tag)
                && !report.coverage.unsupported_pending_by_tag.contains_key(tag),
            "{tag} should be consumed"
        );
    }
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Dragon".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.reach_feet, Some(10));
    assert_eq!(snapshot.movement.get("fly"), Some(&60));
    assert!(snapshot.attack_cycle.iter().any(|cycle| cycle == "BAB,+5"));
}

#[test]
fn language_state_feeds_prelang() {
    let records = vec![record(
        "RACE",
        "Polyglot",
        &[
            ("SIZE", "M"),
            ("LANGBONUS", "Dwarven,Elven"),
            ("AUTO", "LANG|Common"),
            ("BONUS", "VAR|SpeaksElven|1|PRELANG:1,Elven"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p4-language".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Polyglot".to_string(),
        ..CharacterDraft::default()
    });
    assert!(snapshot
        .languages
        .iter()
        .any(|language| language == "common"));
    assert!(snapshot
        .languages
        .iter()
        .any(|language| language == "dwarven"));
    assert_eq!(snapshot.variables.get("SpeaksElven"), Some(&1));
}

#[test]
fn subclass_state_feeds_presubclass() {
    let records = vec![record(
        "RACE",
        "Specialist",
        &[
            ("SIZE", "M"),
            ("BONUS", "VAR|IsAbjurer|1|PRESUBCLASS:1,Abjurer"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p4-subclass".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Specialist".to_string(),
        advancement: vec![AdvancementFrame::new(
            1,
            vec![AdvancementLane::new("a", "Wizard", 1, 4, 0).with_subclass("Abjurer")],
        )],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.variables.get("IsAbjurer"), Some(&1));
}

#[test]
fn ability_pool_usage_tracks_selected_members() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "ABILITYCATEGORY",
            "Skill Pool",
            &[
                ("CATEGORY", "FEAT"),
                ("POOL", "SkillPool"),
                ("ABILITYLIST", "Skill Focus|Weapon Focus"),
            ],
        ),
        record(
            "RACE",
            "Human",
            &[("DEFINE", "SkillPool|0"), ("BONUS", "VAR|SkillPool|3")],
        ),
    ];
    // Two Human records intentionally exercise patch-free merge; the last one
    // does not overwrite the first in `lower_records` coverage and pool logic.
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p4-pool".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        abilities: vec![
            ("FEAT".to_string(), "Skill Focus".to_string()),
            ("FEAT".to_string(), "Weapon Focus".to_string()),
        ],
        ..CharacterDraft::default()
    });
    let usage = snapshot
        .ability_pool_usage
        .get("skill_pool")
        .expect("pool usage");
    assert_eq!(usage.pool, 3);
    assert_eq!(usage.spent, 2);
    assert_eq!(usage.remaining, 1);
    assert_eq!(usage.selected.len(), 2);
    assert_eq!(usage.selection_counts.get("Skill Focus"), Some(&1));
    assert_eq!(usage.selection_counts.get("Weapon Focus"), Some(&1));
}

#[test]
fn duplicate_selection_without_mult_is_diagnosed() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "RACE",
            "Human",
            &[("DEFINE", "TinyPool|0"), ("BONUS", "VAR|TinyPool|3")],
        ),
        record(
            "ABILITYCATEGORY",
            "Tiny Pool",
            &[
                ("CATEGORY", "FEAT"),
                ("POOL", "TinyPool"),
                ("ABILITYLIST", "Skill Focus"),
            ],
        ),
        record(
            "ABILITY",
            "Skill Focus",
            &[("CATEGORY", "FEAT"), ("MULT", "NO")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-mult".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        abilities: vec![
            ("FEAT".to_string(), "Skill Focus".to_string()),
            ("FEAT".to_string(), "Skill Focus".to_string()),
        ],
        ..CharacterDraft::default()
    });
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("without MULT")));
}

#[test]
fn arcane_prestige_class_stacks_caster_level() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Wizard",
            &[
                ("HD", "4"),
                ("SPELLSTAT", "INT"),
                ("MEMORIZE", "YES"),
                ("BAB", "HALF"),
                ("FACT", "SpellType|Arcane"),
            ],
        ),
        record(
            "CLASS",
            "PRC Caster",
            &[
                ("HD", "4"),
                ("SPELLSTAT", "INT"),
                ("MEMORIZE", "YES"),
                ("BAB", "HALF"),
                ("TYPE", "PC.Prestige"),
            ],
        ),
        record_key("CLASS", "PRC Caster", "1", &[("ADD", "SPELLCASTER|Arcane")]),
        record_key("CLASS", "PRC Caster", "2", &[("ADD", "SPELLCASTER|Arcane")]),
        record_key("CLASS", "PRC Caster", "3", &[("ADD", "SPELLCASTER|Arcane")]),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p4-caster".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        stats: [("INT".to_string(), 18)].into_iter().collect(),
        class_levels: vec![
            DraftClassLevel {
                class_id: "Wizard".to_string(),
                level: 5,
                hit_points_gained: 0,
                hit_points_by_level: Vec::new(),
                experience_gained: 0,
            },
            DraftClassLevel {
                class_id: "PRC Caster".to_string(),
                level: 3,
                hit_points_gained: 0,
                hit_points_by_level: Vec::new(),
                experience_gained: 0,
            },
        ],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.caster_levels.get("Wizard"), Some(&8));
    assert!(!snapshot.caster_levels.contains_key("PRC Caster"));
}

#[test]
fn monster_hd_uses_base_hit_die_average() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Animal",
            &[("HD", "8"), ("TYPE", "Monster"), ("BAB", "MEDIUM")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p4-monster-hd".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Animal".to_string(),
            level: 2,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    });
    // d8 average is 5 per level, CON 10 has no modifier.
    assert_eq!(snapshot.hit_points, 10);
}

#[test]
fn applied_to_choice_resolves_entity_context() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "ABILITY",
            "Expanded Knowledge",
            &[
                ("CATEGORY", "FEAT"),
                ("CHOOSE", "SPELLS|SPELLTYPE=Psionic[KNOWN=NO]"),
            ],
        ),
        record(
            "RACE",
            "Choiceful",
            &[
                ("SIZE", "M"),
                ("ABILITY", "FEAT|AUTOMATIC|Expanded Knowledge"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-appliedto".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Choiceful".to_string(),
        choices: [(
            "Expanded Knowledge".to_string(),
            "Animal Affinity".to_string(),
        )]
        .into_iter()
        .collect(),
        ..CharacterDraft::default()
    });
    assert_eq!(
        snapshot.choices.get("SPELLS|SPELLTYPE=Psionic[KNOWN=NO]"),
        Some(&"Animal Affinity".to_string())
    );
    assert!(snapshot.unresolved_choices.is_empty());
}

#[test]
fn class_subclasses_map_feeds_presubclass() {
    let records = vec![record(
        "RACE",
        "Specialist",
        &[
            ("SIZE", "M"),
            ("BONUS", "VAR|IsTelepath|1|PRESUBCLASS:1,Telepath"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-subclass-map".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Specialist".to_string(),
        class_subclasses: [("Psion".to_string(), "Telepath".to_string())]
            .into_iter()
            .collect(),
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.variables.get("IsTelepath"), Some(&1));
}

#[test]
fn class_choice_filters_resolve_dynamic_class_lists() {
    let records = vec![
        record(
            "RACE",
            "Chooser Race",
            &[
                ("SIZE", "M"),
                ("CHOOSE", "CLASS|ALL"),
                ("CHOOSE", "CLASS|PC"),
            ],
        ),
        record("CLASS", "Fighter", &[("HD", "10"), ("BAB", "GOOD")]),
        record(
            "CLASS",
            "Wizard",
            &[
                ("HD", "4"),
                ("SPELLSTAT", "INT"),
                ("MEMORIZE", "YES"),
                ("FACT", "SpellType|Arcane"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-class-filter".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Chooser Race".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    });
    assert_eq!(
        snapshot.choices.get("CLASS|ALL").map(String::as_str),
        Some("Fighter,Wizard")
    );
    assert_eq!(
        snapshot.choices.get("CLASS|PC").map(String::as_str),
        Some("Fighter")
    );
}

#[test]
fn pcg_deity_domains_feed_divine_tracks() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Cleric",
            &[("HD", "8"), ("BAB", "MEDIUM"), ("SPELLSTAT", "WIS")],
        ),
        record(
            "DEITY",
            "Pelor",
            &[("ALIGN", "NG"), ("DOMAINS", "Sun|Healing")],
        ),
        record("DOMAIN", "Sun", &[("SPELLLEVEL", "DOMAIN|Sun=1|Light")]),
        record(
            "DOMAIN",
            "Healing",
            &[("SPELLLEVEL", "DOMAIN|Healing=1|Cure Light Wounds")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-divine".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        deity: Some("Pelor".to_string()),
        domains: vec!["Sun".to_string(), "Healing".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.faith_decisions.len(), 1);
    assert!(snapshot.domain_spells.contains_key("Sun"));
    assert!(snapshot.domain_spells.contains_key("Healing"));
}

#[test]
fn placeholder_choice_binds_to_source_entity() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "ABILITY",
            "Alpha Chooser",
            &[
                ("CATEGORY", "FEAT"),
                ("CHOOSE", "SKILL|LIST"),
                ("BONUS", "VAR|PickedAlpha|1"),
                ("DESC", "%CHOICE%"),
            ],
        ),
        record(
            "ABILITY",
            "Beta Chooser",
            &[
                ("CATEGORY", "FEAT"),
                ("CHOOSE", "SKILL|LIST"),
                ("BONUS", "VAR|PickedBeta|1"),
                ("DESC", "%CHOICE%"),
            ],
        ),
        record(
            "RACE",
            "Two Choosers",
            &[
                ("SIZE", "M"),
                ("ABILITY", "FEAT|AUTOMATIC|Alpha Chooser"),
                ("ABILITY", "FEAT|AUTOMATIC|Beta Chooser"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-binding".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Two Choosers".to_string(),
        choices: [
            ("Alpha Chooser".to_string(), "Climb".to_string()),
            ("Beta Chooser".to_string(), "Jump".to_string()),
        ]
        .into_iter()
        .collect(),
        ..CharacterDraft::default()
    });
    assert_eq!(
        snapshot.choice_bindings.get("alpha_chooser"),
        Some(&vec!["Climb".to_string()])
    );
    assert_eq!(
        snapshot.choice_bindings.get("beta_chooser"),
        Some(&vec!["Jump".to_string()])
    );
}

#[test]
fn overspent_ability_pool_is_diagnosed() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "RACE",
            "Human",
            &[("DEFINE", "TinyPool|0"), ("BONUS", "VAR|TinyPool|1")],
        ),
        record(
            "ABILITYCATEGORY",
            "Tiny Pool",
            &[
                ("CATEGORY", "FEAT"),
                ("POOL", "TinyPool"),
                ("ABILITYLIST", "Skill Focus|Weapon Focus"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-overspend".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        abilities: vec![
            ("FEAT".to_string(), "Skill Focus".to_string()),
            ("FEAT".to_string(), "Weapon Focus".to_string()),
        ],
        ..CharacterDraft::default()
    });
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("overspent")));
}

#[test]
fn nested_chosen_template_is_activated_in_snapshot() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record("TEMPLATE", "Base Template", &[("CHOOSE", "TEMPLATE|ALL")]),
        record(
            "TEMPLATE",
            "Nested Template",
            &[("BONUS", "VAR|NestedChosen|1")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-nested-template".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        templates: vec!["Base Template".to_string(), "Nested Template".to_string()],
        choices: [("Base Template".to_string(), "Nested Template".to_string())]
            .into_iter()
            .collect(),
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.variables.get("NestedChosen"), Some(&1));
}

#[test]
fn selected_domain_is_validated_against_deity_domain_catalog() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Cleric",
            &[("HD", "8"), ("BAB", "MEDIUM"), ("SPELLSTAT", "WIS")],
        ),
        record(
            "DOMAIN",
            "Luck",
            &[("SPELLLEVEL", "DOMAIN|Luck=1|True Strike")],
        ),
        record(
            "DOMAIN",
            "Chaos",
            &[("SPELLLEVEL", "DOMAIN|Chaos=1|Protection from Law")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-deity-catalog".to_string()],
        &records,
    );
    // Deity is unknown, but the `.pcg` catalog says only Chaos/Luck are allowed.
    let allowed = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        deity: Some("Haela Brightaxe".to_string()),
        domains: vec!["Chaos".to_string(), "Luck".to_string()],
        deity_domains: vec!["Chaos".to_string(), "Luck".to_string()],
        ..CharacterDraft::default()
    });
    assert!(allowed.domain_spells.contains_key("Chaos"));
    assert!(allowed.domain_spells.contains_key("Luck"));

    let denied = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        deity: Some("Haela Brightaxe".to_string()),
        domains: vec!["Luck".to_string()],
        deity_domains: vec!["Chaos".to_string()],
        ..CharacterDraft::default()
    });
    assert!(denied
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("is not granted by deity")));
}
