//! Golden tests for the `.pcg` draft reader.
//!
//! These are the migration of archive's golden character parsing tests.

use pcgen_import::parse_pcg_draft_file;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pcg")
        .join(name)
}

fn load(name: &str) -> pcgen_import::PcgDraft {
    parse_pcg_draft_file(fixture(name))
        .unwrap_or_else(|error| panic!("failed to parse {name}: {error}"))
}

fn class_level(character: &pcgen_import::PcgDraft, class_id: &str) -> u32 {
    character
        .class_levels
        .iter()
        .find(|entry| entry.class_id.eq_ignore_ascii_case(class_id))
        .map_or(0, |entry| entry.level)
}

#[test]
fn code_monkey_golden() {
    let character = load("CodeMonkey.pcg");
    assert_eq!(character.name, "Code Monkey");
    assert_eq!(character.race_id, "Half-Orc");
    assert_eq!(character.stats["STR"], 15);
    assert_eq!(character.stats["DEX"], 12);
    assert_eq!(class_level(&character, "Fighter"), 1);
    assert_eq!(class_level(&character, "Barbarian"), 1);
    assert_eq!(class_level(&character, "Rogue"), 1);
    assert_eq!(character.skills["Move Silently"], 2.0);
    assert_eq!(character.skills["Spot"], 1.0);
    assert_eq!(character.skills["Pick Pocket"], 3.0);
}

#[test]
fn figfae_golden_preserves_half_rank() {
    let character = load("3e_FigFae.pcg");
    assert_eq!(character.name, "FigFae");
    assert_eq!(character.race_id, "Half-Orc");
    assert_eq!(character.stats["STR"], 15);
    assert_eq!(character.stats["DEX"], 17);
    assert_eq!(class_level(&character, "Fighter"), 4);
    assert_eq!(character.skills["Climb"], 7.0);
    assert_eq!(character.skills["Jump"], 7.0);
    assert_eq!(character.skills["Disable Device"], 3.5);
}

#[test]
fn bard_golden_multiclass() {
    let character = load("35e_Bard.pcg");
    assert_eq!(character.name, "35eBard");
    assert_eq!(character.race_id, "Human");
    assert_eq!(class_level(&character, "Bard"), 4);
    assert_eq!(class_level(&character, "Rogue"), 1);
    assert_eq!(character.skills["Perform (String Instruments)"], 7.0);
}

#[test]
fn ivan_golden_multiclass() {
    let character = load("35e_Ivan.pcg");
    assert_eq!(character.name, "Ivan");
    assert_eq!(class_level(&character, "Test Nothing"), 2);
    assert_eq!(class_level(&character, "Sorcerer"), 1);
}

#[test]
fn special_wizard_golden() {
    let character = load("SpecialWizard.pcg");
    assert_eq!(character.name, "Wizard (Evoker)");
    assert_eq!(character.race_id, "Gnome");
    assert_eq!(character.stats["INT"], 18);
    assert_eq!(class_level(&character, "Wizard"), 8);
    assert_eq!(character.skills["Spellcraft"], 11.0);
    assert_eq!(character.skills["Concentration"], 11.0);
}
