use pcgen_import::compile_pcc;
use std::path::Path;

/// Monster classes have no static BAB table; their progression comes from
/// `BONUS:COMBAT|BASEAB|classlevel()*3/4`.  `classlevel()` has *empty*
/// parentheses, which a naive regex misses — that bug zeroed monster BAB.
#[test]
fn monster_class_bab_formula_is_evaluated() {
    let pcc = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    );
    if !pcc.exists() {
        return;
    }
    let (pack, _) = compile_pcc(&pcc).expect("compile RSRD basics");
    let giant = pack
        .progressions
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Giant"))
        .map(|(_, v)| v)
        .expect("Giant class should have a progression");

    // 3/4 progression: level 4 -> 3, level 8 -> 6.
    assert_eq!(giant.bab(4), 3, "Giant BAB at level 4");
    assert_eq!(giant.bab(8), 6, "Giant BAB at level 8");
    assert!(
        giant.base_attack_bonus.iter().any(|v| *v > 0),
        "Giant BAB table must not be all zeros"
    );
}
