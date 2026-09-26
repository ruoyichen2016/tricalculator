//! Untrained-skill base expansion.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/skills.lst", 1, "skills.pcc");
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

#[test]
fn typed_bonus_materializes_untrained_skill_base() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record("SKILL", "Spot", &[("KEYSTAT", "WIS")]),
        record(
            "SKILL",
            "Disable Device",
            &[("KEYSTAT", "INT"), ("USEUNTRAINED", "NO")],
        ),
        record(
            "RACE",
            "Skill Race",
            &[
                ("BONUS", "SKILL|Spot|2|TYPE=Racial"),
                ("BONUS", "SKILL|Disable Device|2|TYPE=Racial"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["untrained".to_string()],
        &records,
    );
    let draft = CharacterDraft {
        race_id: "Skill Race".to_string(),
        stats: BTreeMap::from([("WIS".to_string(), 14), ("INT".to_string(), 14)]),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    // Spot is usable untrained: 0 ranks + WIS(+2) + racial(+2) = 4.
    assert_eq!(snapshot.skills.get("Spot"), Some(&4));
    // Disable Device is trained-only: no untrained entry is materialized.
    assert!(!snapshot.skills.contains_key("Disable Device"));
}
