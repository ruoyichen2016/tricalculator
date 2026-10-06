//! Stage 1 acceptance: does the normalised path behave identically?
//!
//! The question the design says Stage 1 must answer is whether normalised data
//! can pass the existing acceptance.  The strongest available form of that is
//! an equivalence test: compile the pack the usual way, transpile it, load it
//! back, and check the class domain and the engine's output are unchanged.

use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::rulepack::CompiledRulePack;
use pcgen_transpile::{read_pack, transpile_pack, write_pack};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn rsrd_pcc() -> PathBuf {
    PathBuf::from(
        "/home/ursa/worksphere/gladius/raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    )
}

/// Compile directly, and compile-then-transpile-then-load.
fn both_paths() -> Option<(CompiledRulePack, CompiledRulePack, usize)> {
    let pcc = rsrd_pcc();
    if !pcc.exists() {
        return None;
    }
    let loaded = pcgen_import::load_pcc(&pcc).ok()?;
    let patched = pcgen_import::apply_record_patches(&loaded.records);
    let (direct, _) = pcgen_import::compile_pcc(&pcc).ok()?;

    let transpiled = transpile_pack(&direct, &patched, &pcc.display().to_string());
    let warning_count = transpiled.warnings.len();

    let dir = std::env::temp_dir().join(format!("pcgen-transpile-stage1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_pack(&dir, &transpiled.manifest, &transpiled.records).expect("write pack");
    let round_tripped = read_pack(&dir).expect("read pack validates");
    let _ = std::fs::remove_dir_all(&dir);

    let mut normalised = direct.clone();
    pcgen_transpile::apply_to_pack(&mut normalised, &round_tripped);
    Some((direct, normalised, warning_count))
}

fn draft(class: &str, level: u32) -> CharacterDraft {
    CharacterDraft {
        advancement: vec![AdvancementFrame {
            character_level: level,
            lanes: vec![AdvancementLane::new(LaneId::primary(), class, level, 0, 0)],
        }],
        class_levels: vec![DraftClassLevel {
            class_id: class.to_string(),
            level,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        stats: BTreeMap::new(),
        ..Default::default()
    }
}

/// The class domain must survive the round trip unchanged.
///
/// The formula fields are the one deliberate difference: the format carries
/// per-level tables instead, so the loaded definitions have no formula.
#[test]
fn class_domain_round_trips() {
    let Some((direct, normalised, warnings)) = both_paths() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert_eq!(warnings, 0, "transpiler should warn about nothing here");

    assert_eq!(
        direct.classes.len(),
        normalised.classes.len(),
        "class count changed"
    );

    for (id, before) in &direct.classes {
        let after = normalised
            .classes
            .get(id)
            .unwrap_or_else(|| panic!("class `{id}` missing after round trip"));
        assert_eq!(before.hit_die, after.hit_die, "hit die changed for {id}");
        assert_eq!(before.max_level, after.max_level, "max level for {id}");
        assert_eq!(
            before.spellcasting_ability, after.spellcasting_ability,
            "spellcasting ability for {id}"
        );
        assert_eq!(
            before.spell_slots, after.spell_slots,
            "spell slots for {id}"
        );
        assert_eq!(
            before.level_abilities, after.level_abilities,
            "level grants for {id}"
        );
        assert!(
            after.base_attack_formula.is_none(),
            "the formula should not survive into the normalised pack"
        );
    }

    for (id, before) in &direct.progressions {
        let after = normalised
            .progressions
            .get(id)
            .unwrap_or_else(|| panic!("progression `{id}` missing"));
        assert_eq!(
            before.base_attack_bonus, after.base_attack_bonus,
            "base attack for {id}"
        );
        assert_eq!(before.fortitude, after.fortitude, "fortitude for {id}");
        assert_eq!(before.will, after.will, "will for {id}");
        assert_eq!(
            before.class_skills, after.class_skills,
            "class skills for {id}"
        );
        assert_eq!(
            before.features_by_level, after.features_by_level,
            "features for {id}"
        );
    }

    assert_eq!(
        direct.variants.len(),
        normalised.variants.len(),
        "variant count changed"
    );
    for (id, before) in &direct.variants {
        let after = normalised.variants.get(id).expect("variant survives");
        assert_eq!(before.replaces, after.replaces, "replaces for {id}");
        assert_eq!(before.grants, after.grants, "grants for {id}");
        assert_eq!(before.applies_to, after.applies_to, "applies_to for {id}");
    }
}

/// The engine must produce the same numbers from normalised data.
#[test]
fn engine_output_is_unchanged() {
    let Some((direct, normalised, _)) = both_paths() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let before = Engine::new(direct);
    let after = Engine::new(normalised);

    let cases = [
        ("Fighter", 1),
        ("Fighter", 5),
        ("Fighter", 20),
        ("Wizard", 1),
        ("Wizard", 5),
        ("Cleric", 3),
        ("Rogue", 4),
        ("Barbarian", 5),
        ("Paladin", 4),
        ("Sorcerer", 3),
    ];

    for (class, level) in cases {
        let a = before.evaluate(&draft(class, level));
        let b = after.evaluate(&draft(class, level));
        assert_eq!(
            a.base_attack_bonus, b.base_attack_bonus,
            "{class} {level}: base attack bonus"
        );
        assert_eq!(a.saves, b.saves, "{class} {level}: saves");
        assert_eq!(a.hit_points, b.hit_points, "{class} {level}: hit points");
        assert_eq!(a.spell_slots, b.spell_slots, "{class} {level}: spell slots");
        assert_eq!(
            a.active_abilities, b.active_abilities,
            "{class} {level}: active abilities"
        );
        assert_eq!(a.turning, b.turning, "{class} {level}: turning");
        assert_eq!(
            a.uncanny_dodge, b.uncanny_dodge,
            "{class} {level}: uncanny dodge"
        );
        assert_eq!(a.feat_slots, b.feat_slots, "{class} {level}: feat slots");
    }
}
