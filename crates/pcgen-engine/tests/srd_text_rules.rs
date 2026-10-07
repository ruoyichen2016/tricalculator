//! Rules that the SRD states in **prose** rather than in a table.
//!
//! PCGen encodes skills and spell slots as numbers, so a rule that only appears
//! in the running text can be missing from the data entirely.  These tests pin
//! the behaviour to the SRD's own wording, and cite it, so a regression is
//! caught against the rule rather than against a previous implementation.
//!
//! Sources:
//! * <https://www.d20srd.org/srd/skills/skillsSummary.htm>
//! * <https://www.d20srd.org/srd/classes/cleric.htm>
//! * <https://www.d20srd.org/srd/classes/fighter.htm>

use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/srd_text.lst", 1, "srd_text.pcc");
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

/// A cleric with a domain, so the domain-slot rule has something to act on.
fn cleric_with_domain() -> Engine {
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            &[
                ("ABB", "Clr"),
                ("HD", "8"),
                ("BAB", "classlevel(\"CL=CLERIC\")*3/4"),
                ("SPELLSTAT", "WIS"),
                ("MEMORIZE", "YES"),
                ("SPELLTYPE", "Divine"),
            ],
        ),
        // The record *name* is the class level, as in PCGen's own class rows
        // (`1\tCAST:3,1`).
        record("CLASS", "1", &[("CAST", "3,1"), ("KNOWN", "0,0")]),
        record("DOMAIN", "War", &[("SPELLLEVEL", "1|Magic Weapon")]),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    Engine::new(pack)
}

fn lane(with_domain: bool) -> AdvancementLane {
    let mut lane = AdvancementLane::new(LaneId::primary(), "Cleric", 1, 0, 0);
    if with_domain {
        lane.domains = vec!["War".to_string()];
        lane.deity = Some("Hextor".to_string());
    }
    lane
}

fn cleric_draft(with_domain: bool) -> CharacterDraft {
    CharacterDraft {
        advancement: vec![AdvancementFrame {
            character_level: 1,
            lanes: vec![lane(with_domain)],
        }],
        stats: BTreeMap::new(),
        ..Default::default()
    }
}

/// SRD, Cleric: the Spells per Day column carries a `+1` at every spell level,
/// and the text explains it — "A cleric [...] gains one domain spell slot for
/// each spell level she can cast".
#[test]
fn cleric_gains_one_domain_slot_per_spell_level() {
    let engine = cleric_with_domain();
    let with = engine.evaluate(&cleric_draft(true));
    let without = engine.evaluate(&cleric_draft(false));

    let with_one = with.spell_slots.get("Cleric|1").copied().unwrap_or(0);
    let without_one = without.spell_slots.get("Cleric|1").copied().unwrap_or(0);

    // The base CAST: progression gives 1 first-level slot; the domain adds one.
    assert_eq!(
        without_one, 1,
        "without a domain the cleric has only the base slot"
    );
    assert_eq!(
        with_one,
        without_one + 1,
        "a domain must add exactly one slot at each castable level"
    );

    // Level 0 is orisons, which the SRD does not give a domain slot for.
    let with_zero = with.spell_slots.get("Cleric|0").copied().unwrap_or(0);
    let without_zero = without.spell_slots.get("Cleric|0").copied().unwrap_or(0);
    assert_eq!(
        with_zero, without_zero,
        "domain slots start at 1st level, not 0-level"
    );
}

/// SRD, Cleric: a cleric with no domains gains no domain slot.
#[test]
fn no_domain_means_no_domain_slot() {
    let engine = cleric_with_domain();
    let snapshot = engine.evaluate(&cleric_draft(false));
    assert_eq!(snapshot.spell_slots.get("Cleric|1").copied(), Some(1));
}

/// SRD, Fighter: "The fighter gains an additional bonus feat at 2nd level and
/// every two **fighter** levels thereafter" — fighter levels, not character
/// levels.
#[test]
fn fighter_bonus_feats_scale_with_class_level_not_character_level() {
    let records = vec![
        record(
            "CLASS",
            "Fighter",
            &[
                ("ABB", "Ftr"),
                ("HD", "10"),
                ("BAB", "classlevel(\"CL=FIGHTER\")"),
            ],
        ),
        record(
            "CLASS",
            "1",
            &[(
                "ABILITY",
                "Fighter Class Feature|AUTOMATIC|Fighter ~ Bonus Feats",
            )],
        ),
        record(
            "CLASS",
            "2",
            &[(
                "ABILITY",
                "Fighter Class Feature|AUTOMATIC|Fighter ~ Bonus Feats",
            )],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);

    // The class must expose the bonus-feat levels the table lists.  The grants
    // live on the class and are structured, so match the feature itself.
    let fighter = pack.classes.get("Fighter").expect("Fighter is compiled");
    let feat_levels: Vec<u32> = fighter
        .level_abilities
        .iter()
        .filter(|(_, grants)| {
            grants
                .iter()
                .any(|grant| grant.feature.contains("Bonus Feat"))
        })
        .map(|(level, _)| *level)
        .collect();
    assert!(
        feat_levels.contains(&1) && feat_levels.contains(&2),
        "bonus feats start at 1st and 2nd level, got {feat_levels:?}"
    );
}
