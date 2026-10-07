//! Acceptance: does the persisted pack behave identically to the compiled one?
//!
//! The question this answers is whether the runtime can stop interpreting PCGen
//! text.  If a pack written to disk and read back reproduces the compiled pack
//! field for field, and the engine produces the same numbers from it, then the
//! load path no longer needs the translator.

use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, DraftClassLevel, Engine};
use pcgen_ir::rulepack::CompiledRulePack;
use pcgen_transpile::{read_pack, write_pack};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn rsrd_pcc() -> PathBuf {
    PathBuf::from(
        "/home/ursa/worksphere/gladius/raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    )
}

/// Compile the pack directly, and compile-write-read it.
fn both_paths() -> Option<(CompiledRulePack, CompiledRulePack, usize)> {
    let pcc = rsrd_pcc();
    if !pcc.exists() {
        return None;
    }
    let loaded = pcgen_import::load_pcc(&pcc).ok()?;
    let patched = pcgen_import::apply_record_patches(&loaded.records);
    let (direct, _) = pcgen_import::compile_pcc(&pcc).ok()?;

    // Unique per call: these tests run in parallel, and a directory keyed only
    // on the process id let one delete the other's files mid-write.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pcgen-transpile-stage3-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    write_pack(&dir, &direct, &patched, &pcc.display().to_string()).expect("write pack");
    let round_tripped = read_pack(&dir).expect("read pack validates");
    let _ = std::fs::remove_dir_all(&dir);

    Some((direct, round_tripped.pack, round_tripped.warnings.len()))
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

/// Every domain of the pack must survive the round trip.
///
/// The progression formulas are the one deliberate difference: the format
/// carries per-level tables instead, so the loaded classes have no formula.
#[test]
fn whole_pack_round_trips() {
    let Some((direct, loaded, warnings)) = both_paths() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    assert_eq!(warnings, 27, "the RSRD has 27 classes without level rows");

    assert_eq!(direct.entities.len(), loaded.entities.len(), "entities");
    assert_eq!(direct.races.len(), loaded.races.len(), "races");
    assert_eq!(direct.classes.len(), loaded.classes.len(), "classes");
    assert_eq!(direct.skills.len(), loaded.skills.len(), "skills");
    assert_eq!(direct.spells.len(), loaded.spells.len(), "spells");
    assert_eq!(direct.equipment.len(), loaded.equipment.len(), "equipment");
    assert_eq!(direct.deities.len(), loaded.deities.len(), "deities");
    assert_eq!(direct.domains.len(), loaded.domains.len(), "domains");
    assert_eq!(direct.variants.len(), loaded.variants.len(), "variants");
    assert_eq!(direct.kits.len(), loaded.kits.len(), "kits");
    assert_eq!(direct.templates.len(), loaded.templates.len(), "templates");
    assert_eq!(direct.abilities.len(), loaded.abilities.len(), "abilities");
    assert_eq!(direct.sizes.len(), loaded.sizes.len(), "sizes");
    assert_eq!(direct.stats.len(), loaded.stats.len(), "stats");
    assert_eq!(
        direct.alignments.len(),
        loaded.alignments.len(),
        "alignments"
    );
    assert_eq!(
        direct.progressions.len(),
        loaded.progressions.len(),
        "progressions"
    );
    assert_eq!(direct.coverage, loaded.coverage, "coverage");
    assert_eq!(direct.header, loaded.header, "header");

    // Entities carry the effects the engine actually applies, so compare them
    // wholesale rather than only counting.
    assert_eq!(direct.entities, loaded.entities, "entity bodies differ");

    for (id, before) in &direct.classes {
        let after = loaded
            .classes
            .get(id)
            .unwrap_or_else(|| panic!("class `{id}` missing after round trip"));
        assert_eq!(before.hit_die, after.hit_die, "hit die for {id}");
        assert_eq!(before.max_level, after.max_level, "max level for {id}");
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
            "the formula should not survive into the persisted pack"
        );
    }

    for (id, before) in &direct.progressions {
        let after = loaded
            .progressions
            .get(id)
            .unwrap_or_else(|| panic!("progression `{id}` missing"));
        assert_eq!(
            before.base_attack_bonus, after.base_attack_bonus,
            "base attack for {id}"
        );
        // The grants live on the class now, and are compared there; the
        // progression no longer carries a copy.
    }

    for (id, before) in &direct.variants {
        let after = loaded.variants.get(id).expect("variant survives");
        assert_eq!(before.replaces, after.replaces, "replaces for {id}");
        assert_eq!(before.grants, after.grants, "grants for {id}");
    }
}

/// The engine must produce the same numbers from a persisted pack.
#[test]
fn engine_output_is_unchanged() {
    let Some((direct, loaded, _)) = both_paths() else {
        eprintln!("skipping: RSRD data not available");
        return;
    };
    let before = Engine::new(direct);
    let after = Engine::new(loaded);

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

/// A pack directory is human-reviewable: one record per line, one file per kind.
#[test]
fn pack_is_reviewable() {
    let pcc = rsrd_pcc();
    if !pcc.exists() {
        eprintln!("skipping: RSRD data not available");
        return;
    }
    let loaded = pcgen_import::load_pcc(&pcc).unwrap();
    let patched = pcgen_import::apply_record_patches(&loaded.records);
    let (pack, _) = pcgen_import::compile_pcc(&pcc).unwrap();

    let dir = std::env::temp_dir().join(format!("pcgen-transpile-review-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_pack(&dir, &pack, &patched, &pcc.display().to_string()).unwrap();

    for file in ["classes.jsonl", "class_levels.jsonl", "entities.jsonl"] {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        let first = text.lines().next().expect("a first record");
        // Each line parses on its own: a diff is a data diff.
        let _: serde_json::Value = serde_json::from_str(first).unwrap();
        assert!(first.starts_with('{'), "{file} should be JSON Lines");
    }

    // A class level names its own class, which is what removes the cursor.
    let levels = std::fs::read_to_string(dir.join("class_levels.jsonl")).unwrap();
    let first: serde_json::Value = serde_json::from_str(levels.lines().next().unwrap()).unwrap();
    assert_eq!(first["kind"], "class_level");
    assert!(first["class"].is_string(), "a level names its class");

    let _ = std::fs::remove_dir_all(&dir);
}
