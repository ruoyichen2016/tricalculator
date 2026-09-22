//! Source-aware raw records produced by the compatibility layer.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A source file and line reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: PathBuf,
    pub line: u32,
    pub source_id: String,
}

impl SourceRef {
    pub fn new(path: impl Into<PathBuf>, line: u32, source_id: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            line,
            source_id: source_id.into(),
        }
    }
}

/// One raw `TAG:value` pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTag {
    pub name: String,
    pub value: String,
    pub source: SourceRef,
}

/// One lossless `.lst` record after tab/first-colon splitting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRecord {
    pub category: String,
    pub key: String,
    pub name: String,
    pub tags: Vec<RawTag>,
    pub source: SourceRef,
    pub raw: String,
}

impl RawRecord {
    pub fn tag(&self, name: &str) -> Option<&RawTag> {
        self.tags
            .iter()
            .find(|tag| tag.name.eq_ignore_ascii_case(name))
    }
}
