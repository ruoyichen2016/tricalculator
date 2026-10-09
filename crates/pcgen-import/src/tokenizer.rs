//! Tokenizer for PCGen tab-separated `.lst` files.
//!
//! PCGen list files are line-oriented:
//!
//! ```text
//! Climb<TAB>KEYSTAT:STR<TAB>ACHECK:YES<TAB>TYPE:Strength.Standard
//! ```
//!
//! The first field is the definition's display name.  Remaining fields are
//! `TAG:value` pairs.  Values may themselves contain `|` or `.`, so the
//! tokenizer keeps the raw field as well as the split tag/value.

pub use pcgen_ir::tags::{is_known_tag, is_name_prefix_tag};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Location of a source line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: PathBuf,
    /// 1-based line number.
    pub line: usize,
    /// Logical source id, e.g. the `.pcc` that referenced this file.
    pub source_id: String,
}

impl SourceRef {
    pub fn new(path: impl Into<PathBuf>, line: usize, source_id: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            line,
            source_id: source_id.into(),
        }
    }

    pub fn display(&self) -> String {
        format!("{}:{}", self.path.display(), self.line)
    }
}

/// One `TAG:value` token from a `.lst` line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagToken {
    pub tag: String,
    pub value: String,
    /// Exact field text, including the `TAG:` prefix.
    pub raw: String,
    pub source: SourceRef,
}

impl TagToken {
    /// Reconstruct the PCGen token, e.g. `BONUS:AC|1|TYPE=Armor`.
    pub fn full(&self) -> String {
        format!("{}:{}", self.tag, self.value)
    }

    pub fn pipe_parts(&self) -> Vec<&str> {
        self.value.split('|').map(str::trim).collect()
    }

    pub fn is_tag(&self, tag: &str) -> bool {
        self.tag.eq_ignore_ascii_case(tag)
    }
}

/// A parsed `.lst` line before typed interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLstLine {
    /// First tab-separated field, e.g. `Climb` or `Climb.MOD`.
    pub name: String,
    pub tags: Vec<TagToken>,
    pub source: SourceRef,
    pub raw: String,
}

impl RawLstLine {
    /// Explicit `KEY:` tag, if any.
    pub fn explicit_key(&self) -> Option<&str> {
        self.tags
            .iter()
            .find(|tag| tag.is_tag("KEY"))
            .map(|tag| tag.value.as_str())
    }

    /// This line's key after applying `.MOD` / `.COPY=` semantics.
    pub fn key(&self) -> String {
        if let Some(explicit) = self.explicit_key() {
            return explicit.to_string();
        }
        self.name
            .trim_end_matches(".MOD")
            .trim_end_matches(".FORGET")
            .split(".COPY=")
            .next()
            .unwrap_or(&self.name)
            .to_string()
    }

    pub fn tag(&self, tag: &str) -> Option<&TagToken> {
        self.tags.iter().find(|token| token.is_tag(tag))
    }

    pub fn tags_named<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a TagToken> + 'a {
        self.tags.iter().filter(move |token| token.is_tag(tag))
    }
}

/// Tokenize `.lst` text.
pub fn tokenize_lst(
    path: impl Into<PathBuf>,
    source_id: impl Into<String>,
    text: &str,
) -> Vec<RawLstLine> {
    let path = path.into();
    let source_id = source_id.into();
    let mut result = Vec::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = raw_line.trim_end_matches('\r');
        let line = trimmed.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = trimmed.split('\t').map(str::trim).collect();
        let Some(first_field) = fields.first().copied() else {
            continue;
        };
        if first_field.is_empty() {
            continue;
        }
        let source = SourceRef::new(path.clone(), line_number, source_id.clone());
        let (name, first_tag) = if let Some((tag, value)) = first_field.split_once(':') {
            let tag = tag.trim().to_ascii_uppercase();
            if is_name_prefix_tag(&tag) {
                (
                    value.trim().to_string(),
                    Some(TagToken {
                        tag,
                        value: value.trim().to_string(),
                        raw: first_field.to_string(),
                        source: source.clone(),
                    }),
                )
            } else {
                (first_field.to_string(), None)
            }
        } else {
            (first_field.to_string(), None)
        };
        if name.is_empty() {
            continue;
        }
        let mut tags = Vec::new();
        if let Some(first_tag) = first_tag {
            tags.push(first_tag);
        }
        for field in fields.iter().skip(1) {
            if field.is_empty() {
                continue;
            }
            if let Some((tag, value)) = field.split_once(':') {
                tags.push(TagToken {
                    tag: tag.trim().to_ascii_uppercase(),
                    value: value.trim().to_string(),
                    raw: (*field).to_string(),
                    source: source.clone(),
                });
            } else {
                tags.push(TagToken {
                    tag: field.trim().to_ascii_uppercase(),
                    value: String::new(),
                    raw: (*field).to_string(),
                    source: source.clone(),
                });
            }
        }
        result.push(RawLstLine {
            name: name.to_string(),
            tags,
            source,
            raw: trimmed.to_string(),
        });
    }

    result
}

/// Read and tokenize one `.lst` file.
pub fn tokenize_lst_file(
    path: impl AsRef<Path>,
    source_id: impl Into<String>,
) -> std::io::Result<Vec<RawLstLine>> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    Ok(tokenize_lst(path.to_path_buf(), source_id, &text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_tabs_and_preserves_unknown_tags() {
        let text = "Climb\tKEYSTAT:STR\tACHECK:YES\tBONUS:SKILL|Climb|2|TYPE=Racial\tUNKNOWN:value";
        let lines = tokenize_lst("/tmp/skills.lst", "test", text);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].name, "Climb");
        assert_eq!(lines[0].tags.len(), 4);
        assert_eq!(lines[0].tag("KEYSTAT").unwrap().value, "STR");
        assert_eq!(lines[0].tag("UNKNOWN").unwrap().value, "value");
        assert_eq!(lines[0].source.line, 1);
        assert!(!is_known_tag("UNKNOWN"));
    }

    #[test]
    fn mod_and_copy_keys() {
        let line = RawLstLine {
            name: "Climb.MOD".to_string(),
            tags: vec![],
            source: SourceRef::new("/tmp/skills.lst", 1, "test"),
            raw: "Climb.MOD".to_string(),
        };
        assert_eq!(line.key(), "Climb");

        let copy = RawLstLine {
            name: "Climb.COPY=Swim".to_string(),
            tags: vec![],
            source: SourceRef::new("/tmp/skills.lst", 2, "test"),
            raw: "Climb.COPY=Swim".to_string(),
        };
        assert_eq!(copy.key(), "Climb");
    }
}
