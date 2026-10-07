use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, ClassProgress, Engine};
use pcgen_ir::lower_records;
use std::collections::BTreeMap;

fn demo_engine() -> Engine {
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["demo".to_string()], &[]);
    let mut progressions = BTreeMap::new();
    progressions.insert(
        "Fighter".to_string(),
        ClassProgress {
            base_attack_bonus: vec![1, 2, 3, 4, 5],
            fortitude: vec![2, 3, 3, 4, 4],
            reflex: vec![0, 0, 1, 1, 1],
            will: vec![0, 0, 1, 1, 1],
            skill_points_per_level: 2,
            first_level_skill_points: 8,
            class_skills: vec!["Climb".to_string()],
            ..ClassProgress::default()
        },
    );
    progressions.insert(
        "Wizard".to_string(),
        ClassProgress {
            base_attack_bonus: vec![0, 1, 1, 2, 2],
            fortitude: vec![0, 0, 1, 1, 1],
            reflex: vec![0, 3, 3, 4, 4],
            will: vec![2, 3, 3, 4, 4],
            skill_points_per_level: 2,
            first_level_skill_points: 8,
            class_skills: vec!["Spellcraft".to_string()],
            ..ClassProgress::default()
        },
    );
    Engine::new(pack).with_progressions(progressions)
}

#[test]
fn gestalt_draft_produces_merged_snapshot() {
    let draft = CharacterDraft {
        id: "d1".to_string(),
        name: "Gestalt".to_string(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        stats: BTreeMap::from([("STR".to_string(), 14), ("INT".to_string(), 14)]),
        advancement: vec![
            AdvancementFrame::new(
                1,
                vec![
                    AdvancementLane::new("a", "Fighter", 1, 10, 0),
                    AdvancementLane::new("b", "Wizard", 1, 4, 0),
                ],
            ),
            AdvancementFrame::single(2, "Fighter", 2, 6, 0),
        ],
        ..CharacterDraft::default()
    };
    let snapshot = demo_engine().evaluate(&draft);
    assert_eq!(snapshot.total_level, 2);
    assert!(snapshot.gestalt.is_some());
    assert_eq!(snapshot.hit_points, 16);
    assert_eq!(snapshot.base_attack_bonus, 2);
}
