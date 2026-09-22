use pcgen_import::compile_pcc;
use std::path::Path;

#[test]
fn mini_campaign_records_tag_histograms() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaign-mini/mini.pcc");
    if !path.exists() {
        return;
    }
    let (_pack, report) = compile_pcc(&path).expect("mini campaign should compile");

    // The histogram totals must equal the scalar counters.
    let legacy_total: usize = report.coverage.legacy_by_tag.values().sum();
    assert_eq!(legacy_total, report.coverage.legacy_tags);
    let unsupported_total: usize = report.coverage.unsupported_by_tag.values().sum();
    assert_eq!(unsupported_total, report.coverage.unsupported_tags);

    // Keys are bare tag names (upper-cased), not "TAG:value" strings.
    for tag in report.coverage.legacy_by_tag.keys() {
        assert!(
            !tag.contains(':'),
            "histogram key should be a bare tag: {tag}"
        );
    }
}

#[test]
fn compiled_pack_has_stable_rules_fingerprint() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaign-mini/mini.pcc");
    if !path.exists() {
        return;
    }
    let (first, _) = compile_pcc(&path).expect("compile");
    let (second, _) = compile_pcc(&path).expect("compile again");

    assert!(first.header.rules_fingerprint.starts_with("fnv1a64:"));
    assert_eq!(
        first.header.rules_fingerprint, second.header.rules_fingerprint,
        "fingerprint must be deterministic across identical loads"
    );
}
