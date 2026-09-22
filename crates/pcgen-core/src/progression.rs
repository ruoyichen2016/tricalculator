//! Shared progression enums used by the runtime IR.

use serde::{Deserialize, Serialize};

/// The three d20 saving throw categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind {
    Fortitude,
    Reflex,
    Will,
}

impl SaveKind {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Fortitude => "Fortitude",
            Self::Reflex => "Reflex",
            Self::Will => "Will",
        }
    }
}
