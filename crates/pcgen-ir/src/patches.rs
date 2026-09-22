//! Explicit patch operations for `.MOD / .COPY / .CLEAR / .FORGET`.

use crate::source::SourceRef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PatchOperation {
    Modify {
        category: String,
        key: String,
        source: SourceRef,
    },
    Copy {
        category: String,
        source_key: String,
        target_key: String,
        source: SourceRef,
    },
    Forget {
        category: String,
        key: String,
        source: SourceRef,
    },
    Clear {
        category: String,
        key: String,
        tag_name: String,
        source: SourceRef,
    },
}
