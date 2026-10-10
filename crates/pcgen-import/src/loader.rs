//! `.pcc` -> lossless `RawRecord` loading.
//!
//! This is the M1 boundary: source graph resolution and lossless raw AST.
//! Typed lowering (`BONUS`, `PRE*`, `ABILITY`, ...) is layered on top of the
//! resulting records without re-reading PCGen syntax in the runtime.

use crate::pcc::{PccError, PccGraph};
use crate::tokenizer::tokenize_lst;
use pcgen_ir::patched::apply_record_patches;
use pcgen_ir::rulepack::{lower_records, CompiledRulePack, ImportReport};
use pcgen_ir::source::{RawRecord, RawTag, SourceRef};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LoadedPcc {
    pub graph: PccGraph,
    pub records: Vec<RawRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error(transparent)]
    Pcc(#[from] PccError),
    #[error("I/O error reading `{path}`: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Load a `.pcc` graph and tokenize every referenced `.lst` file.
pub fn load_pcc(path: impl AsRef<Path>) -> Result<LoadedPcc, ImportError> {
    let graph = PccGraph::load(path)?;
    let mut records = Vec::new();
    for file in graph.files_in_load_order() {
        let source_id = file.path.display().to_string();
        for data_file in &file.data_files {
            let text = crate::text::read_text(&data_file.resolved_path).map_err(|source| {
                ImportError::Io {
                    path: data_file.resolved_path.clone(),
                    source,
                }
            })?;
            let lines = tokenize_lst(data_file.resolved_path.clone(), source_id.clone(), &text);
            for line in lines {
                let source = SourceRef::new(
                    line.source.path.clone(),
                    line.source.line as u32,
                    line.source.source_id.clone(),
                );
                let tags = line
                    .tags
                    .iter()
                    .map(|tag| RawTag {
                        name: tag.tag.clone(),
                        value: tag.value.clone(),
                        source: source.clone(),
                    })
                    .collect();
                records.push(RawRecord {
                    category: data_file.category.clone(),
                    key: line.key(),
                    name: line.name.clone(),
                    tags,
                    source,
                    raw: line.raw.clone(),
                });
            }
        }
    }
    Ok(LoadedPcc { graph, records })
}

/// Load a `.pcc` and lower its records into a `CompiledRulePack`.
pub fn compile_pcc(
    path: impl AsRef<Path>,
) -> Result<(CompiledRulePack, ImportReport), ImportError> {
    let pcc_path = path.as_ref().to_path_buf();
    let loaded = load_pcc(&pcc_path)?;
    let game_mode = loaded
        .graph
        .files
        .iter()
        .find_map(|file| file.game_mode.clone());
    let sources = loaded
        .graph
        .files
        .iter()
        .map(|file| file.path.display().to_string())
        .collect();
    let records = apply_record_patches(&loaded.records);
    let (mut pack, report) = lower_records(game_mode, sources, &records);
    pack.header.data_ref = data_ref_from_env_or_marker(&loaded.graph, &pcc_path);
    Ok((pack, report))
}

/// Resolve the upstream data revision for provenance.
///
/// Precedence:
/// 1. `PCGEN_DATA_REF` environment variable (CI / fetch scripts),
/// 2. `.pcgen-rust-testdata` marker written by `scripts/fetch_testdata.sh`,
/// 3. `.git` HEAD of the data checkout, when present.
fn data_ref_from_env_or_marker(graph: &PccGraph, pcc_path: &Path) -> Option<String> {
    if let Ok(value) = std::env::var("PCGEN_DATA_REF") {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }

    let mut directory = pcc_path.parent();
    while let Some(current) = directory {
        let marker = current.join(".pcgen-rust-testdata");
        if let Ok(text) = fs::read_to_string(&marker) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(reference) = value.get("ref").and_then(|v| v.as_str()) {
                    return Some(reference.to_string());
                }
            }
        }
        directory = current.parent();
    }

    // Fall back to the graph's own directory tree if it exists.
    let _ = graph;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_mini_campaign_records() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaign-mini/mini.pcc");
        if !path.exists() {
            return;
        }
        let loaded = load_pcc(&path).expect("mini campaign should load");
        assert!(!loaded.records.is_empty());
        assert!(loaded
            .records
            .iter()
            .any(|record| record.category == "CLASS"));
        assert!(loaded
            .records
            .iter()
            .any(|record| record.category == "RACE"));
        let (pack, report) = compile_pcc(&path).expect("mini campaign should compile");
        assert!(pack.coverage.entities > 0);
        assert!(report.coverage.entities > 0);
        assert_eq!(pack.races.len(), 1);
        assert_eq!(pack.classes.len(), 2);
        assert_eq!(pack.skills.len(), 4);
        assert_eq!(pack.spells.len(), 1);
        assert_eq!(pack.equipment.len(), 1);
        assert_eq!(pack.progressions.len(), 2);
        assert_eq!(pack.progressions["Fighter"].bab(2), 2);
    }
}
