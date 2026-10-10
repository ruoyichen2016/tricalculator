//! PCGen campaign (`.pcc`) reader.
//!
//! `.pcc` files are the entry points for PCGen datasets.  They contain a
//! `CAMPAIGN`/`KEY`, a game mode, zero or more `PCC:` include files, and a
//! list of data files such as `CLASS:`, `RACE:`, `SKILL:`, and `SPELL:`.
//!
//! Loading order matters: included campaigns must be loaded before the file
//! that includes them.  This module builds the include graph and resolves it
//! with the core topological-sort algorithm (CLRS 22.4).

use pcgen_core::algorithms::{strongly_connected_components, topological_sort};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A data-file entry from a `.pcc` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PccDataFile {
    /// PCGen category, e.g. `CLASS` or `RACE`.
    pub category: String,
    /// Raw value as written in the `.pcc`.
    pub raw: String,
    /// Resolved path relative to the `.pcc` file.
    pub resolved_path: PathBuf,
    /// 1-based source line in the `.pcc`.
    pub line: usize,
}

/// A `.pcc` include entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PccInclude {
    /// Raw value as written in the `.pcc`.
    pub raw: String,
    /// Resolved path relative to the including `.pcc`.
    pub resolved_path: PathBuf,
}

/// Parsed representation of one `.pcc` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PccFile {
    pub path: PathBuf,
    pub campaign: Option<String>,
    pub key: Option<String>,
    pub game_mode: Option<String>,
    pub includes: Vec<PccInclude>,
    pub data_files: Vec<PccDataFile>,
    /// All key/value pairs, including unknown tags.
    pub tags: Vec<(String, String)>,
}

impl PccFile {
    /// Parse `.pcc` text.
    pub fn parse_str(path: impl Into<PathBuf>, text: &str) -> Result<Self, PccError> {
        let path = path.into();
        let mut file = Self {
            path: path.clone(),
            campaign: None,
            key: None,
            game_mode: None,
            includes: Vec::new(),
            data_files: Vec::new(),
            tags: Vec::new(),
        };

        for (line_index, raw_line) in text.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim_end_matches('\r').trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((raw_tag, raw_value)) = line.split_once(':') else {
                continue;
            };
            let tag = raw_tag.trim().to_ascii_uppercase();
            let value = raw_value.trim();
            file.tags.push((tag.clone(), value.to_string()));

            match tag.as_str() {
                "CAMPAIGN" => file.campaign = Some(value.to_string()),
                "KEY" => file.key = Some(value.to_string()),
                "GAMEMODE" => file.game_mode = Some(value.to_string()),
                "PCC" | "INCLUDE" => {
                    if value.is_empty() {
                        continue;
                    }
                    let resolved_path = resolve_path(&path, value);
                    file.includes.push(PccInclude {
                        raw: value.to_string(),
                        resolved_path,
                    });
                }
                _ if is_data_category(&tag) => {
                    if value.is_empty() {
                        continue;
                    }
                    // PCGen data-file entries may carry inline `|PRE...`
                    // conditions; only the path portion is resolved here.
                    let path_value = value.split('|').next().unwrap_or(value).trim();
                    if path_value.is_empty() {
                        continue;
                    }
                    let resolved_path = resolve_path(&path, path_value);
                    file.data_files.push(PccDataFile {
                        category: tag,
                        raw: value.to_string(),
                        resolved_path,
                        line: line_number,
                    });
                }
                _ => {}
            }
        }

        if file.campaign.is_none() && file.key.is_none() {
            return Err(PccError::Parse {
                path: path.clone(),
                line: 0,
                message: "missing CAMPAIGN and KEY tags".to_string(),
            });
        }
        Ok(file)
    }

    /// Load and parse a `.pcc` file from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PccError> {
        let path = path.as_ref();
        let text = crate::text::read_text(path).map_err(|source| PccError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse_str(path.to_path_buf(), &text)
    }
}

/// A resolved `.pcc` include graph.
#[derive(Debug, Clone)]
pub struct PccGraph {
    pub root: PathBuf,
    pub files: Vec<PccFile>,
    /// `adjacency[child]` contains parents that include `child`.
    pub adjacency: Vec<Vec<usize>>,
    /// Indices into `files`, dependency-first.
    pub load_order: Vec<usize>,
}

impl PccGraph {
    /// Load a `.pcc` and all of its transitive `PCC:` includes.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, PccError> {
        let root = resolve_existing(root.as_ref())?;
        let mut files = Vec::new();
        let mut index = HashMap::new();
        let mut adjacency = Vec::new();
        let root_index = load_recursive(&root, &mut files, &mut index, &mut adjacency)?;

        let cyclic_nodes: Vec<usize> = strongly_connected_components(&adjacency)
            .into_iter()
            .filter(|component| {
                component.len() > 1
                    || component
                        .first()
                        .is_some_and(|node| adjacency[*node].contains(node))
            })
            .flatten()
            .collect();
        if !cyclic_nodes.is_empty() {
            return Err(PccError::Cycle {
                files: cyclic_nodes
                    .iter()
                    .filter_map(|index| files.get(*index))
                    .map(|file| file.path.clone())
                    .collect(),
            });
        }
        let load_order = topological_sort(&adjacency).ok_or_else(|| PccError::Cycle {
            files: files.iter().map(|file| file.path.clone()).collect(),
        })?;

        // Keep the root index available for callers without another lookup.
        debug_assert_eq!(
            load_order.iter().filter(|&&idx| idx == root_index).count(),
            1
        );

        Ok(Self {
            root,
            files,
            adjacency,
            load_order,
        })
    }

    /// Files in dependency-first load order.
    pub fn files_in_load_order(&self) -> impl Iterator<Item = &PccFile> {
        self.load_order.iter().map(|&index| &self.files[index])
    }

    /// All `.lst` data files in dependency-first load order.
    pub fn data_files_in_load_order(&self) -> Vec<&PccDataFile> {
        self.files_in_load_order()
            .flat_map(|file| file.data_files.iter())
            .collect()
    }

    /// Verify that every referenced data file exists.
    pub fn validate_data_files(&self) -> Vec<PccValidationError> {
        let mut errors = Vec::new();
        for file in &self.files {
            for data_file in &file.data_files {
                if !data_file.resolved_path.exists() {
                    errors.push(PccValidationError::MissingDataFile {
                        pcc: file.path.clone(),
                        category: data_file.category.clone(),
                        path: data_file.resolved_path.clone(),
                    });
                }
            }
        }
        errors
    }

    pub fn campaign_name(&self) -> Option<&str> {
        self.files
            .get(*self.load_order.last()?)
            .and_then(|file| file.campaign.as_deref())
    }
}

/// Graph validation problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PccValidationError {
    MissingDataFile {
        pcc: PathBuf,
        category: String,
        path: PathBuf,
    },
}

/// Errors produced by `.pcc` parsing and graph loading.
#[derive(Debug, thiserror::Error)]
pub enum PccError {
    #[error("I/O error reading `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error in `{path}` at line {line}: {message}")]
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("missing include `{include}` referenced by `{parent}`")]
    MissingInclude { parent: PathBuf, include: PathBuf },
    #[error("cycle detected in PCC include graph: {files:?}")]
    Cycle { files: Vec<PathBuf> },
}

fn load_recursive(
    path: &Path,
    files: &mut Vec<PccFile>,
    index: &mut HashMap<PathBuf, usize>,
    adjacency: &mut Vec<Vec<usize>>,
) -> Result<usize, PccError> {
    let canonical = resolve_existing(path).map_err(|_| PccError::MissingInclude {
        parent: PathBuf::new(),
        include: path.to_path_buf(),
    })?;
    if let Some(&existing) = index.get(&canonical) {
        return Ok(existing);
    }

    let file = PccFile::load(&canonical)?;
    let child_paths: Vec<PathBuf> = file
        .includes
        .iter()
        .map(|include| include.resolved_path.clone())
        .collect();
    let current = files.len();
    files.push(file);
    index.insert(canonical, current);
    adjacency.push(Vec::new());

    for child_path in child_paths {
        let child =
            load_recursive(&child_path, files, index, adjacency).map_err(|error| match error {
                PccError::MissingInclude { include, .. } => PccError::MissingInclude {
                    parent: files[current].path.clone(),
                    include,
                },
                other => other,
            })?;
        if !adjacency[child].contains(&current) {
            adjacency[child].push(current);
        }
    }
    Ok(current)
}

fn resolve_existing(path: &Path) -> Result<PathBuf, PccError> {
    fs::canonicalize(path).map_err(|source| PccError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_path(base: &Path, raw: &str) -> PathBuf {
    let raw = raw.trim().replace('\\', "/");
    let raw = raw.as_str();
    if let Some(data_relative) = raw.strip_prefix("@/") {
        return resolve_data_root_path(base, data_relative);
    }
    let raw = raw.strip_prefix('@').unwrap_or(raw);
    if let Some(wildcard_tail) = raw.strip_prefix('*') {
        return resolve_wildcard_path(base, wildcard_tail);
    }
    let candidate = Path::new(raw);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(candidate)
    }
}

/// Resolve PCGen's `@/relative/path` notation: relative to the dataset
/// root, i.e. the ancestor directory named `data`.
fn resolve_data_root_path(base: &Path, tail: &str) -> PathBuf {
    let mut directory = base.parent();
    while let Some(current) = directory {
        if current.file_name().is_some_and(|name| name == "data") {
            return current.join(tail);
        }
        directory = current.parent();
    }
    base.parent().unwrap_or_else(|| Path::new(".")).join(tail)
}

/// Resolve PCGen's `*/relative/path.lst` wildcard.
///
/// PCGen datasets use paths such as `*/_universal/races.lst`; the `*`
/// means "search for this path in the data tree".  We walk up from the
/// `.pcc` directory and use the first existing match.
fn resolve_wildcard_path(base: &Path, tail: &str) -> PathBuf {
    let tail = tail.trim_start_matches(['/', '\\']);
    let mut directory = base.parent();
    while let Some(current) = directory {
        let candidate = current.join(tail);
        if candidate.exists() {
            return candidate;
        }
        directory = current.parent();
    }
    base.parent().unwrap_or_else(|| Path::new(".")).join(tail)
}

fn is_data_category(tag: &str) -> bool {
    matches!(
        tag,
        "ABILITY"
            | "ABILITYCATEGORY"
            | "ALIGNMENT"
            | "ARMORPROF"
            | "BIOSET"
            | "CLASS"
            | "COMPANIONMOD"
            | "DATACONTROL"
            | "DATATABLE"
            | "DEITY"
            | "DOMAIN"
            | "DYNAMIC"
            | "EQUIPMENT"
            | "EQUIPMOD"
            | "GLOBALMODIFIER"
            | "KIT"
            | "LANGUAGE"
            | "RACE"
            | "SAVE"
            | "SHIELDPROF"
            | "SIZE"
            | "SKILL"
            | "SPELL"
            | "STAT"
            | "TEMPLATE"
            | "WEAPONPROF"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_only_support_pcc() {
        let text = "\
KEY:support ~ dwarf race
RACE:dwarf_races.lst
";
        let file = PccFile::parse_str("/tmp/race/_race.pcc", text).unwrap();
        assert!(file.campaign.is_none());
        assert_eq!(file.key.as_deref(), Some("support ~ dwarf race"));
        assert_eq!(file.data_files.len(), 1);
    }

    #[test]
    fn parse_windows_style_include_and_conditioned_data_file() {
        let text = "\
CAMPAIGN:Pathfinder Test
KEY:pathfinder-test
PCC:@\\pathfinder\\paizo\\_core.pcc
CLASS:data/classes.lst|PRECAMPAIGN:1,BOOKTYPE=Core Rules
";
        let file = PccFile::parse_str("/tmp/test.pcc", text).unwrap();
        assert_eq!(
            file.includes[0].resolved_path,
            PathBuf::from("/tmp/pathfinder/paizo/_core.pcc")
        );
        assert_eq!(file.data_files.len(), 1);
        assert_eq!(
            file.data_files[0].resolved_path,
            PathBuf::from("/tmp/data/classes.lst")
        );
        assert!(file.data_files[0]
            .raw
            .contains("PRECAMPAIGN:1,BOOKTYPE=Core Rules"));
    }

    #[test]
    fn parse_minimal_pcc() {
        let text = "\
CAMPAIGN:Test Campaign
KEY:test-campaign
GAMEMODE:3e
PCC:child.pcc
CLASS:data/classes.lst
RACE:data/races.lst
";
        let file = PccFile::parse_str("/tmp/test.pcc", text).unwrap();
        assert_eq!(file.campaign.as_deref(), Some("Test Campaign"));
        assert_eq!(file.key.as_deref(), Some("test-campaign"));
        assert_eq!(file.game_mode.as_deref(), Some("3e"));
        assert_eq!(
            file.includes[0].resolved_path,
            PathBuf::from("/tmp/child.pcc")
        );
        assert_eq!(file.data_files.len(), 2);
        assert_eq!(file.data_files[0].category, "CLASS");
        assert_eq!(
            file.data_files[0].resolved_path,
            PathBuf::from("/tmp/data/classes.lst")
        );
    }
}
