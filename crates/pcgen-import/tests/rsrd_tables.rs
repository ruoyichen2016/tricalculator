use pcgen_core::SaveKind;
use pcgen_import::compile_pcc;
use std::path::Path;

#[test]
fn rsrd_class_tables_parse() {
    let pcc = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    );
    if !pcc.exists() {
        eprintln!("skipping RSRD table test: {} missing", pcc.display());
        return;
    }
    let (pack, _) = compile_pcc(&pcc).expect("RSRD basics should compile");
    let fighter = pack.progressions.get("Fighter").expect("Fighter");
    assert_eq!(fighter.bab(4), 4);
    assert_eq!(fighter.save(4, SaveKind::Fortitude), 4);
    assert_eq!(fighter.save(4, SaveKind::Reflex), 1);

    let bard = pack.progressions.get("Bard").expect("Bard");
    assert_eq!(bard.bab(4), 3);
    assert_eq!(bard.save(4, SaveKind::Fortitude), 1);
    assert_eq!(bard.save(4, SaveKind::Reflex), 4);
    let bard_class = pack.classes.get("Bard").expect("Bard class");
    assert_eq!(
        bard_class.casting_type,
        pcgen_core::CastingType::Spontaneous
    );
    assert_eq!(bard_class.spell_slots.get(&1), Some(&vec![2]));
    assert_eq!(bard_class.spells_known.get(&1), Some(&vec![4]));
}

#[test]
fn killshot_real_campaign_compiles() {
    let pcc = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../raw/pcgen-master/data/killshot/broken_ruler_games/killshot/killshot.pcc");
    if !pcc.exists() {
        eprintln!("skipping Killshot test: {} missing", pcc.display());
        return;
    }
    let (pack, report) = compile_pcc(&pcc).expect("Killshot should compile");
    assert!(!pack.races.is_empty());
    assert!(!pack.classes.is_empty());
    assert!(!pack.equipment.is_empty());
    assert_eq!(report.error_diagnostics, 0);
}
