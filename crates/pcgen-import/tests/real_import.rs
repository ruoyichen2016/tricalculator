use pcgen_import::compile_pcc;
use std::path::Path;

#[test]
fn real_srd_typed_smoke() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../raw/pcgen-master/data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc");
    if !path.exists() {
        eprintln!("skipping real SRD smoke: {} does not exist", path.display());
        return;
    }
    let (pack, report) = compile_pcc(&path).expect("real SRD should compile");
    assert!(pack.races.len() >= 50);
    assert!(pack.classes.len() >= 40);
    assert!(pack.skills.len() >= 80);
    assert!(pack.spells.len() >= 300);
    assert!(pack.equipment.len() >= 300);
    assert!(pack.progressions.len() >= 40);
    assert!(report.coverage.lowered_effects > 2_000);
    assert!(report.coverage.unsupported_tags < 800);
    assert!(report.coverage.tag_occurrences > 10_000);
}

#[test]
fn real_rsrd_companion_rows_are_preserved() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    );
    if !path.exists() {
        eprintln!(
            "skipping companion row check: {} does not exist",
            path.display()
        );
        return;
    }
    let (pack, _) = compile_pcc(&path).expect("RSRD companion data should compile");
    let row_count: usize = pack.companion_mods.values().map(Vec::len).sum();
    assert!(
        row_count >= 100,
        "expected all FOLLOWER rows to be preserved, got {row_count}"
    );
    assert!(
        pack.companion_mods.len() >= 30,
        "expected grouped companion ids, got {}",
        pack.companion_mods.len()
    );
}
