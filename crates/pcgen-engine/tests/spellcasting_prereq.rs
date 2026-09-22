use pcgen_core::SpellPowerType;
use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/spell.lst", 1, "spell.pcc");
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

fn wizard_records(cast: &str) -> Vec<RawRecord> {
    vec![
        record(
            "CLASS",
            "Wizard",
            &[
                ("HD", "4"),
                ("BAB", "HALF"),
                ("SPELLSTAT", "INT"),
                ("FACT", "SpellType|Arcane"),
                ("SPELLBOOK", "YES"),
                ("TYPE", "Base.PC"),
            ],
        ),
        record("CLASS", "5", &[("CAST", cast)]),
        record(
            "RACE",
            "Human",
            &[
                ("SIZE", "M"),
                ("BONUS", "VAR|ArcaneGated|2|PRESPELLTYPE:1,Arcane=3"),
            ],
        ),
    ]
}

fn draft(level: u32) -> CharacterDraft {
    CharacterDraft {
        id: "wiz".to_string(),
        name: "Wiz".to_string(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        stats: BTreeMap::from([("INT".to_string(), 18)]),
        class_levels: vec![DraftClassLevel {
            class_id: "Wizard".to_string(),
            level,
            hit_points_gained: 0,
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    }
}

#[test]
fn wizard_derives_spell_power_book_and_gates_prespelltype_bonus() {
    // Wizard level 5 with slots up to 4th level.
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["test".to_string()],
        &wizard_records("4,4,3,2,1"),
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "unexpected unsupported: {:?}",
        report.coverage.unsupported_pending_by_tag
    );

    let snapshot = Engine::new(pack).evaluate(&draft(5));
    assert!(snapshot.spell_power_types.contains(&SpellPowerType::Arcane));
    assert!(snapshot.has_spellbook);
    assert_eq!(
        snapshot
            .max_spell_level
            .get(&SpellPowerType::Arcane)
            .copied(),
        Some(4)
    );
    // PRESPELLTYPE:1,Arcane=3 is satisfied by the 3rd/4th-level slots.
    assert_eq!(snapshot.variables.get("ArcaneGated").copied(), Some(2));
}

#[test]
fn prespelltype_gate_stays_off_below_required_level() {
    // Wizard level 5 but only 1st-level slots: the 3rd-level gate must not fire.
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["test".to_string()],
        &wizard_records("3,1"),
    );
    let snapshot = Engine::new(pack).evaluate(&draft(5));
    assert!(
        snapshot.spell_power_types.contains(&SpellPowerType::Arcane),
        "a caster is still an arcane caster"
    );
    assert!(!snapshot.variables.contains_key("ArcaneGated"));
}
