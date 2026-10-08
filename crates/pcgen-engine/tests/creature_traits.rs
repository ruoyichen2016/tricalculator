use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/traits.lst", 1, "traits.pcc");
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

fn dragon_records() -> Vec<RawRecord> {
    vec![record(
        "RACE",
        "Dragon",
        &[
            ("SIZE", "L"),
            ("HANDS", "4"),
            ("MOVE", "Walk,30,Fly,60"),
            ("TEMPLATE", "Dragon"),
            ("BONUS", "VAR|Templated|2|PRETEMPLATE:1,Dragon"),
            ("BONUS", "VAR|ManyHands|2|PREHANDSGTEQ:3"),
            ("BONUS", "VAR|ExactlyTwo|2|PREHANDSEQ:2"),
            ("BONUS", "VAR|Flyer|2|PREMOVE:1,Fly=1"),
            ("BONUS", "VAR|Swimmer|2|PREMOVE:1,Swim=1"),
        ],
    )]
}

fn draft(templates: Vec<String>) -> CharacterDraft {
    CharacterDraft {
        id: "dragon".to_string(),
        name: "Dragon".to_string(),
        race_id: "Dragon".to_string(),
        campaign: "3e".to_string(),
        stats: BTreeMap::new(),
        templates,
        ..CharacterDraft::default()
    }
}

#[test]
fn race_hands_movement_and_templates_gate_predicates() {
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["traits".to_string()],
        &dragon_records(),
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let snapshot = Engine::new(pack).evaluate(&draft(Vec::new()));
    assert_eq!(snapshot.variables.get("Templated").copied(), Some(2));
    assert_eq!(snapshot.variables.get("ManyHands").copied(), Some(2));
    assert_eq!(snapshot.variables.get("Flyer").copied(), Some(2));
    // 4 hands -> exactly-two gate must not fire; 30 ft walk has no swim mode.
    assert!(!snapshot.variables.contains_key("ExactlyTwo"));
    assert!(!snapshot.variables.contains_key("Swimmer"));
}

#[test]
fn draft_applied_template_satisfies_pretemplate() {
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["traits".to_string()],
        &[
            record("RACE", "Human", &[("SIZE", "M")]),
            record(
                "ABILITY",
                "Vampire Trait",
                &[("BONUS", "VAR|Vampiric|2|PRETEMPLATE:1,Vampire")],
            ),
        ],
    );
    // The ability is not active for a plain Human, so no bonus either way.
    let plain = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        id: "h".to_string(),
        name: "H".to_string(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        stats: BTreeMap::new(),
        ..CharacterDraft::default()
    });
    assert!(!plain.variables.contains_key("Vampiric"));
}

#[test]
fn equipped_item_creature_traits_reach_snapshot() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "EQUIPMENT",
            "Reach Boots",
            &[
                ("TYPE", "Boots"),
                ("REACHMULT", "2"),
                ("SR", "13"),
                ("DR", "5/-"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["equipment-traits".to_string()],
        &records,
    );
    let unequipped = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(unequipped.reach_mult, None);
    assert_eq!(unequipped.spell_resistance, None);

    let equipped = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        equipment: vec!["Reach Boots".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(equipped.reach_mult, Some(2));
    assert_eq!(equipped.spell_resistance, Some(13));
    assert_eq!(equipped.damage_reduction, vec!["5/-"]);
}

#[test]
fn equipped_item_equipmod_traits_reach_snapshot() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "EQUIPMENT",
            "Full Plate",
            &[("TYPE", "Armor.Heavy"), ("COST", "1500")],
        ),
        record(
            "EQUIPMOD",
            "Adamantine",
            &[("KEY", "Adamantine"), ("TYPE", "Armor"), ("DR", "3/-")],
        ),
        record(
            "EQUIPMOD",
            "Spell Resistance 13",
            &[("KEY", "SPELL_RES_13"), ("TYPE", "Armor"), ("SR", "13")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["equipmod-traits".to_string()],
        &records,
    );
    let mut modifiers = std::collections::BTreeMap::new();
    modifiers.insert(
        "full plate".to_string(),
        vec!["adamantine".to_string(), "SPELL_RES_13".to_string()],
    );
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        equipment: vec!["Full Plate".to_string()],
        equipment_modifiers: modifiers,
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.damage_reduction, vec!["3/-"]);
    assert_eq!(snapshot.spell_resistance, Some(13));
}

#[test]
fn active_moveclone_expands_movement_modes() {
    let records = vec![record(
        "RACE",
        "Dragon",
        &[
            ("SIZE", "L"),
            ("MOVE", "Walk,30"),
            ("MOVECLONE", "Walk,Fly,*2"),
        ],
    )];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["moveclone".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Dragon".to_string(),
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.movement.get("walk"), Some(&30));
    assert_eq!(snapshot.movement.get("fly"), Some(&60));
}

#[test]
fn companion_mod_sr_and_hd_are_derived() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "COMPANIONMOD",
            "FOLLOWER:FamiliarLVL=11",
            &[("TYPE", "Familiar"), ("SR", "mastervar(\"FamiliarLVL\")+5")],
        ),
        record(
            "COMPANIONMOD",
            "FOLLOWER:SpecialMountLVL=15",
            &[
                ("TYPE", "Special Mount"),
                ("HD", "2"),
                ("SR", "mastervar(\"SpecialMountLVL\")+5"),
                ("ABILITY", "Special Ability|AUTOMATIC|Improved Speed"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["companions".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        companions: vec![
            pcgen_engine::DraftCompanion::new("Familiar", 11),
            pcgen_engine::DraftCompanion::new("Special Mount", 15),
        ],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.companions.len(), 2);
    assert_eq!(snapshot.companions[0].spell_resistance, Some(16));
    let mount = &snapshot.companions[1];
    assert_eq!(mount.spell_resistance, Some(20));
    assert_eq!(mount.hit_dice_bonus, 2);
    assert!(mount.abilities.iter().any(|a| a == "Improved Speed"));
}

#[test]
fn class_and_race_metadata_tags_are_typed() {
    let records = vec![
        record(
            "CLASS",
            "Psion",
            &[
                ("SKILLMULT", "4"),
                ("LEVELSPERFEAT", "3"),
                ("HITDIE", "6"),
                ("EXCLASS", "Ex"),
                ("EXCHANGELEVEL", "Ex"),
            ],
        ),
        record(
            "RACE",
            "Monster",
            &[("XTRASKILLPTSPERLVL", "2"), ("SKILLMULT", "2")],
        ),
    ];
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["meta".to_string()], &records);
    assert_eq!(report.coverage.unsupported_tags, 0);
    let class = pack.classes.get("Psion").expect("class");
    assert_eq!(class.skill_multiplier, Some(4));
    assert_eq!(class.levels_per_feat, Some(3));
    assert_eq!(class.hit_die_override.as_deref(), Some("d6"));
    assert_eq!(class.excluded_class.as_deref(), Some("Ex"));
    let race = pack.races.get("Monster").expect("race");
    assert_eq!(race.extra_skill_points_per_level, Some(2));
    assert_eq!(race.skill_multiplier, Some(2));
}

#[test]
fn companion_rows_with_same_id_are_preserved_not_overwritten() {
    // Real RSRD data reuses `FOLLOWER:FamiliarLVL=1` for several rows.  The
    // first row carries unconditional familiar modifiers; later rows select
    // specific companion races via `PRERACE` + `KIT`.
    let records = vec![
        record(
            "COMPANIONMOD",
            "FOLLOWER:FamiliarLVL=1",
            &[("TYPE", "Familiar"), ("HD", "1")],
        ),
        record(
            "COMPANIONMOD",
            "FOLLOWER:FamiliarLVL=1",
            &[
                ("TYPE", "Familiar"),
                ("HD", "2"),
                ("PRERACE", "1,Companion ~ Bat"),
                ("KIT", "1|Bat Companion"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["companion-rows".to_string()],
        &records,
    );
    let rows = pack
        .companion_mods
        .get("FamiliarLVL=1")
        .expect("grouped companion rows");
    assert_eq!(rows.len(), 2, "duplicate ids must not overwrite each other");
    assert!(rows[1]
        .requirements
        .iter()
        .any(|requirement| requirement.starts_with("PRERACE:")));

    // Only the unconditional row is applied by the current runtime; the
    // conditioned race-selection row is preserved for the companion-selector
    // follow-up and must not accidentally add another +2 HD.
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        companions: vec![pcgen_engine::DraftCompanion::new("Familiar", 1)],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.companions[0].hit_dice_bonus, 1);
}

#[test]
fn companion_condition_matching_applies_hd_sr_abilities_traits_and_bonuses() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "COMPANIONMOD",
            "FOLLOWER:FamiliarLVL=1",
            &[
                ("TYPE", "Familiar"),
                ("HD", "1"),
                ("SR", "mastervar(\"FamiliarLVL\")+5"),
                ("ABILITY", "Special Ability|AUTOMATIC|Alertness"),
                ("BONUS", "COMBAT|AC|2|TYPE=NaturalArmor"),
                ("BONUS", "STAT|DEX|2"),
                (
                    "NATURALATTACKS",
                    "Claw,Weapon.Natural.Melee.Piercing.Slashing,*1,1d4",
                ),
            ],
        ),
        record(
            "COMPANIONMOD",
            "FOLLOWER:FamiliarLVL=1",
            &[
                ("TYPE", "Familiar"),
                ("PRERACE", "1,Companion ~ Bat"),
                ("ABILITY", "Special Ability|AUTOMATIC|Bat Traits"),
                ("BONUS", "STAT|STR|1"),
            ],
        ),
        record(
            "COMPANIONMOD",
            "FOLLOWER:FamiliarLVL=1",
            &[
                ("TYPE", "Familiar"),
                ("PRERACE", "1,Companion ~ Cat"),
                ("BONUS", "STAT|DEX|5"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["companion-runtime".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        companions: vec![
            pcgen_engine::DraftCompanion::new("Familiar", 1).with_race("Companion ~ Bat")
        ],
        ..CharacterDraft::default()
    });
    let companion = &snapshot.companions[0];
    assert_eq!(companion.hit_dice_bonus, 1);
    assert_eq!(companion.spell_resistance, Some(6));
    assert_eq!(companion.armor_class_bonus, 2);
    assert_eq!(companion.stat_adjustments.get("DEX"), Some(&2));
    assert_eq!(companion.stat_adjustments.get("STR"), Some(&1));
    assert!(companion
        .abilities
        .iter()
        .any(|ability| ability == "Alertness"));
    assert!(companion
        .abilities
        .iter()
        .any(|ability| ability == "Bat Traits"));
    assert!(companion
        .traits
        .natural_attacks
        .iter()
        .any(|attack| attack.name.contains("Claw")));
}

#[test]
fn damage_reduction_gates_are_evaluated() {
    // `5/Magic|PRELEVEL:MIN=4` is a value plus a gate. While the field was a
    // `String` the gate was never evaluated, so the snapshot exposed the raw
    // PCGen text and reported both entries regardless of level:
    //   ["10/Magic|PRELEVEL:MIN=12", "5/Magic|PRELEVEL:MIN=4"]
    let records = vec![
        record(
            "TEMPLATE",
            "Celestial",
            &[
                ("DR", "5/Magic|PRELEVEL:MIN=4"),
                ("DR", "10/Magic|PRELEVEL:MIN=12"),
            ],
        ),
        record("TEMPLATE", "Skeleton", &[("DR", "5/Bludgeoning")]),
        record("RACE", "Human", &[("SIZE", "M"), ("MOVE", "Walk,30")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let engine = Engine::new(pack);

    // Both Celestial entries are gated at level 4 and 12, so at level 1
    // neither applies — and no raw PCGen text may reach the snapshot.
    let celestial = engine.evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        templates: vec!["Celestial".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(
        celestial.damage_reduction,
        Vec::<String>::new(),
        "a level-1 Celestial has no damage reduction yet"
    );

    // An ungated entry still comes through, as its value alone.
    let skeleton = engine.evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        templates: vec!["Skeleton".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(skeleton.damage_reduction, vec!["5/Bludgeoning"]);
}
