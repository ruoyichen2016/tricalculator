//! Record-level patch semantics for `.MOD / .COPY= / .CLEAR / .FORGET`.
//!
//! The patch pass runs before typed lowering so both typed definitions and
//! Rule IR see the same effective source records.

use crate::source::{RawRecord, RawTag};
use indexmap::IndexMap;

/// Apply PCGen patch operations to a record stream.
pub fn apply_record_patches(records: &[RawRecord]) -> Vec<RawRecord> {
    let mut output: Vec<RawRecord> = Vec::new();
    let mut index_by_key: IndexMap<String, usize> = IndexMap::new();
    // `.MOD` rows whose base has not been seen yet.  PCGen's order is "Native
    // line, COPY, MOD, then FORGET", so a MOD is applied after the native line
    // however the two are ordered in the stream.
    let mut deferred_mods: Vec<(String, RawRecord)> = Vec::new();

    for record in records {
        let category = record.category.to_ascii_uppercase();
        let name = record.name.trim();
        let key = record.key.to_lowercase();
        let map_key = format!("{category}::{key}");

        // Numeric class rows are level rows, not patchable definitions.
        if category == "CLASS" && name.parse::<u32>().is_ok() {
            output.push(record.clone());
            continue;
        }

        if name.ends_with(".MOD") {
            // For CLASS blocks the `.MOD` line is also the header that
            // carries `current_class` into following `SUBSTITUTIONCLASS` /
            // `SUBSTITUTIONLEVEL` rows.  Keep it in the stream; typed
            // lowering's `merge_class` applies it to the base class.
            if category == "CLASS" {
                output.push(record.clone());
                index_by_key.insert(map_key, output.len() - 1);
            } else if let Some(&index) = index_by_key.get(&map_key) {
                let mut base = output[index].clone();
                merge_tags_with_clear(&mut base.tags, &record.tags);
                output[index] = base;
            } else {
                // No base yet: hold the MOD until the pass is over, then apply
                // it to whatever base turned up.
                deferred_mods.push((map_key.clone(), record.clone()));
            }
        } else if let Some((_, target)) = name.split_once(".COPY=") {
            let target = target.trim().to_string();
            if let Some(&index) = index_by_key.get(&map_key) {
                let mut copy = output[index].clone();
                copy.key = target.clone();
                copy.name = target.clone();
                merge_tags_with_clear(&mut copy.tags, &record.tags);
                output.push(copy);
                let target_key = format!("{category}::{}", target.to_lowercase());
                index_by_key.insert(target_key, output.len() - 1);
            }
        } else if name.ends_with(".FORGET") {
            if let Some(&index) = index_by_key.get(&map_key) {
                output[index].category = "__REMOVED__".to_string();
            }
        } else {
            output.push(record.clone());
            index_by_key.insert(map_key, output.len() - 1);
        }
    }

    for (map_key, record) in deferred_mods {
        if let Some(&index) = index_by_key.get(&map_key) {
            let mut base = output[index].clone();
            merge_tags_with_clear(&mut base.tags, &record.tags);
            output[index] = base;
        } else {
            // Still no base: keep the row so typed lowering can report it
            // rather than silently dropping the source.
            output.push(record);
        }
    }

    output
        .into_iter()
        .filter(|record| record.category != "__REMOVED__")
        .collect()
}

fn merge_tags_with_clear(target: &mut Vec<RawTag>, incoming: &[RawTag]) {
    for tag in incoming {
        let value = tag.value.trim();
        if value.eq_ignore_ascii_case(".CLEAR") {
            target.retain(|existing| !existing.name.eq_ignore_ascii_case(&tag.name));
            continue;
        }
        if let Some(specific) = value.strip_prefix(".CLEAR.") {
            target.retain(|existing| {
                !(existing.name.eq_ignore_ascii_case(&tag.name)
                    && existing.value.eq_ignore_ascii_case(specific))
            });
            continue;
        }
        target.push(tag.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceRef;

    fn record(name: &str, category: &str, tags: &[(&str, &str)]) -> RawRecord {
        let source = SourceRef::new("/tmp/test.lst", 1, "test.pcc");
        RawRecord {
            category: category.to_string(),
            // Mirror `RawLstLine::key`: `.MOD` and `.FORGET` come off, and
            // everything from `.COPY=` on is the target rather than the base.
            // This used to be `name.split('.').next()`, which the parser never
            // produces — the FORGET test passed against a fiction.
            key: name
                .trim_end_matches(".MOD")
                .trim_end_matches(".FORGET")
                .split(".COPY=")
                .next()
                .unwrap_or(name)
                .to_string(),
            name: name.to_string(),
            tags: tags
                .iter()
                .map(|(name, value)| RawTag {
                    name: (*name).to_string(),
                    value: (*value).to_string(),
                    source: source.clone(),
                })
                .collect(),
            source,
            raw: String::new(),
        }
    }

    #[test]
    fn mod_clear_and_copy_are_applied_in_order() {
        let records = vec![
            record("Fighter", "CLASS", &[("HD", "8"), ("TYPE", "Base")]),
            record(
                "Fighter.MOD",
                "CLASS",
                &[("TYPE", ".CLEAR"), ("TYPE", "Prestige")],
            ),
            record("Fighter.COPY=Knight", "CLASS", &[("MAXLEVEL", "10")]),
        ];
        let patched = apply_record_patches(&records);
        // CLASS `.MOD` rows are intentionally kept as headers so substitution
        // blocks can retain `current_class`; typed lowering merges them later.
        assert_eq!(patched.len(), 3);
        let fighter = patched
            .iter()
            .find(|record| record.key == "Fighter" && !record.name.ends_with(".MOD"))
            .expect("fighter");
        assert!(fighter
            .tags
            .iter()
            .any(|tag| tag.name == "TYPE" && tag.value == "Base"));
        let fighter_mod = patched
            .iter()
            .find(|record| record.name == "Fighter.MOD")
            .expect("fighter mod header");
        assert!(fighter_mod
            .tags
            .iter()
            .any(|tag| tag.name == "TYPE" && tag.value == "Prestige"));
        let knight = patched
            .iter()
            .find(|record| record.key == "Knight")
            .expect("knight");
        assert!(knight.tags.iter().any(|tag| tag.name == "MAXLEVEL"));
    }

    #[test]
    fn forget_removes_definition() {
        let records = vec![
            record("Fighter", "CLASS", &[("HD", "8")]),
            record("Fighter.FORGET", "CLASS", &[]),
        ];
        assert!(apply_record_patches(&records).is_empty());
    }
}
