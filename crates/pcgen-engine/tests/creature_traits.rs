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
