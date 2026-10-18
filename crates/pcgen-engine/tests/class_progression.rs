//! P4-CLASS-2: class progression consumes SKILLMULT / LEVELSPERFEAT /
//! HITDIE / ADDLEVEL / EXCLASS / EXCHANGELEVEL.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_import::PcgClassLevel as DraftClassLevel;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/class-progression.lst", 1, "class.pcc");
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

fn class_records() -> Vec<RawRecord> {
    vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record("TEMPLATE", "Monster Template", &[("ADDLEVEL", "Fighter|2")]),
        record(
            "CLASS",
            "Fighter",
            &[
                ("HD", "10"),
                ("BAB", "GOOD"),
                ("STARTSKILLPTS", "2"),
                ("SKILLMULT", "3"),
                ("LEVELSPERFEAT", "2"),
                ("HITDIE", "12"),
                ("EXCLASS", "Ex Fighter"),
                ("EXCHANGELEVEL", "Ex Paladin|11|10|1"),
            ],
        ),
    ]
}

#[test]
fn progression_fields_are_typed_into_runtime_table() {
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["class-progression".to_string()],
        &class_records(),
    );
    let progression = pack
        .progressions
        .get("Fighter")
        .expect("fighter progression");
    assert_eq!(progression.first_level_skill_points, 6);
    assert_eq!(progression.skill_multiplier, 3);
    assert_eq!(progression.hit_die, "d12");
    assert_eq!(progression.levels_per_feat, 2);
    assert_eq!(progression.excluded_class.as_deref(), Some("Ex Fighter"));
    // Structured from PCGen's `w|x|y|z` spec: donating class, minimum level in
    // it, the most levels that may be donated, and the floor.
    let exchange = progression.exchange_level.clone().expect("EXCHANGELEVEL");
    assert_eq!(exchange.donating_class, "Ex Paladin");
    assert_eq!(exchange.minimum_level, 11);
    assert_eq!(exchange.maximum_levels, 10);
    assert_eq!(exchange.floor_level, 1);
    assert_eq!(progression.feat_slots(1), 1);
    assert_eq!(progression.feat_slots(2), 2);
    assert_eq!(progression.feat_slots(4), 3);
}

#[test]
fn hitdie_skillmult_and_levelsperfeat_reach_snapshot() {
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["class-progression".to_string()],
        &class_records(),
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        advancement: pcgen_import::advancement_from_class_levels(&vec![DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 2,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }]),
        ..CharacterDraft::default()
    });
    // d12 average is 7 per level, CON 10 has no modifier.
    assert_eq!(snapshot.hit_points, 14);
    // First level: 2 * SKILLMULT 3 = 6; second level: 2.
    assert_eq!(snapshot.skill_points, 8);
    // LEVELSPERFEAT 2: one feat at level 1 and another at level 2.
    assert_eq!(snapshot.feat_slots, 2);
    assert_eq!(snapshot.class_interactions.len(), 1);
    assert_eq!(
        snapshot.class_interactions[0].excluded_class.as_deref(),
        Some("Ex Fighter")
    );
    assert_eq!(
        snapshot.class_interactions[0]
            .exchange_level
            .clone()
            .expect("EXCHANGELEVEL")
            .donating_class,
        "Ex Paladin"
    );
}

#[test]
fn template_addlevel_changes_effective_total_level() {
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["class-progression".to_string()],
        &class_records(),
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Human".to_string(),
        advancement: pcgen_import::advancement_from_class_levels(&vec![DraftClassLevel {
            class_id: "Fighter".to_string(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }]),
        templates: vec!["Monster Template".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(snapshot.total_level, 3);
    assert_eq!(snapshot.hit_points, 21);
    assert_eq!(snapshot.feat_slots, 2);
    assert_eq!(snapshot.skill_points, 10);
}
