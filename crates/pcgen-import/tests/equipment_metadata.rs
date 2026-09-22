use pcgen_import::compile_pcc;
use std::path::Path;

#[test]
fn equipment_weapon_and_modifier_metadata_is_typed() {
    let pcc = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../raw/pcgen-master/data/3e/wizards_of_the_coast/srd/basics/_rsrd_basics.pcc");
    if !pcc.exists() {
        return;
    }
    let (pack, report) = compile_pcc(&pcc).expect("compile RSRD basics");

    // A weapon should carry damage dice, a crit multiplier and a wield type.
    let weapon = pack
        .equipment
        .values()
        .find(|e| e.damage.is_some() && e.crit_mult.is_some())
        .expect("at least one weapon with DAMAGE + CRITMULT");
    assert!(
        weapon.crit_range.is_some(),
        "{} has no CRITRANGE",
        weapon.id
    );
    assert!(weapon.wield.is_some(), "{} has no WIELD", weapon.id);

    // The +N enhancement modifiers should carry a PLUS value and item types.
    let plus_mod = pack
        .equipment_modifiers
        .values()
        .find(|m| m.plus.is_some())
        .expect("at least one EQUIPMOD with PLUS");
    assert!(plus_mod.plus.unwrap() >= 1);
    assert!(
        !plus_mod.item_types.is_empty(),
        "{} has no ITYPE",
        plus_mod.id
    );

    // These tags must no longer be reported as legacy/pending.
    for tag in [
        "PLUS",
        "ITYPE",
        "REPLACES",
        "CRITMULT",
        "CRITRANGE",
        "WIELD",
    ] {
        assert_eq!(
            report.coverage.legacy_pending_by_tag.get(tag),
            None,
            "{tag} should be consumed, not pending"
        );
    }
}
