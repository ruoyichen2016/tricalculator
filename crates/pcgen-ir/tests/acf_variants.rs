//! 3.5e Alternate Class Features (`CATEGORY:ACF`).
//!
//! `archetype` is Pathfinder's name for what 3.5e calls an *ACF*.  Both swap
//! class features, but they say so differently:
//!
//! * Pathfinder marks the record `TYPE:Archetype.<Class>Archetype` and names
//!   the displaced feature with `REPLACES:`.
//! * 3.5e marks the record `CATEGORY:ACF` and carries the displaced feature as
//!   a type tag (`TYPE:ACF.ClericTurnUndead`), which is exactly the anchor the
//!   RSRD class row guards with `!PREABILITY:1,CATEGORY=ACF,TYPE.ClericTurnUndead`.
//!
//! Before ACF was recognised these records parsed as ordinary abilities and no
//! variant was produced, so the whole 3.5e ACF mechanism was invisible even
//! though the anchors already lined up.

use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(name: &str, key: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/ua_abilities_class.lst", 1, "ua.pcc");
    RawRecord {
        category: "ABILITY".to_string(),
        key: key.to_string(),
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

/// A minimal `CLASS:` header.
///
/// A variant's owner is only kept when the pack actually declares that class:
/// the pack loader refuses a variant naming an undeclared class, so a fixture
/// that asserts an owner must declare the class too.
fn class_record(name: &str) -> RawRecord {
    let source = SourceRef::new("/tmp/ua_classes.lst", 1, "ua.pcc");
    RawRecord {
        category: "CLASS".to_string(),
        key: name.to_string(),
        name: name.to_string(),
        tags: vec![RawTag {
            name: "HD".to_string(),
            value: "8".to_string(),
            source: source.clone(),
        }],
        source,
        raw: String::new(),
    }
}

fn lower(records: &[RawRecord]) -> pcgen_ir::CompiledRulePack {
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["ua.pcc".to_string()], records);
    pack
}

/// The shape of `Unearthed Arcana`'s `Cleric ~ Smite Evil`.
fn smite_evil() -> RawRecord {
    record(
        "Smite Evil Cleric",
        "Cleric ~ Smite Evil",
        &[
            ("SORTKEY", "Cleric"),
            ("CATEGORY", "ACF"),
            ("TYPE", "ACF.ClericTurnUndead"),
            ("ABILITY", "Special Ability|AUTOMATIC|Smite Evil"),
        ],
    )
}

#[test]
fn acf_record_becomes_a_variant() {
    let pack = lower(&[class_record("Cleric"), smite_evil()]);
    let variant = pack
        .variants
        .get("Cleric ~ Smite Evil")
        .expect("ACF should be recognised as a variant");
    assert_eq!(variant.applies_to.as_deref(), Some("Cleric"));
    // `ACF` is a pool name, never a class.
    assert_ne!(variant.applies_to.as_deref(), Some("ACF"));
}

#[test]
fn acf_displaced_feature_becomes_a_type_target() {
    // The displaced feature is addressed the same way the RSRD guards it.
    let pack = lower(&[smite_evil()]);
    let variant = pack.variants.get("Cleric ~ Smite Evil").expect("variant");
    assert_eq!(variant.replaces, vec!["TYPE=ClericTurnUndead".to_string()]);

    let anchor = "Cleric Class Feature|AUTOMATIC|Cleric ~ Turn or Rebuke Undead|!PREABILITY:1,CATEGORY=ACF,TYPE.ClericTurnUndead";
    assert!(
        variant.replaces_feature_line(anchor),
        "the anchor the RSRD guards must be the one we replace"
    );
}

#[test]
fn acf_grants_carry_their_level() {
    // ACF grants gate on `PREVARGTEQ:<Class>LVL,<n>`; `Ape Totem ~ Climb (Full)`
    // is a 5th-level grant.
    let ape = record(
        "Ape",
        "Totem Barbarian ~ Ape",
        &[
            ("CATEGORY", "ACF"),
            (
                "TYPE",
                "Totem Barbarian.SpecialQuality.BarbarianUncannyDodge.BarbarianTrapSense",
            ),
            (
                "ABILITY",
                "Barbarian Class Feature|AUTOMATIC|Ape Totem ~ Climb (Half)|PREVARGTEQ:BarbarianLVL,1",
            ),
            (
                "ABILITY",
                "Barbarian Class Feature|AUTOMATIC|Ape Totem ~ Climb (Full)|PREVARGTEQ:BarbarianLVL,5",
            ),
        ],
    );
    let pack = lower(&[class_record("Barbarian"), ape]);
    let variant = pack.variants.get("Totem Barbarian ~ Ape").expect("variant");

    // The class comes from the `PREVARGTEQ:BarbarianLVL` guard, not from the
    // `Totem Barbarian` group name: the displaced-feature anchors are
    // namespaced by the class (`BarbarianUncannyDodge`).
    assert_eq!(variant.applies_to.as_deref(), Some("Barbarian"));
    let entries = variant.grant_entries();
    let full = entries
        .iter()
        .find(|entry| entry.name == "Ape Totem ~ Climb (Full)")
        .expect("grant present");
    assert_eq!(full.level, Some(5));
    assert!(!full.applies_at(4));
    assert!(full.applies_at(5));

    assert!(
        variant
            .replaces
            .iter()
            .any(|t| t == "TYPE=BarbarianUncannyDodge")
            && variant
                .replaces
                .iter()
                .any(|t| t == "TYPE=BarbarianTrapSense"),
        "displaced features not captured: {:?}",
        variant.replaces
    );
}

#[test]
fn plain_ability_is_still_not_a_variant() {
    // Regression guard: widening detection must not turn every ability into a
    // variant.
    let plain = record(
        "Alertness",
        "Alertness",
        &[("CATEGORY", "Special Ability"), ("TYPE", "SpecialQuality")],
    );
    let pack = lower(&[plain]);
    assert!(pack.variants.is_empty(), "{:?}", pack.variants.keys());
}

#[test]
fn archetype_and_acf_coexist_in_one_pack() {
    let archetype = record(
        "Weapon Master",
        "Weapon Master",
        &[
            ("CATEGORY", "Special Ability"),
            ("TYPE", "Archetype"),
            ("APPLIESTO", "Fighter"),
            ("REPLACES", "TYPE=FighterBonusFeat"),
        ],
    );
    let pack = lower(&[archetype, smite_evil()]);
    assert_eq!(pack.variants.len(), 2, "{:?}", pack.variants.keys());
}
