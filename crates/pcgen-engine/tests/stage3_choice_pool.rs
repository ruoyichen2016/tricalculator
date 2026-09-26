//! Stage 3: ability pools, stack/multi metadata and deterministic choices.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/stage3.lst", 1, "stage3.pcc");
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
fn ability_and_ability_category_are_typed() {
    let records = vec![
        record(
            "ABILITY",
            "Skill Focus",
            &[
                ("CATEGORY", "FEAT"),
                ("STACK", "NO"),
                ("MULT", "YES"),
                ("CHOOSE", "SKILL|Climb"),
            ],
        ),
        record(
            "ABILITYCATEGORY",
            "Skill Focus Pool",
            &[
                ("CATEGORY", "FEAT"),
                ("ABILITYLIST", "Skill Focus|Weapon Focus"),
                ("POOL", "SkillPool"),
            ],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["stage3-abilities".to_string()],
        &records,
    );
    for tag in ["ABILITYLIST", "POOL", "STACK", "CHOICE"] {
        assert!(!report.coverage.legacy_pending_by_tag.contains_key(tag));
    }
    let ability = pack
        .abilities
        .values()
        .find(|ability| ability.name == "Skill Focus")
        .expect("typed ability");
    assert_eq!(ability.category.as_deref(), Some("FEAT"));
    assert_eq!(ability.stack, Some(false));
    assert_eq!(ability.multi, Some(true));
    assert_eq!(ability.choose.as_deref(), Some("SKILL|Climb"));

    let category = pack
        .ability_categories
        .values()
        .find(|category| category.name == "Skill Focus Pool")
        .expect("typed ability category");
    assert_eq!(category.pool.as_deref(), Some("SkillPool"));
    assert!(category
        .ability_list
        .iter()
        .any(|entry| entry == "Skill Focus"));
}

#[test]
fn ability_pool_points_and_list_reach_snapshot() {
    let records = vec![
        record(
            "RACE",
            "Human",
            &[
                ("SIZE", "M"),
                ("DEFINE", "SkillPool|0"),
                ("BONUS", "VAR|SkillPool|3"),
            ],
        ),
        record(
            "ABILITYCATEGORY",
            "Skill Focus Pool",
            &[
                ("CATEGORY", "FEAT"),
                ("ABILITYLIST", "Skill Focus|Weapon Focus"),
                ("POOL", "SkillPool"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["stage3-pools".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        ..CharacterDraft::default()
    });
    let category_id = "skill_focus_pool";
    assert_eq!(snapshot.ability_pools.get(category_id), Some(&3));
    assert!(snapshot
        .ability_lists
        .get(category_id)
        .is_some_and(|members| members.iter().any(|member| member == "Skill Focus")));
}

#[test]
fn choices_resolve_from_draft_and_unresolved_choices_are_diagnosed() {
    let records = vec![record(
        "RACE",
        "Choice Race",
        &[
            ("SIZE", "M"),
            ("CHOOSE", "SKILL|LIST"),
            ("CHOOSE", "SPELLS|SPELLTYPE=Psionic[KNOWN=NO]"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["stage3-choices".to_string()],
        &records,
    );
    let mut choices = BTreeMap::new();
    choices.insert("SKILL|LIST".to_string(), "Climb".to_string());
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Choice Race".to_string(),
        choices,
        ..CharacterDraft::default()
    });
    assert_eq!(
        snapshot.choices.get("SKILL|LIST"),
        Some(&"Climb".to_string())
    );
    assert!(snapshot
        .unresolved_choices
        .iter()
        .any(|spec| spec == "SPELLS|SPELLTYPE=Psionic[KNOWN=NO]"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("unresolved choice")));
}

#[test]
fn subclass_choice_and_repeatable_ability_metadata_reach_snapshot() {
    let records = vec![
        record(
            "CLASS",
            "SUBCLASS:Abjurer",
            &[("CHOICE", "SCHOOL|Abjuration")],
        ),
        record(
            "ABILITY",
            "Favored Enemy Bonus",
            &[
                ("CATEGORY", "Special Ability"),
                ("STACK", "YES"),
                ("MULT", "YES"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["stage3-subclass".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft::default());
    assert_eq!(
        snapshot.class_choices.get("SUBCLASS:Abjurer"),
        Some(&"SCHOOL|Abjuration".to_string())
    );
    assert!(snapshot
        .repeatable_abilities
        .iter()
        .any(|ability| ability == "favored_enemy_bonus"));
}
