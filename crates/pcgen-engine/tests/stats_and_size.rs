//! P4-STAT-2: DEFINESTAT minimums, STATMOD formulas and SIZENUM ordering.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/stats-size.lst", 1, "stats.pcc");
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
fn stat_minimum_statmod_and_sizenum_affect_snapshot() {
    let records = vec![
        record("SIZE", "M", &[("SIZENUM", "050")]),
        record("SIZE", "L", &[("SIZENUM", "060")]),
        record("STAT", "DEX", &[("KEY", "DEX"), ("STATMOD", "SCORE-10")]),
        record(
            "RACE",
            "Giant",
            &[("SIZE", "L"), ("DEFINESTAT", "MINVALUE|INT|6")],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["stats-size".to_string()],
        &records,
    );
    let snapshot = Engine::new(pack).evaluate(&CharacterDraft {
        race_id: "Giant".to_string(),
        stats: BTreeMap::from([("DEX".to_string(), 14), ("INT".to_string(), 3)]),
        ..CharacterDraft::default()
    });
    // DEFINESTAT:MINVALUE raises INT from 3 to 6.
    assert_eq!(snapshot.stats.get("INT"), Some(&6));
    // STATMOD:SCORE-10 makes DEX 14 a +4 modifier, used by initiative.
    assert_eq!(snapshot.initiative, 4);
    // SIZENUM:060 ranks Large after Medium; Large has a -1 size modifier.
    assert_eq!(snapshot.size_rank, 5);
    assert_eq!(snapshot.armor_class, 13);
}
