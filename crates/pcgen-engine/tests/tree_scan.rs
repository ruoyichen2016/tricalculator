//! Whole-tree import sanity scan.
//!
//! This is a diagnostic, not a gate: it walks the raw data tree and reports
//! files whose class level rows were keyed after something that is not a class.
//!
//! Why it exists: the RSRD acceptance set is small, and two bugs of the same
//! shape lived in it undetected — `SUBCLASS:` and `SUBSTITUTIONCLASS:` rows
//! were taken for class headers, so `current_class` moved onto the specialty
//! and every following level row was keyed `<specialty>#<level>` instead of
//! `<class>#<level>`.  The engine activates a level's class features by that
//! key, so those features silently never applied.  The RSRD has no
//! `SUBSTITUTIONCLASS:` rows at all, so only a wider scan can see them.
//!
//! The invariant is self-contained: `lower_records` names a class level entity
//! `<class key>#<level>`, and that class key must name a class the same pack
//! declares.  Anything else means the cursor was somewhere it should not be.
//!
//! Run with:
//! ```text
//! cargo test -p pcgen-engine --test tree_scan -- --ignored --nocapture
//! ```

use pcgen_import::{lower_records, tokenize_lst, RawRecord, RawTag, SourceRef};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Raw data trees to scan, relative to the checkout root.  The raw upstream
/// data lives beside the Rust workspace rather than inside it.
const TREES: [&str; 3] = [
    "../raw/pcgen-master/data/35e",
    "../raw/pcgen-master/data/3e",
    "../raw/pcgen-master/data/pathfinder",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Collect `.lst` files that could contain class level rows.
fn candidate_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "lst") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Build records from one `.lst` the same way the loader does.
fn records_for(path: &Path, text: &str) -> Vec<RawRecord> {
    let source_id = "scan".to_string();
    tokenize_lst(path.to_path_buf(), source_id.clone(), text)
        .into_iter()
        .map(|line| {
            let source = SourceRef::new(
                path.to_path_buf(),
                line.source.line as u32,
                source_id.clone(),
            );
            RawRecord {
                category: "CLASS".to_string(),
                key: line.key(),
                name: line.name.clone(),
                tags: line
                    .tags
                    .iter()
                    .map(|tag| RawTag {
                        name: tag.tag.clone(),
                        value: tag.value.clone(),
                        source: source.clone(),
                    })
                    .collect(),
                source,
                raw: line.raw.clone(),
            }
        })
        .collect()
}

/// Mirror `pcgen_core::identity`'s key normalization.
///
/// Entity keys are normalized (lowercased, spaces to underscores) while
/// `pack.classes` keeps the raw key, so comparing them without this reports
/// every legitimate class as suspicious.
fn normalize_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.contains(':') || trimmed.contains('|') {
        trimmed.to_string()
    } else {
        trimmed.to_ascii_lowercase().replace(' ', "_")
    }
}

/// Why a level entity key does not name a declared class.
#[derive(Debug, PartialEq, Eq)]
enum Kind {
    /// The cursor held a name that is not a class at all — a `SUBCLASS:` or
    /// `SUBSTITUTIONCLASS:` row, say.  This is the bug the scan exists for.
    Hijacked,
    /// The cursor was empty, but the file carries `.MOD` class rows, so its
    /// base class lives elsewhere.  Expected: many books split a class across
    /// files, and lowering one on its own cannot see the header.
    OrphanMod,
    /// The cursor was empty, the file declares no class, and it carries no
    /// `.MOD` row either, yet it has level rows.  Those rows can never attach
    /// to anything.
    Orphan,
    /// The cursor was empty even though the file *does* declare a class, which
    /// means the level rows appear before the header that owns them.
    OrphanDeclared,
}

/// A level entity key that does not name a declared class.
#[derive(Debug)]
struct Finding {
    file: String,
    key: String,
    kind: Kind,
}

#[test]
#[ignore = "diagnostic scan over the whole data tree"]
fn level_rows_are_always_keyed_by_a_declared_class() {
    let root = workspace_root();
    let mut findings: Vec<Finding> = Vec::new();
    let mut missing_trees: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    let mut with_levels = 0usize;
    let mut unreadable: Vec<String> = Vec::new();

    for tree in TREES {
        let tree_root = root.join(tree);
        if !tree_root.is_dir() {
            missing_trees.push(tree.to_string());
            continue;
        }
        for path in candidate_files(&tree_root) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Cheap pre-filter: a class level row starts with a number and a tab.
            if !text.lines().any(|line| {
                line.split_once('\t')
                    .is_some_and(|(first, _)| first.trim().parse::<u32>().is_ok())
            }) {
                continue;
            }
            scanned += 1;
            let records = records_for(&path, &text);
            // A `.MOD` row names a class defined elsewhere, so a file that has
            // one is expected to carry level rows without a header.
            let has_class_mod_row = records
                .iter()
                .any(|record| record.name.contains(".MOD") && !record.name.contains(".COPY="));
            let declares_class = records.iter().any(|record| {
                pcgen_import::is_class_header_row(record)
                    && !record.name.contains(".MOD")
                    && !record.name.contains(".COPY=")
            });
            let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);

            let declared: BTreeSet<String> =
                pack.classes.keys().map(|key| normalize_key(key)).collect();
            let mut found_here = false;
            for entity in &pack.entities {
                if !format!("{:?}", entity.category).contains("ClassLevel") {
                    continue;
                }
                found_here = true;
                let key = entity.id.key.as_str();
                let Some((class_key, _level)) = key.rsplit_once('#') else {
                    findings.push(Finding {
                        file: path.display().to_string(),
                        key: key.to_string(),
                        kind: Kind::Hijacked,
                    });
                    continue;
                };
                if !declared.contains(&normalize_key(class_key)) {
                    let kind = if class_key == "?" {
                        if has_class_mod_row {
                            Kind::OrphanMod
                        } else if declares_class {
                            Kind::OrphanDeclared
                        } else {
                            Kind::Orphan
                        }
                    } else {
                        Kind::Hijacked
                    };
                    findings.push(Finding {
                        file: path.display().to_string(),
                        key: key.to_string(),
                        kind,
                    });
                }
            }
            if found_here {
                with_levels += 1;
            } else {
                unreadable.push(path.display().to_string());
            }
        }
    }

    if !missing_trees.is_empty() {
        println!("SCAN missing trees (skipped)      : {missing_trees:?}");
    }
    let hijacked: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.kind == Kind::Hijacked)
        .collect();
    let orphans: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.kind == Kind::OrphanMod)
        .collect();
    let unattached: Vec<&Finding> = findings.iter().filter(|f| f.kind == Kind::Orphan).collect();
    let out_of_order: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.kind == Kind::OrphanDeclared)
        .collect();
    let hijacked_files: BTreeSet<&str> = hijacked.iter().map(|f| f.file.as_str()).collect();
    let orphan_files: BTreeSet<&str> = orphans.iter().map(|f| f.file.as_str()).collect();

    println!("SCAN candidate files scanned      : {scanned}");
    println!("SCAN files producing level rows   : {with_levels}");
    println!();
    println!("SCAN HIJACKED keys (real defect)  : {}", hijacked.len());
    println!("SCAN   distinct keys              : {}", {
        let set: BTreeSet<&str> = hijacked.iter().map(|f| f.key.as_str()).collect();
        set.len()
    });
    println!(
        "SCAN   files affected             : {}",
        hijacked_files.len()
    );
    for file in &hijacked_files {
        println!("SCAN   {file}");
    }
    for finding in hijacked.iter().take(20) {
        println!("SCAN     {} :: {}", finding.file, finding.key);
    }
    println!();
    println!(
        "SCAN UNATTACHED keys (no class at all in file): {}",
        unattached.len()
    );
    println!("SCAN   files affected             : {}", {
        let set: BTreeSet<&str> = unattached.iter().map(|f| f.file.as_str()).collect();
        set.len()
    });
    for finding in unattached.iter().take(12) {
        println!("SCAN   {} :: {}", finding.file, finding.key);
    }
    println!();
    println!(
        "SCAN OUT-OF-ORDER keys (rows before their header): {}",
        out_of_order.len()
    );
    println!("SCAN   files affected             : {}", {
        let set: BTreeSet<&str> = out_of_order.iter().map(|f| f.file.as_str()).collect();
        set.len()
    });
    for finding in out_of_order.iter().take(12) {
        println!("SCAN   {} :: {}", finding.file, finding.key);
    }
    println!();
    println!("SCAN ORPHAN keys (.MOD file)      : {}", orphans.len());
    println!("SCAN   distinct keys              : {}", {
        let set: BTreeSet<&str> = orphans.iter().map(|f| f.key.as_str()).collect();
        set.len()
    });
    println!("SCAN   files affected             : {}", orphan_files.len());
    for file in orphan_files.iter().take(12) {
        println!("SCAN   {file}");
    }
    if !unreadable.is_empty() {
        println!(
            "SCAN files with level-like rows but no level entities: {}",
            unreadable.len()
        );
        for path in unreadable.iter().take(10) {
            println!("SCAN   (no levels) {path}");
        }
    }
}

/// Cross-file check: every `.MOD` class row must have a class header somewhere
/// in the same data tree.
///
/// The per-file scan reports level rows that carry no header in their own file,
/// and explains them as `.MOD` files whose base lives elsewhere.  That is an
/// assumption until it is checked, and this checks it: a `.MOD` row with no
/// header anywhere is a modification of a class that does not exist, so its
/// level rows can never attach to anything.
///
/// This pass only tokenizes — no lowering — so it is cheap enough to run over
/// the whole tree.
#[test]
#[ignore = "diagnostic scan over the whole data tree"]
fn mod_class_rows_have_a_header_somewhere_in_the_tree() {
    let root = workspace_root();
    for tree in TREES {
        let tree_root = root.join(tree);
        if !tree_root.is_dir() {
            continue;
        }
        let mut headers: BTreeSet<String> = BTreeSet::new();
        let mut mods: BTreeSet<String> = BTreeSet::new();

        for path in candidate_files(&tree_root) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in tokenize_lst(path.clone(), "scan".to_string(), &text) {
                // The tokenizer strips a name-prefix tag, so `line.name` is
                // already the class name; the `CLASS` tag tells us the record
                // *is* a class rather than a level row.
                let Some(class_tag) = line.tag("CLASS") else {
                    continue;
                };
                let class_name = class_tag.value.trim();
                // `CLASS:Rogue.COPY=Rogue (CS)` *declares* `Rogue (CS)` by
                // copying `Rogue`, so the copy target is a header in its own
                // right.  Skipping these rows made `Rogue (CS)` look like a
                // `.MOD` target with no base (Cityscape, 3e).
                if let Some((_, copy_target)) = class_name.split_once(".COPY=") {
                    let copy_target = copy_target.trim();
                    if !copy_target.is_empty() {
                        headers.insert(normalize_key(copy_target));
                    }
                    continue;
                }
                if let Some(base) = class_name.strip_suffix(".MOD") {
                    mods.insert(normalize_key(base));
                } else if class_name.parse::<u32>().is_err()
                    && !pcgen_import::is_class_specialty_row(class_name)
                    && !class_name.starts_with("SUBSTITUTION")
                {
                    headers.insert(normalize_key(class_name));
                }
            }
        }

        let dangling: Vec<&String> = mods.difference(&headers).collect();
        println!(
            "SCAN2 {tree}: headers={} mod-targets={} dangling={}",
            headers.len(),
            mods.len(),
            dangling.len()
        );
        for key in dangling.iter().take(25) {
            println!("SCAN2   dangling .MOD base: {key}");
        }
    }
}
