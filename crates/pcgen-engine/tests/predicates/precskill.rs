//! `PRECSKILL:x,y,y` — the listed skills must be *class skills*.
//!
//! PCGen's reference:
//!
//! ```text
//! PRECSKILL:x,y
//!   x: Number (The number of skills that must be considered Class Skills for
//!              the check to succeed)
//!   y: Text (A defined skill name)   y: TYPE.Text (A defined skill type)
//!
//! PRECSKILL:1,Spot,Listen  "either Spot or Listen as a Class Skill"
//! PRECSKILL:2,TYPE.Spy     "two Spy skills as Class Skills"
//! ```
//!
//! This is not `PRESKILL`: nothing here looks at ranks.  `PreCSkillTester` asks
//! `character.isClassSkill(skill)`, and `PlayerCharacter.isClassSkill` returns
//! true when *any* of the character's classes lists the skill.

use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/precskill.lst", 1, "precskill.pcc");
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

/// Is the ability gated on `PRECSKILL:<value>` active for a 1st-level Ranger?
///
/// `tag` is `PRECSKILL` or `!PRECSKILL`; `archetype` optionally layers an ACF
/// that swaps a class skill in.
fn active(tag: &str, value: &str, archetype: Option<&str>) -> bool {
    let records = vec![
        record(
            "SKILL",
            "Climb",
            &[("KEYSTAT", "STR"), ("TYPE", "Strength.Standard.Base.Climb")],
        ),
        record(
            "SKILL",
            "Spot",
            &[("KEYSTAT", "WIS"), ("TYPE", "Wisdom.Standard.Base.Spot")],
        ),
        record(
            "SKILL",
            "Spellcraft",
            &[
                ("KEYSTAT", "INT"),
                ("TYPE", "Intelligence.Standard.Base.Spellcraft"),
            ],
        ),
        record(
            "SKILL",
            "Craft (Pottery)",
            &[
                ("KEYSTAT", "INT"),
                ("TYPE", "Intelligence.Craft.Base.Craft (Pottery)"),
            ],
        ),
        record(
            "SKILL",
            "Perform (Dance)",
            &[
                ("KEYSTAT", "CHA"),
                ("TYPE", "Charisma.Perform.Base.Perform (Dance)"),
            ],
        ),
        // `TYPE=Craft` is the class-list spelling of "every Craft skill"; the
        // lowering strips the marker, so the engine has to recover it.
        record("CLASS", "Ranger", &[("CSKILL", "Climb|Spot|TYPE=Craft")]),
        record(
            "ABILITY",
            "City Dweller",
            &[
                ("CATEGORY", "ACF"),
                ("APPLIESTO", "Ranger"),
                ("CSKILL", "Spellcraft"),
            ],
        ),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                (tag, value),
                ("AUTO", "WEAPONPROF|Canaryblade"),
            ],
        ),
        record(
            "TEMPLATE",
            "Grantor",
            &[("ABILITY", "Special Ability|AUTOMATIC|Gated")],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let mut lane = AdvancementLane::primary("Ranger", 1, 0, 0);
    lane.archetype = archetype.map(str::to_string);
    let draft = CharacterDraft {
        templates: vec!["Grantor".to_string()],
        advancement: vec![AdvancementFrame::new(1, vec![lane])],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn a_class_skill_satisfies_precskill() {
    assert!(active("PRECSKILL", "1,Climb", None));
    assert!(active("PRECSKILL", "1,Spot", None));
    assert!(!active("PRECSKILL", "1,Spellcraft", None));
    // `PRECSKILL:1,Spot,Listen` is "either"; only Spot is on the Ranger list.
    assert!(active("PRECSKILL", "1,Spot,Listen", None));
    assert!(!active("PRECSKILL", "1,Listen,Spellcraft", None));
}

#[test]
fn a_count_of_two_needs_two_class_skills() {
    assert!(active("PRECSKILL", "2,Climb,Spot", None));
    assert!(!active("PRECSKILL", "2,Climb,Spellcraft", None));
}

#[test]
fn a_type_selector_matches_every_skill_of_that_type() {
    // `CSKILL:TYPE=Craft` makes every Craft skill a class skill, so the type
    // selector has to resolve through the skill type tags.
    assert!(active("PRECSKILL", "1,TYPE.Craft", None));
    assert!(!active("PRECSKILL", "1,TYPE.Perform", None));
}

#[test]
fn an_archetype_can_swap_a_class_skill_in() {
    // The ACF grants Spellcraft, and PCGen's `class_skills_for_lane` merges a
    // variant's class skills into the class's own list.
    assert!(!active("PRECSKILL", "1,Spellcraft", None));
    assert!(active("PRECSKILL", "1,Spellcraft", Some("City Dweller")));
}

#[test]
fn a_negated_precskill_holds_for_a_cross_class_skill() {
    assert!(active("!PRECSKILL", "1,Spellcraft", None));
    assert!(!active("!PRECSKILL", "1,Climb", None));
}
