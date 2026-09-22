//! Integration tests for `.pcc` include graph loading.

use pcgen_import::PccGraph;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pcc")
        .join(name)
}

#[test]
fn pcc_includes_are_loaded_in_dependency_order() {
    let graph = PccGraph::load(fixture("root.pcc")).expect("root pcc should load");
    assert_eq!(graph.files.len(), 3);
    assert_eq!(graph.campaign_name(), Some("Fixture Root"));
    let order: Vec<String> = graph
        .files_in_load_order()
        .map(|file| file.campaign.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        order,
        ["Fixture Grandchild", "Fixture Child", "Fixture Root"]
    );
    let data_files: Vec<&str> = graph
        .data_files_in_load_order()
        .iter()
        .map(|file| file.raw.as_str())
        .collect();
    assert_eq!(
        data_files,
        [
            "data/grandchild_skills.lst",
            "data/child_races.lst",
            "data/root_classes.lst"
        ]
    );
    assert!(graph.validate_data_files().is_empty());
}

#[test]
fn pcc_cycles_are_rejected() {
    let result = PccGraph::load(fixture("cycle_a.pcc"));
    assert!(result.is_err());
}
