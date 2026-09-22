use pcgen_core::tagclass::TagClass;
use pcgen_import::compile_pcc;
use std::path::Path;

#[test]
fn legacy_split_partitions_legacy_tags() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaign-mini/mini.pcc");
    if !path.exists() {
        return;
    }
    let (_pack, report) = compile_pcc(&path).expect("compile mini");

    let legacy_total: usize = report.coverage.legacy_by_tag.values().sum();
    let allowlisted: usize = report.coverage.legacy_allowlisted_by_tag.values().sum();
    let pending: usize = report.coverage.legacy_pending_by_tag.values().sum();

    assert_eq!(
        allowlisted + pending,
        legacy_total,
        "allowlisted + pending must equal legacy_tags"
    );

    let unsupported_total: usize = report.coverage.unsupported_by_tag.values().sum();
    let uns_allowlisted: usize = report
        .coverage
        .unsupported_allowlisted_by_tag
        .values()
        .sum();
    let uns_pending: usize = report.coverage.unsupported_pending_by_tag.values().sum();
    assert_eq!(
        uns_allowlisted + uns_pending,
        unsupported_total,
        "unsupported allowlisted + pending must equal unsupported_tags"
    );
}

#[test]
fn each_histogram_key_lands_in_the_right_bucket() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaign-mini/mini.pcc");
    if !path.exists() {
        return;
    }
    let (_pack, report) = compile_pcc(&path).expect("compile mini");

    for (tag, count) in &report.coverage.legacy_by_tag {
        let expected = pcgen_core::classify_tag(tag);
        let in_allowlist = report.coverage.legacy_allowlisted_by_tag.contains_key(tag);
        let in_pending = report.coverage.legacy_pending_by_tag.contains_key(tag);
        match expected {
            TagClass::Allowlisted => {
                assert!(in_allowlist, "{tag} should be allowlisted");
                assert!(!in_pending, "{tag} should not be pending");
            }
            TagClass::Pending => {
                assert!(in_pending, "{tag} should be pending");
                assert!(!in_allowlist, "{tag} should not be allowlisted");
            }
        }
        assert!(*count > 0);
    }
}

#[test]
fn semantic_completion_counts_unsupported_too() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaign-mini/mini.pcc");
    if !path.exists() {
        return;
    }
    let (_pack, report) = compile_pcc(&path).expect("compile mini");
    let c = &report.coverage;
    let expected = c.lowered_effects as f64
        / (c.lowered_effects + c.legacy_pending() + c.unsupported_pending()) as f64;
    assert!((c.semantic_completion() - expected).abs() < f64::EPSILON);
    assert_eq!(
        c.semantic_pending(),
        c.legacy_pending() + c.unsupported_pending()
    );
}
