use pcgen_import::compile_pcc;
use std::path::Path;

#[test]
fn rsrd_deities_and_domains_are_typed() {
    let pcc = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    );
    if !pcc.exists() {
        return;
    }
    let (pack, _) = compile_pcc(&pcc).expect("RSRD should compile");
    assert!(pack.deities.len() >= 2);
    assert!(pack.domains.len() >= 20);
    let healing = pack.domains.get("Healing").expect("Healing domain");
    assert!(healing
        .domain_spells
        .iter()
        .any(|spell| spell.name.contains("Cure Light Wounds")));
}
