//! P5-6 / P5-7: real-character assertion baseline and numeric spell effects.

use pcgen_engine::{CharacterDraft, DraftClassLevel, DraftSpell, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/p5-runtime.lst", 1, "p5.pcc");
    RawRecord {
        category: category.to_string(),
        key: name.to_string(),
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
fn numeric_spell_effects_are_reported_in_snapshot() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "SPELL",
            "Mage Armor",
            &[
                ("SCHOOL", "Conjuration"),
                ("LEVEL", "1"),
                ("CLASSES", "Wizard=1"),
                ("BONUS", "COMBAT|AC|4|TYPE=Armor"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-spell-numeric".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Wizard".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        spells: vec![DraftSpell {
            name: "Mage Armor".to_string(),
            level: 1,
            class_id: Some("Wizard".to_string()),
            times_memorized: 1,
            book: None,
        }],
        ..CharacterDraft::default()
    });
    let entry = snapshot
        .applied_spell_effects
        .iter()
        .find(|(key, _)| key.contains("Mage Armor"))
        .expect("mage armor effect");
    assert_eq!(entry.1[0].target, "COMBAT.AC");
    assert_eq!(entry.1[0].value, 4);
    assert_eq!(entry.1[0].spell, "Mage Armor");
    // P5-8: the numeric effect is folded into the derived AC.
    assert_eq!(snapshot.armor_class, 14);
}

#[test]
fn numeric_save_spell_folds_into_saves() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "SPELL",
            "Bless",
            &[
                ("SCHOOL", "Enchantment"),
                ("LEVEL", "1"),
                ("CLASSES", "Cleric=1"),
                ("BONUS", "SAVE|Fortitude,Reflex,Will|1|TYPE=Morale"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-save-spell".to_string()],
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
        spells: vec![DraftSpell {
            name: "Bless".to_string(),
            level: 1,
            class_id: Some("Cleric".to_string()),
            times_memorized: 1,
            book: None,
        }],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.saves.get("Fortitude"), Some(&1));
    assert_eq!(snapshot.saves.get("Reflex"), Some(&1));
    assert_eq!(snapshot.saves.get("Will"), Some(&1));
}

#[test]
fn prepared_domain_spell_numeric_effect_is_reported() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Cleric",
            &[("HD", "8"), ("BAB", "MEDIUM"), ("SPELLSTAT", "WIS")],
        ),
        record("DEITY", "Pelor", &[("ALIGN", "NG"), ("DOMAINS", "Sun")]),
        record("DOMAIN", "Sun", &[("SPELLLEVEL", "DOMAIN|Sun=1|Bless")]),
        record(
            "SPELL",
            "Bless",
            &[
                ("SCHOOL", "Enchantment"),
                ("LEVEL", "1"),
                ("CLASSES", "Cleric=1"),
                ("BONUS", "SAVE|Fortitude,Reflex,Will|1|TYPE=Morale"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-domain-spell".to_string()],
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
        domains: vec!["Sun".to_string()],
        spells: vec![DraftSpell {
            name: "Bless".to_string(),
            level: 1,
            class_id: Some("Cleric".to_string()),
            times_memorized: 1,
            book: None,
        }],
        ..CharacterDraft::default()
    });
    assert!(snapshot.domain_spells.contains_key("Sun"));
    assert!(snapshot
        .applied_spell_effects
        .keys()
        .any(|key| key.contains("Bless")));
    assert_eq!(snapshot.saves.get("Fortitude"), Some(&1));
}

#[test]
fn domain_spell_catalog_alone_does_not_inflate_ac() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Cleric",
            &[("HD", "8"), ("BAB", "MEDIUM"), ("SPELLSTAT", "WIS")],
        ),
        record("DEITY", "Pelor", &[("ALIGN", "NG"), ("DOMAINS", "Sun")]),
        record(
            "DOMAIN",
            "Sun",
            &[("SPELLLEVEL", "DOMAIN|Sun=1|Catalog Only Ward")],
        ),
        record(
            "SPELL",
            "Catalog Only Ward",
            &[
                ("SCHOOL", "Abjuration"),
                ("LEVEL", "1"),
                ("CLASSES", "Cleric=1"),
                ("BONUS", "COMBAT|AC|4|TYPE=Deflection"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-domain-catalog".to_string()],
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
        domains: vec!["Sun".to_string()],
        ..CharacterDraft::default()
    });
    assert!(snapshot.domain_spells.contains_key("Sun"));
    // Catalog-only domain spells must not be applied as active buffs.
    assert_eq!(snapshot.armor_class, 10);
    assert!(snapshot.applied_spell_effects.is_empty());
}

#[test]
fn numeric_stat_and_skill_spells_fold_into_derived_values() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record("SKILL", "Climb", &[("KEYSTAT", "STR")]),
        record(
            "SPELL",
            "Bull's Strength",
            &[
                ("SCHOOL", "Transmutation"),
                ("LEVEL", "2"),
                ("CLASSES", "Wizard=2"),
                ("BONUS", "STAT|STR|4"),
            ],
        ),
        record(
            "SPELL",
            "Bull's Strength Skill",
            &[
                ("SCHOOL", "Transmutation"),
                ("LEVEL", "1"),
                ("CLASSES", "Wizard=1"),
                ("BONUS", "SKILL|Climb|4|TYPE=Competence"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-stat-skill-spell".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        stats: [("STR".to_string(), 10)].into_iter().collect(),
        class_levels: vec![DraftClassLevel {
            class_id: "Wizard".to_string(),
            level: 3,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        skills: [("Climb".to_string(), 0.0)].into_iter().collect(),
        spells: vec![
            DraftSpell {
                name: "Bull's Strength".to_string(),
                level: 2,
                class_id: Some("Wizard".to_string()),
                times_memorized: 1,
                book: None,
            },
            DraftSpell {
                name: "Bull's Strength Skill".to_string(),
                level: 1,
                class_id: Some("Wizard".to_string()),
                times_memorized: 1,
                book: None,
            },
        ],
        ..CharacterDraft::default()
    });
    // Bull's Strength raises STR 10 -> 14.
    assert_eq!(snapshot.stats.get("STR"), Some(&14));
    // Climb ranks 0 + STR(+2) + Jump competence(+4) = 6.
    assert_eq!(snapshot.skills.get("Climb"), Some(&6));
}

#[test]
fn curated_choice_and_pool_state_is_assertable() {
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
        record("DOMAIN", "Sun", &[("SPELLLEVEL", "DOMAIN|Sun=1|Bless")]),
        record(
            "DOMAIN",
            "Healing",
            &[("SPELLLEVEL", "DOMAIN|Healing=1|Cure Light Wounds")],
        ),
        record(
            "ABILITYCATEGORY",
            "Favored Enemy",
            &[
                ("CATEGORY", "Special Ability"),
                ("POOL", "FavoredEnemyCount"),
                ("ABILITYLIST", "Favored Enemy ~ Orc"),
            ],
        ),
        record(
            "RACE",
            "Human",
            &[
                ("DEFINE", "FavoredEnemyCount|0"),
                ("BONUS", "VAR|FavoredEnemyCount|2"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-real-baseline".to_string()],
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
        deity_domains: vec!["Sun".to_string(), "Healing".to_string()],
        abilities: vec![(
            "Special Ability".to_string(),
            "Favored Enemy ~ Orc".to_string(),
        )],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.faith_decisions.len(), 1);
    assert!(snapshot.domain_spells.contains_key("Sun"));
    assert!(snapshot.domain_spells.contains_key("Healing"));
    assert_eq!(snapshot.deity_domains.len(), 2);
    let usage = snapshot
        .ability_pool_usage
        .get("favored_enemy")
        .expect("favored enemy pool");
    assert_eq!(usage.pool, 2);
    assert_eq!(usage.spent, 1);
    assert_eq!(usage.remaining, 1);
    assert_eq!(usage.selection_counts.get("Favored Enemy ~ Orc"), Some(&1));
}

#[test]
fn numeric_attack_spell_folds_into_attack_bonuses() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "SPELL",
            "Bless",
            &[
                ("SCHOOL", "Enchantment"),
                ("LEVEL", "1"),
                ("CLASSES", "Cleric=1"),
                ("BONUS", "COMBAT|TOHIT|1|TYPE=Morale"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["p5-attack-spell".to_string()],
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
        spells: vec![DraftSpell {
            name: "Bless".to_string(),
            level: 1,
            class_id: Some("Cleric".to_string()),
            times_memorized: 1,
            book: None,
        }],
        ..CharacterDraft::default()
    });
    assert!(snapshot.attack_bonuses.values().any(|value| *value == 1));
    assert!(snapshot
        .applied_spell_effects
        .keys()
        .any(|key| key.contains("Bless")));
}
