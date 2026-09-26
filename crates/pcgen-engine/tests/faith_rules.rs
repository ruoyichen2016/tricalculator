use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/faith.lst", 1, "faith.pcc");
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
fn deity_domain_spells_and_weapon_alignment_are_checked() {
    let records = vec![
        record(
            "DEITY",
            "Pelor",
            &[
                ("ALIGN", "NG"),
                ("DOMAINS", "Good|Healing"),
                ("DEITYWEAP", "Mace"),
            ],
        ),
        record(
            "DOMAIN",
            "Healing",
            &[(
                "SPELLLEVEL",
                "DOMAIN|Healing=1|Cure Light Wounds|Healing=2|Cure Moderate Wounds",
            )],
        ),
    ];
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["faith".to_string()], &records);
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        alignment: "N".to_string(),
        advancement: vec![AdvancementFrame::new(
            1,
            vec![AdvancementLane::new("a", "Cleric", 1, 8, 0)
                .with_faith("Pelor", ["Healing".to_string()])],
        )],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.faith_decisions.len(), 1);
    assert!(snapshot.domain_spells["Healing"]
        .iter()
        .any(|spell| spell.name == "Cure Light Wounds" && spell.level == 1));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("alignment")));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("favored weapon")));
}

#[test]
fn valid_deity_and_follower_flags_are_diagnosed() {
    let records = vec![
        record(
            "DEITY",
            "Dead God",
            &[
                ("ALIGN", "NG"),
                ("DOMAINS", "Sun"),
                ("DEITYWEAP", "Mace"),
                ("VALIDFORDEITY", "NO"),
                ("VALIDFORFOLLOWER", "NO"),
            ],
        ),
        record(
            "ALIGNMENT",
            "N",
            &[("VALIDFORDEITY", "NO"), ("VALIDFORFOLLOWER", "NO")],
        ),
        record("DOMAIN", "Sun", &[("SPELLLEVEL", "DOMAIN|Sun=1|Light")]),
    ];
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["faith".to_string()], &records);
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        alignment: "N".to_string(),
        advancement: vec![AdvancementFrame::new(
            1,
            vec![AdvancementLane::new("a", "Cleric", 1, 8, 0)
                .with_faith("Dead God", ["Sun".to_string()])],
        )],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("not valid for deity selection")));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("not valid for followers")));
}
