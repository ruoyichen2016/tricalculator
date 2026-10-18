use pcgen_core::AdvancementFrame;
use pcgen_engine::{martial::MartialAction, martial_case::MartialCase, CharacterDraft, Engine};
use std::{collections::HashMap, path::Path};
#[test]
fn complete_class_rows_then_shadow_pounce_extremes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let engine = Engine::new(
        pcgen_import::native::compile_database(
            &root.join("data/rules/rainbow-warweaver.rules.json"),
        )
        .unwrap(),
    );
    // Exercise every class level before constructing the forum combination.
    for (class, max) in [
        ("Cleric", 20),
        ("Crusader", 20),
        ("Swordsage", 20),
        ("Swashbuckler", 20),
        ("Crinti Shadow Marauder", 5),
        ("Ruby Knight Vindicator", 10),
    ] {
        assert_eq!(engine.pack.classes[class].max_level, max);
        let magic = engine
            .pack
            .entities
            .iter()
            .find(|e| e.name == class && e.metadata.class_magic.is_some())
            .unwrap()
            .metadata
            .class_magic
            .as_ref()
            .unwrap();
        let mut draft = CharacterDraft {
            race_id: "Human".into(),
            alignment: "LN".into(),
            ..Default::default()
        };
        for n in 1..=max {
            draft
                .advancement
                .push(AdvancementFrame::single(n, class, n, 1, 0));
            let snapshot = engine.evaluate(&draft);
            assert_eq!(
                snapshot.base_attack_bonus,
                engine.pack.progressions[class].bab(n),
                "{class} {n}"
            );
            if let Some(m) = &magic.martial {
                assert!(m.at_level(n).is_some(), "{class} {n}");
            }
            let vars = HashMap::from([("CLASSLEVEL".to_string(), n as i32)]);
            for f in magic.feature_values.iter().filter(|f| f.at_level <= n) {
                for v in f.values.values() {
                    pcgen_formula::Formula::parse(v)
                        .unwrap()
                        .eval_i32(&vars)
                        .unwrap();
                }
            }
        }
    }
    let crusader = engine
        .pack
        .entities
        .iter()
        .find(|e| e.name == "Crusader" && e.metadata.class_magic.is_some())
        .unwrap()
        .metadata
        .class_magic
        .as_ref()
        .unwrap();
    assert_eq!(
        crusader
            .progression
            .iter()
            .find(|r| r.name == "Mettle")
            .unwrap()
            .level,
        13
    );
    let case: MartialCase = serde_json::from_str(
        &std::fs::read_to_string(root.join("tests/fixtures/forum-builds/shadow-pounce.json"))
            .unwrap(),
    )
    .unwrap();
    let report = engine.audit_martial_case(&case);
    assert_eq!(report.base_attack_bonus, 16);
    assert_eq!(report.training.tracks["Crusader"].initiator_level, 14);
    assert_eq!(report.training.tracks["Swordsage"].initiator_level, 15);
    assert!(report
        .training
        .errors
        .iter()
        .any(|e| e.contains("current IL 12")));
    assert!(report
        .build_errors
        .iter()
        .any(|e| e.contains("actual BAB 8")));
    assert!(report
        .action_rules_probe
        .errors
        .iter()
        .any(|e| e.contains("standard action")));
    assert_eq!(
        (
            report.resource_probe.full_attacks,
            report.resource_probe.main_attacks,
            report.resource_probe.off_attacks
        ),
        (13, 65, 39)
    );
    assert_eq!(
        report.resource_probe.main_attack_bonuses,
        vec![35, 35, 30, 25, 20]
    );
    assert_eq!(report.resource_probe.off_attack_bonuses, vec![35, 30, 25]);
    assert_eq!(report.resource_probe.turning_remaining, 0);
    assert_eq!(report.resource_probe.all_hit_average, Some(9282.0));
    let mut d = case.character();
    d.martial_training
        .acquisitions
        .retain(|a| a.maneuver != "Divine Surge, Greater");
    let mut plan = case.plan.clone();
    plan.free_impetus_ruling = Some("Explicit stress-test house rule".into());
    plan.deadly_precision_mic = true;
    assert_eq!(
        engine.replay_martial_turn(&d, &plan).all_hit_average,
        Some(8918.0)
    );
    plan.teleport_distance_feet = 51;
    assert!(engine.replay_martial_turn(&d, &plan).errors[0].contains("50 feet"));
    plan.teleport_distance_feet = 50;
    plan.nightstick = false;
    let short = engine.replay_martial_turn(&d, &plan);
    assert!(!short.errors.is_empty());
    assert_eq!(short.full_attacks, 11);
    plan.nightstick = true;
    plan.teleport_destination_clear = false;
    assert_eq!(engine.replay_martial_turn(&d, &plan).full_attacks, 1);
    plan.teleport_destination_clear = true;
    plan.target_visible_before = false;
    assert_eq!(engine.replay_martial_turn(&d, &plan).full_attacks, 1);
    plan.target_visible_before = true;
    plan.precision_immune = true;
    plan.fire_immune = true;
    assert!(
        engine
            .replay_martial_turn(&d, &plan)
            .all_hit_average
            .unwrap()
            < 3000.0
    );
    plan.actions = vec![MartialAction::Recovery {
        track: "Swordsage".into(),
        maneuver: "Shadow Blink".into(),
    }];
    assert!(engine.replay_martial_turn(&d, &plan).errors[0].contains("not expended"));
    plan.actions = vec![
        MartialAction::Initiate {
            track: "Swordsage".into(),
            maneuver: "Shadow Blink".into(),
        },
        MartialAction::Impetus,
        MartialAction::Initiate {
            track: "Swordsage".into(),
            maneuver: "Shadow Blink".into(),
        },
    ];
    assert!(engine.replay_martial_turn(&d, &plan).errors[0].contains("already expended"));
}
