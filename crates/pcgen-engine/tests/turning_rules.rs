//! Turn/rebuke undead rules, which the SRD states in prose.
//!
//! Sources:
//! * <https://www.d20srd.org/srd/combat/specialAttacks.htm>
//! * <https://www.d20srd.org/srd/classes/multiclass.htm>
//!
//! The paladin rule and the multiclass rule are two halves of the same thing:
//!
//! > Beginning at 4th level, paladins can turn undead as if they were clerics
//! > of three levels lower than they actually are.
//! >
//! > In the special case of turning undead, both clerics and experienced
//! > paladins have the same ability. If the character's paladin level is 4th
//! > or higher, her effective turning level is her cleric level plus her
//! > paladin level minus 3.

use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::CompiledRulePack;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn rsrd() -> Option<Engine> {
    let pcc = PathBuf::from(
        "/home/ursa/worksphere/gladius/raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    );
    if !pcc.exists() {
        return None;
    }
    let (pack, _): (CompiledRulePack, _) = pcgen_import::compile_pcc(&pcc).ok()?;
    Some(Engine::new(pack))
}

/// Build a draft that takes the given classes in round-robin order.
fn draft(class_levels: &[(&str, u32)]) -> CharacterDraft {
    let total: u32 = class_levels.iter().map(|(_, levels)| *levels).sum();
    let mut taken = vec![0u32; class_levels.len()];
    let mut counts = vec![0u32; class_levels.len()];
    let mut frames = Vec::new();

    for character_level in 1..=total {
        let mut lanes = Vec::new();
        for (index, (class_id, levels)) in class_levels.iter().enumerate() {
            if taken[index] < *levels {
                counts[index] += 1;
                taken[index] += 1;
                lanes.push(AdvancementLane::new(
                    LaneId::new(format!("lane{index}")),
                    *class_id,
                    counts[index],
                    0,
                    0,
                ));
                break;
            }
        }
        frames.push(AdvancementFrame {
            character_level,
            lanes,
        });
    }

    CharacterDraft {
        advancement: frames,
        stats: BTreeMap::new(),
        ..Default::default()
    }
}

fn turning_level(engine: &Engine, class_levels: &[(&str, u32)]) -> Option<i32> {
    let snapshot = engine.evaluate(&draft(class_levels));
    snapshot
        .variables
        .get("TurnLevelBase")
        .copied()
        .filter(|level| *level > 0)
}

/// SRD, Cleric: a cleric turns undead at her cleric level.
#[test]
fn cleric_turning_level_is_cleric_level() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert_eq!(turning_level(&engine, &[("Cleric", 3)]), Some(3));
    assert_eq!(turning_level(&engine, &[("Cleric", 1)]), Some(1));
}

/// SRD, Paladin: "Beginning at 4th level, paladins can turn undead as if they
/// were clerics of three levels lower than they actually are."
#[test]
fn paladin_turns_as_a_cleric_three_levels_lower() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    // 4 - 3 = 1, 8 - 3 = 5.
    assert_eq!(turning_level(&engine, &[("Paladin", 4)]), Some(1));
    assert_eq!(turning_level(&engine, &[("Paladin", 8)]), Some(5));
}

/// SRD, Paladin: the ability arrives at 4th level, so a 3rd-level paladin has
/// no turning at all.
#[test]
fn paladin_below_fourth_level_has_no_turning() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert_eq!(turning_level(&engine, &[("Paladin", 3)]), None);
}

/// SRD, Multiclass: "her effective turning level is her cleric level plus her
/// paladin level minus 3."
///
/// This is the case that distinguishes *summing* the two classes from taking
/// the best of them: Cleric 3 / Paladin 6 is 3 + (6 - 3) = 6, not max(3, 3) = 3.
#[test]
fn multiclass_turning_level_sums_cleric_and_paladin() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert_eq!(
        turning_level(&engine, &[("Cleric", 3), ("Paladin", 6)]),
        Some(6),
        "cleric 3 + (paladin 6 - 3) = 6"
    );
    assert_eq!(
        turning_level(&engine, &[("Cleric", 1), ("Paladin", 4)]),
        Some(2),
        "cleric 1 + (paladin 4 - 3) = 2"
    );
}

/// SRD, Cleric: "A cleric may attempt to turn undead a number of times per day
/// equal to 3 + his Charisma modifier", and turning damage is
/// "2d6 + your cleric level + your Charisma modifier".
#[test]
fn turning_uses_per_day_and_damage_follow_charisma() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let mut draft = draft(&[("Cleric", 3)]);
    draft.stats = [("CHA".to_string(), 14)].into_iter().collect();
    let snapshot = engine.evaluate(&draft);

    // Cha 14 -> +2.
    assert_eq!(snapshot.variables.get("TurnTimesBase").copied(), Some(5));
    // Cleric level 3 + Cha 2 = 5.
    assert_eq!(
        snapshot.variables.get("TurnDamagePlusBase").copied(),
        Some(5)
    );
}

/// SRD, Special Attacks: good clerics channel positive energy and turn undead.
#[test]
fn good_cleric_turns_undead() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    for alignment in ["LG", "NG", "CG"] {
        let mut draft = draft(&[("Cleric", 3)]);
        draft.alignment = alignment.to_string();
        let turning = engine
            .evaluate(&draft)
            .turning
            .unwrap_or_else(|| panic!("{alignment} cleric should have turning"));
        assert_eq!(
            turning.mode,
            pcgen_engine::TurningMode::Turn,
            "{alignment} is good, so it turns"
        );
        assert_eq!(turning.level, 3);
        assert!(turning.note.is_none());
    }
}

/// SRD, Special Attacks: evil clerics channel negative energy and rebuke.
#[test]
fn evil_cleric_rebukes_undead() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    for alignment in ["LE", "NE", "CE"] {
        let mut draft = draft(&[("Cleric", 3)]);
        draft.alignment = alignment.to_string();
        let turning = engine
            .evaluate(&draft)
            .turning
            .unwrap_or_else(|| panic!("{alignment} cleric should have turning"));
        assert_eq!(
            turning.mode,
            pcgen_engine::TurningMode::Rebuke,
            "{alignment} is evil, so it rebukes"
        );
    }
}

/// SRD, Cleric: "A neutral cleric of a neutral deity must choose whether his
/// turning ability functions as that of a good cleric or an evil cleric."
///
/// A neutral character's answer depends on the deity's alignment, and the RSRD
/// ships no deities (they are closed content), so the engine reports the choice
/// and says why rather than guessing.
#[test]
fn neutral_cleric_reports_the_choice_and_why() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    for alignment in ["LN", "TN", "CN"] {
        let mut draft = draft(&[("Cleric", 3)]);
        draft.alignment = alignment.to_string();
        let turning = engine
            .evaluate(&draft)
            .turning
            .unwrap_or_else(|| panic!("{alignment} cleric should have turning"));
        assert_eq!(turning.mode, pcgen_engine::TurningMode::Choice);
        assert!(
            turning.note.is_some(),
            "the choice must explain why it cannot be decided"
        );
    }
}

/// A class that grants no turning produces no turning state at all.
#[test]
fn class_without_turning_has_no_turning_state() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert!(engine.evaluate(&draft(&[("Wizard", 3)])).turning.is_none());
}

/// SRD, Paladin: turning arrives at 4th level and follows alignment like a
/// cleric's.
#[test]
fn good_paladin_turns_from_fourth_level() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let mut below = draft(&[("Paladin", 3)]);
    below.alignment = "LG".to_string();
    assert!(engine.evaluate(&below).turning.is_none());

    let mut at = draft(&[("Paladin", 4)]);
    at.alignment = "LG".to_string();
    let turning = engine
        .evaluate(&at)
        .turning
        .expect("4th-level paladin turns");
    assert_eq!(turning.mode, pcgen_engine::TurningMode::Turn);
    assert_eq!(turning.level, 1, "as a cleric of three levels lower");
}
