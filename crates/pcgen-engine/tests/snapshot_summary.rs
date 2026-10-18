use pcgen_engine::{CharacterDraft, Engine};
use pcgen_import::compile_pcc;
use pcgen_import::PcgClassLevel as DraftClassLevel;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn mini_fighter_snapshot_matches_expected_core_values() {
    let pcc =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaign-mini/mini.pcc");
    if !pcc.exists() {
        eprintln!("skipping snapshot test: {} does not exist", pcc.display());
        return;
    }
    let (pack, _) = compile_pcc(&pcc).expect("mini campaign should compile");
    let draft = CharacterDraft {
        id: "mini".to_string(),
        name: "Mini Fighter".to_string(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        stats: BTreeMap::from([
            ("STR".to_string(), 14),
            ("DEX".to_string(), 12),
            ("CON".to_string(), 14),
            ("INT".to_string(), 10),
            ("WIS".to_string(), 10),
            ("CHA".to_string(), 10),
        ]),
        advancement: pcgen_import::advancement_from_class_levels(&vec![DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 2,
            hit_points_gained: 16,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }]),
        skills: BTreeMap::from([("Climb".to_string(), 4.0)]),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.total_level, 2);
    assert_eq!(snapshot.hit_points, 20);
    assert_eq!(snapshot.base_attack_bonus, 2);
    assert_eq!(snapshot.saves["Fortitude"], 2);
    assert_eq!(snapshot.skills["Climb"], 6);
    assert!(snapshot.armor_class >= 10);
}
