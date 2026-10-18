//! The two remaining multiclass "special cases" from the SRD.
//!
//! Source: <https://www.d20srd.org/srd/classes/multiclass.htm>
//!
//! > In the special case of uncanny dodge, both experienced barbarians and
//! > experienced rogues have the same ability. When a barbarian/rogue would
//! > gain uncanny dodge a second time (for her second class), she instead
//! > gains improved uncanny dodge, if she does not already have it. Her
//! > barbarian and rogue levels stack to determine the rogue level an attacker
//! > needs to flank her.
//! >
//! > In the special case of obtaining a familiar, both wizards and sorcerers
//! > have the same ability. A sorcerer/wizard stacks his sorcerer and wizard
//! > levels to determine the familiar's natural armor, Intelligence score, and
//! > special abilities.

use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, DraftCompanion, Engine};
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

/// SRD, Barbarian: uncanny dodge at 2nd level, improved at 5th.
#[test]
fn barbarian_gains_uncanny_dodge_then_improved() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let second = engine.evaluate(&draft(&[("Barbarian", 2)]));
    let dodge = second.uncanny_dodge.expect("2nd-level barbarian has it");
    assert!(dodge.uncanny_dodge);
    assert!(!dodge.improved, "improved arrives at 5th level");
    assert_eq!(dodge.flanking_level, 2);

    let fifth = engine.evaluate(&draft(&[("Barbarian", 5)]));
    let dodge = fifth.uncanny_dodge.expect("5th-level barbarian has it");
    assert!(dodge.uncanny_dodge);
    assert!(dodge.improved);
    assert!(!dodge.improved_from_second_class);
}

/// SRD, Rogue: uncanny dodge at 4th level, improved at 8th.
#[test]
fn rogue_gains_uncanny_dodge_then_improved() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert!(
        engine
            .evaluate(&draft(&[("Rogue", 3)]))
            .uncanny_dodge
            .is_none(),
        "a 3rd-level rogue does not have it yet"
    );

    let fourth = engine.evaluate(&draft(&[("Rogue", 4)]));
    let dodge = fourth.uncanny_dodge.expect("4th-level rogue has it");
    assert!(dodge.uncanny_dodge);
    assert!(!dodge.improved, "improved arrives at 8th level");
    assert_eq!(dodge.flanking_level, 4);

    let eighth = engine.evaluate(&draft(&[("Rogue", 8)]));
    assert!(eighth.uncanny_dodge.expect("8th-level rogue").improved);
}

/// SRD, Multiclass: "When a barbarian/rogue would gain uncanny dodge a second
/// time (for her second class), she instead gains improved uncanny dodge."
///
/// Barbarian 2 gives uncanny dodge; the rogue's own copy at 4th level is the
/// second one and converts.  Neither class has reached its improved level, so
/// the improved ability can only have come from the conversion.
#[test]
fn second_class_converts_uncanny_dodge_into_improved() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let snapshot = engine.evaluate(&draft(&[("Barbarian", 2), ("Rogue", 4)]));
    let dodge = snapshot.uncanny_dodge.expect("both classes grant it");
    assert!(dodge.uncanny_dodge);
    assert!(
        dodge.improved,
        "the second granting class converts to improved"
    );
    assert!(dodge.improved_from_second_class);
    assert!(
        dodge.improved_classes.is_empty(),
        "no class granted improved directly"
    );
    assert_eq!(dodge.uncanny_dodge_classes.len(), 2);
}

/// SRD, Multiclass: "Her barbarian and rogue levels stack to determine the
/// rogue level an attacker needs to flank her."
#[test]
fn granting_class_levels_stack_for_flanking() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let snapshot = engine.evaluate(&draft(&[("Barbarian", 2), ("Rogue", 4)]));
    assert_eq!(
        snapshot.uncanny_dodge.expect("has it").flanking_level,
        6,
        "barbarian 2 + rogue 4"
    );

    let snapshot = engine.evaluate(&draft(&[("Barbarian", 5), ("Rogue", 8)]));
    assert_eq!(
        snapshot.uncanny_dodge.expect("has it").flanking_level,
        13,
        "barbarian 5 + rogue 8"
    );
}

/// A class with no uncanny dodge produces no state.
#[test]
fn class_without_uncanny_dodge_has_no_state() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert!(engine
        .evaluate(&draft(&[("Wizard", 5)]))
        .uncanny_dodge
        .is_none());
}

/// SRD, Multiclass: "A sorcerer/wizard stacks his sorcerer and wizard levels to
/// determine the familiar's natural armor, Intelligence score, and special
/// abilities."
#[test]
fn sorcerer_and_wizard_levels_stack_for_the_familiar() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let sorcerer = engine.evaluate(&draft(&[("Sorcerer", 3)]));
    assert_eq!(sorcerer.variables.get("FamiliarLVL").copied(), Some(3));

    let wizard = engine.evaluate(&draft(&[("Wizard", 3)]));
    assert_eq!(wizard.variables.get("FamiliarLVL").copied(), Some(3));

    let both = engine.evaluate(&draft(&[("Sorcerer", 3), ("Wizard", 3)]));
    assert_eq!(
        both.variables.get("FamiliarLVL").copied(),
        Some(6),
        "the two classes stack, so 3 + 3"
    );
}

/// The familiar's master level comes from the data's `FOLLOWER:FamiliarLVL=...`
/// rows, so a stacked sorcerer/wizard gets the stacked level even when the
/// caller passes something else.
#[test]
fn familiar_master_level_follows_the_stacked_variable() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let mut draft = draft(&[("Sorcerer", 3), ("Wizard", 3)]);
    draft.companions = vec![DraftCompanion::new("Familiar", 1)];
    let snapshot = engine.evaluate(&draft);
    let familiar = snapshot
        .companions
        .iter()
        .find(|companion| companion.companion_type.eq_ignore_ascii_case("Familiar"))
        .expect("the familiar is modelled");
    assert_eq!(
        familiar.master_level, 6,
        "the stacked sorcerer + wizard level, not the caller's 1"
    );
}
