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
            times_memorized: 0,
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
}

#[test]
fn domain_spell_numeric_effect_is_reported() {
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
        ..CharacterDraft::default()
    });
    assert!(snapshot.domain_spells.contains_key("Sun"));
    assert!(snapshot
        .applied_spell_effects
        .keys()
        .any(|key| key.contains("Bless")));
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
