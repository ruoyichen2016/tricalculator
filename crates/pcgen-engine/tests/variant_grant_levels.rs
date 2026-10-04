//! Level-scoped `GRANTS` on archetypes.
//!
//! PCGen encodes a granted feature's level with a `PRECLASS` guard on the
//! `ABILITY` line, and this project's `GRANTS:` tag mirrors that
//! (`Weapon Training|PRECLASS:1,Fighter=5`).  Before the engine was
//! level-aware these surfaced at class level 1, so a 1st-level Weapon Master
//! already listed Weapon Training (PF has it at 5th) and Weapon Guard (9th).

use pcgen_import::compile_pcc;
use std::path::PathBuf;

fn features(cls: &str, archetype: &str, level: u32) -> Option<Vec<String>> {
    let pcc = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../raw/pcgen-master/data/_pf35e_extension/rsrd_pf35e.pcc"
    ));
    if !pcc.exists() {
        return None;
    }
    let (pack, _) = compile_pcc(&pcc).expect("compile the layered pack");
    let variant = pack
        .variants
        .values()
        .find(|variant| variant.name == archetype)
        .expect("archetype present");
    let base = pack
        .progressions
        .get(cls)
        .map(|progression| progression.features_up_to(level))
        .unwrap_or_default();
    Some(variant.apply_to_features_at_level(base, level))
}

fn has(features: &[String], needle: &str) -> bool {
    features.iter().any(|line| line.contains(needle))
}

#[test]
fn weapon_master_grants_follow_pf_levels() {
    let Some(l1) = features("Fighter", "Weapon Master", 1) else {
        return;
    };
    assert!(!has(&l1, "Weapon Training"), "granted at 5th: {l1:?}");
    assert!(!has(&l1, "Weapon Guard"), "granted at 9th: {l1:?}");

    let l4 = features("Fighter", "Weapon Master", 4).expect("features");
    assert!(!has(&l4, "Weapon Training"), "not yet at 4th: {l4:?}");

    let l5 = features("Fighter", "Weapon Master", 5).expect("features");
    assert!(has(&l5, "Weapon Training"), "due at 5th: {l5:?}");
    assert!(!has(&l5, "Weapon Guard"), "not yet at 5th: {l5:?}");

    let l9 = features("Fighter", "Weapon Master", 9).expect("features");
    assert!(has(&l9, "Weapon Training"), "still applies: {l9:?}");
    assert!(has(&l9, "Weapon Guard"), "due at 9th: {l9:?}");
}

#[test]
fn armor_master_grants_follow_pf_levels() {
    let Some(l1) = features("Fighter", "Armor Master", 1) else {
        return;
    };
    assert!(!has(&l1, "Armor Training"), "granted at 3rd: {l1:?}");
    assert!(!has(&l1, "Armor Mastery"), "granted at 19th: {l1:?}");

    let l3 = features("Fighter", "Armor Master", 3).expect("features");
    assert!(has(&l3, "Armor Training"), "due at 3rd: {l3:?}");

    let l18 = features("Fighter", "Armor Master", 18).expect("features");
    assert!(!has(&l18, "Armor Mastery"), "not yet at 18th: {l18:?}");

    let l19 = features("Fighter", "Armor Master", 19).expect("features");
    assert!(has(&l19, "Armor Mastery"), "due at 19th: {l19:?}");
}

#[test]
fn replaced_feature_stays_replaced_at_every_level() {
    // `REPLACES` is not level-scoped: the traded-away feature never comes back.
    let Some(l1) = features("Fighter", "Weapon Master", 1) else {
        return;
    };
    for level in [1u32, 5, 9, 20] {
        let features = features("Fighter", "Weapon Master", level).expect("features");
        assert!(
            !has(&features, "Fighter ~ Bonus Feats"),
            "Bonus Feats must stay replaced at level {level}: {features:?}"
        );
    }
    assert!(!has(&l1, "Weapon Training"));
}

#[test]
fn level_1_grants_still_apply_immediately() {
    // A guard naming level 1 (Arcane School) must not be delayed.
    let Some(features) = features("Wizard", "School Specialist", 1) else {
        return;
    };
    assert!(has(&features, "Arcane School"), "{features:?}");
    assert!(has(&features, "Arcane Bond"), "{features:?}");
    assert!(
        !has(&features, "Wizard ~ Familiar"),
        "Familiar is replaced at 1st: {features:?}"
    );
}
