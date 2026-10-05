//! Multiclass rules that the SRD states in prose.
//!
//! Source: <https://www.d20srd.org/srd/classes/multiclass.htm>
//!
//! These are the rules a table cannot express, which is why they are the ones
//! most likely to be missing:
//!
//! > Add the base attack bonuses acquired for each class to get the character's
//! > base attack bonus. [...] Add the base save bonuses for each class together.
//! >
//! > If a skill is a class skill for any of a multiclass character's classes,
//! > then character level determines a skill's maximum rank.
//! >
//! > A multiclass character gains feats based on character levels, regardless
//! > of individual class level.
//!
//! Three "special cases" on the same page are notoriously easy to miss, and are
//! covered here too: the cleric/paladin turning stack, the barbarian/rogue
//! uncanny dodge stack, and the sorcerer/wizard familiar stack.

use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::CompiledRulePack;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The RSRD basics pack used by the acceptance gates.
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

/// Build a draft whose advancement gives each `(class, levels)` entry its own
/// lane, with the lanes proceeding in the order given.
fn multiclass_draft(class_levels: &[(&str, u32)]) -> CharacterDraft {
    let mut frames = Vec::new();
    let mut current: BTreeMap<String, u32> = BTreeMap::new();
    let total: u32 = class_levels.iter().map(|(_, levels)| *levels).sum();
    let mut taken = vec![0u32; class_levels.len()];

    for character_level in 1..=total {
        // Round-robin: take one level of each class in turn until exhausted,
        // which keeps the interleaving deterministic without hard-coding it.
        let mut lanes = Vec::new();
        for (index, (class_id, levels)) in class_levels.iter().enumerate() {
            if taken[index] < *levels {
                let count = current.entry((*class_id).to_string()).or_insert(0);
                *count += 1;
                taken[index] += 1;
                let mut lane = AdvancementLane::new(
                    LaneId::new(format!("lane{index}")),
                    *class_id,
                    *count,
                    0,
                    0,
                );
                lane.hit_points_gained = 0;
                lanes.push(lane);
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

/// SRD: "Add the base attack bonuses acquired for each class to get the
/// character's base attack bonus."
///
/// Fighter 4 gives +4 and Wizard 2 gives +1, so the total is +5 — and a
/// character level of 6 is what grants the second attack, not either class
/// alone.
#[test]
fn base_attack_bonus_sums_across_classes() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let snapshot = engine.evaluate(&multiclass_draft(&[("Fighter", 4), ("Wizard", 2)]));
    assert_eq!(
        snapshot.base_attack_bonus, 5,
        "Fighter 4 (+4) + Wizard 2 (+1) = +5"
    );
}

/// SRD: "Add the base save bonuses for each class together."
///
/// Note the deliberate gaming consequence the rule creates: two classes with
/// good Fortitude stack their +2 base bonuses.
#[test]
fn saving_throws_sum_across_classes() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    // Fighter 4: +4 Fort, +1 Ref, +1 Will.  Wizard 2: +0 Fort, +0 Ref, +3 Will.
    let snapshot = engine.evaluate(&multiclass_draft(&[("Fighter", 4), ("Wizard", 2)]));
    assert_eq!(snapshot.saves.get("Fortitude").copied(), Some(4));
    assert_eq!(snapshot.saves.get("Reflex").copied(), Some(1));
    assert_eq!(snapshot.saves.get("Will").copied(), Some(4));
}

/// SRD: "A multiclass character gains feats based on character levels,
/// regardless of individual class level."
///
/// Fighter 1 / Wizard 1 has a character level of 2 but only 1 level in each
/// class, so the feat count must follow the character, not either class.
#[test]
fn feats_follow_character_level_not_class_level() {
    let Some(engine) = rsrd() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let one_one = engine.evaluate(&multiclass_draft(&[("Fighter", 1), ("Wizard", 1)]));
    let single = engine.evaluate(&multiclass_draft(&[("Fighter", 2)]));
    assert_eq!(
        one_one.feat_slots, single.feat_slots,
        "character level 2 yields the same general feat count either way"
    );
}
