//! Stage 2: equipment alternate profiles / proficiencies and spell residue.

use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    record_key(category, name, name, tags)
}

fn record_key(category: &str, key: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/stage2.lst", 1, "stage2.pcc");
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
fn alternate_weapon_and_armor_metadata_are_typed() {
    let records = vec![
        record(
            "EQUIPMENT",
            "Quarterstaff",
            &[
                ("TYPE", "Weapon.Melee.Simple"),
                ("BASEITEM", "Quarterstaff"),
                ("DAMAGE", "1d6"),
                ("ALTDAMAGE", "1d6"),
                ("ALTCRITMULT", "x2"),
                ("ALTCRITRANGE", "1"),
                ("ALTTYPE", "Weapon.Melee.Bludgeoning"),
                ("ALTEQMOD", "WOOD"),
                ("BASEQTY", "20"),
                ("PROFICIENCY", "WEAPON|Quarterstaff"),
            ],
        ),
        record(
            "EQUIPMOD",
            "Mithral",
            &[("TYPE", "BaseMaterial"), ("ARMORTYPE", "Medium|Light")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["stage2-equipment".to_string()],
        &records,
    );
    let quarterstaff = &pack.equipment["Quarterstaff"];
    assert_eq!(quarterstaff.alternate_damage.as_deref(), Some("1d6"));
    assert_eq!(quarterstaff.alternate_crit_mult.as_deref(), Some("x2"));
    assert_eq!(quarterstaff.alternate_crit_range, Some(1));
    assert!(quarterstaff
        .alternate_types
        .iter()
        .any(|entry| entry == "Bludgeoning"));
    assert!(quarterstaff
        .alternate_equipment_mods
        .iter()
        .any(|entry| entry == "WOOD"));
    assert_eq!(quarterstaff.base_quantity.as_deref(), Some("20"));
    assert_eq!(
        quarterstaff.proficiency.as_deref(),
        Some("WEAPON|Quarterstaff")
    );
    let mithral = &pack.equipment_modifiers["Mithral"];
    assert!(mithral.armor_types.iter().any(|entry| entry == "Medium"));
    for tag in [
        "ALTCRITMULT",
        "ALTCRITRANGE",
        "ALTDAMAGE",
        "ALTEQMOD",
        "ALTTYPE",
        "ARMORTYPE",
        "BASEQTY",
    ] {
        assert!(!report.coverage.legacy_pending_by_tag.contains_key(tag));
    }
}

#[test]
fn preweaponprof_uses_class_weapon_bonus_state() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Fighter",
            &[
                ("HD", "10"),
                ("BAB", "GOOD"),
                ("WEAPONBONUS", "Longsword"),
                ("BONUS", "VAR|HasLongsword|1|PREWEAPONPROF:1,Longsword"),
            ],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["stage2-preweaponprof".to_string()],
        &records,
    );
    assert!(!report
        .coverage
        .legacy_pending_by_tag
        .contains_key("WEAPONBONUS"));
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.variables.get("HasLongsword"), Some(&1));
}

#[test]
fn added_domains_with_pretemplate_condition_reach_snapshot() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record("TEMPLATE", "Vampire", &[]),
        record("DOMAIN", "Sun", &[("SPELLLEVEL", "DOMAIN|Sun=1|Light")]),
        record(
            "DOMAIN",
            "Chaos",
            &[("SPELLLEVEL", "DOMAIN|Chaos=1|Protection from Law")],
        ),
        record(
            "CLASS",
            "Cleric",
            &[
                ("HD", "8"),
                ("ADDDOMAINS", "Sun"),
                ("ADDDOMAINS", "Chaos[PRETEMPLATE:1,Vampire]"),
            ],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["stage2-adddomains".to_string()],
        &records,
    );
    assert!(!report
        .coverage
        .legacy_pending_by_tag
        .contains_key("ADDDOMAINS"));
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        templates: vec!["Vampire".to_string()],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack.clone()).evaluate(&draft);
    assert!(snapshot.domain_spells.contains_key("Sun"));
    assert!(snapshot.domain_spells.contains_key("Chaos"));

    let mut without_template = draft.clone();
    without_template.templates.clear();
    let no_template = Engine::new(pack).evaluate(&without_template);
    assert!(no_template.domain_spells.contains_key("Sun"));
    assert!(!no_template.domain_spells.contains_key("Chaos"));
}

#[test]
fn prohibited_cost_and_donotadd_are_typed_and_visible() {
    let records = vec![
        record("CLASS", "SUBCLASS:Egoist", &[("PROHIBITCOST", "2")]),
        record("CLASS", "Fighter", &[("HD", "10")]),
        record_key(
            "CLASS",
            "Fighter",
            "1",
            &[("DONOTADD", "HITDIE|SKILLPOINTS")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["stage2-prohibited".to_string()],
        &records,
    );
    for tag in ["PROHIBITCOST", "DONOTADD"] {
        assert!(!report.coverage.legacy_pending_by_tag.contains_key(tag));
    }
    assert_eq!(pack.classes["SUBCLASS:Egoist"].prohibited_cost, Some(2));
    assert!(pack.classes["Fighter"]
        .level_do_not_add
        .get(&1)
        .is_some_and(|entries| entries.iter().any(|entry| entry == "HITDIE")));
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft::default());
    assert!(snapshot.prohibited_costs.values().any(|cost| *cost == 2));
}

#[test]
fn spell_variants_are_typed_metadata() {
    let records = vec![record(
        "SPELL",
        "Align Weapon",
        &[
            ("SCHOOL", "Transmutation"),
            ("VARIANTS", "Good|Evil|Lawful|Chaotic"),
        ],
    )];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["stage2-variants".to_string()],
        &records,
    );
    assert!(!report
        .coverage
        .legacy_pending_by_tag
        .contains_key("VARIANTS"));
    let spell = pack
        .spells
        .values()
        .find(|spell| spell.name == "Align Weapon")
        .expect("spell");
    assert!(spell.variants.iter().any(|variant| variant == "Good"));
}

#[test]
fn auto_weaponprof_grant_feeds_preweaponprof_state() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "CLASS",
            "Ranger",
            &[
                ("HD", "10"),
                ("BAB", "GOOD"),
                ("AUTO", "WEAPONPROF|Longbow"),
                ("BONUS", "VAR|HasLongbow|1|PREWEAPONPROF:1,Longbow"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["stage2-autoprof".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: "Ranger".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.variables.get("HasLongbow"), Some(&1));
}
