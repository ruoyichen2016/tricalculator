//! Psionic closed loop: power points, manifester level, power DC and powers.

use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_import::compile_pcc;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::path::{Path, PathBuf};

fn psionics_pcc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/psionics/_rsrd_psionics.pcc",
    )
}

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/psionics.lst", 1, "psionics.pcc");
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
fn temp_based_psionic_power_counted_as_psionic_effect() {
    // The psionics dataset declares powers under `SPELL:` with
    // `TYPE:Psionic`; `psionic_effects` counts those records.
    let records = vec![record(
        "SPELL",
        "Burst",
        &[
            ("TYPE", "Psionic"),
            ("CLASSES", "Psion=1"),
            ("TEMPBONUS", "PC|MOVEADD|TYPE.Walk|10|TYPE=Enhancement"),
        ],
    )];
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["psionic-power".to_string()],
        &records,
    );
    assert_eq!(report.psionic_effects, 1);
    assert_eq!(report.spell_effects, 1);
    let power = pack
        .entities
        .iter()
        .find(|entity| entity.id.key == "burst")
        .expect("burst power entity");
    assert_eq!(power.category, pcgen_core::EntityCategory::Spell);
    assert!(pack
        .spells
        .get("burst")
        .is_some_and(|spell| spell.is_psionic));
}

#[test]
fn psion_power_points_manifester_level_and_dc() {
    let pcc = psionics_pcc();
    if !pcc.exists() {
        eprintln!("skipping psionics test: {} does not exist", pcc.display());
        return;
    }
    let (pack, report) = compile_pcc(&pcc).expect("psionics campaign should compile");
    assert!(
        report.psionic_effects > 0,
        "POWER effects should be counted"
    );

    let draft = CharacterDraft {
        id: "psion".to_string(),
        name: "Psion".to_string(),
        race_id: "Human".to_string(),
        stats: [("INT".to_string(), 18), ("WIS".to_string(), 16)]
            .into_iter()
            .collect(),
        class_levels: vec![DraftClassLevel {
            class_id: "Psion".to_string(),
            level: 5,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        spells: vec![pcgen_engine::DraftSpell {
            name: "Psionic Charm".to_string(),
            level: 2,
            class_id: Some("Psion".to_string()),
            times_memorized: 0,
        }],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);

    // Psion 5, INT 18: 25 base + (4 * 5 / 2) = 35.
    assert_eq!(snapshot.power_points.get("Psion"), Some(&35));
    assert_eq!(snapshot.manifester_levels.get("Psion"), Some(&5));
    assert!(snapshot
        .spell_power_types
        .contains(&pcgen_core::SpellPowerType::Psionic));
    assert_eq!(snapshot.spell_dcs.get("Psionic Charm|Psion"), Some(&16));
    assert!(!snapshot.caster_levels.contains_key("Psion"));
}

#[test]
fn psychic_warrior_power_points_use_wisdom() {
    let pcc = psionics_pcc();
    if !pcc.exists() {
        eprintln!("skipping psionics test: {} does not exist", pcc.display());
        return;
    }
    let (pack, _) = compile_pcc(&pcc).expect("psionics campaign should compile");
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        stats: [("INT".to_string(), 10), ("WIS".to_string(), 16)]
            .into_iter()
            .collect(),
        class_levels: vec![DraftClassLevel {
            class_id: "Psychic Warrior".to_string(),
            level: 6,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    // Psychic Warrior 6: 11 base + (WIS 16 -> +3) * 6 / 2 = 11 + 9 = 20.
    assert_eq!(snapshot.power_points.get("Psychic Warrior"), Some(&20));
    assert_eq!(snapshot.manifester_levels.get("Psychic Warrior"), Some(&6));
}
