use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use pcgen_builder::{
    create_draft, level_up_draft, preview_level_up, starting_gold_for_class, CharacterRepository,
    CreateRequest, CreateSpell, LevelUpLane, LevelUpRequest, NativeSaveEnvelope,
};
use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, ClassProgress, Engine};
use pcgen_import::draft_from_pcg;
use pcgen_import::{compile_pcc, lower_records, parse_pcg_draft_file};
use pcgen_output::{render, render_json, render_text, OutputFormat};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "pcgen", version, about = "PCGen-Rust character engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// CLI subcommands.
///
/// The `Create` variant is much larger than the others, but this is a one-shot
/// argv DTO, so the stack footprint does not matter; boxing the fields would
/// only make the clap derive less readable.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
enum Command {
    /// Render a built-in Gestalt demo snapshot.
    Demo,
    /// Write a `.pcc` campaign to a normalised pack directory (build step).
    ///
    /// This is the expensive half: it interprets the PCGen data once and
    /// persists the result.  `pack-info` then loads that directory without
    /// interpreting any PCGen text.
    Transpile {
        #[arg(long)]
        pcc: PathBuf,
        /// Directory to write the pack into.
        #[arg(long)]
        out: PathBuf,
    },
    /// Load a persisted pack directory and report what it holds.
    PackInfo {
        #[arg(long)]
        pack: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Load a real `.pcc` graph and print raw import coverage.
    ImportReport {
        pcc: PathBuf,
        #[arg(long)]
        json: bool,
        /// Print the unsupported/legacy tag histograms instead of the summary.
        #[arg(long)]
        histogram: bool,
        /// How many tags to show in the histogram (0 = all).
        #[arg(long, default_value_t = 0)]
        top: usize,
    },
    /// List creature templates available in a `.pcc` campaign.
    ListTemplates {
        #[arg(long)]
        pcc: PathBuf,
        /// Case-insensitive substring filter on template id or name.
        #[arg(long)]
        search: Option<String>,
        /// Only templates carrying this type tag (case-insensitive); repeatable.
        #[arg(long = "type-tag")]
        type_tags: Vec<String>,
        /// Include CR / hit die / level adjustments / tags in the text output.
        #[arg(long)]
        with_details: bool,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// List archetype / substitution variants and whether their `REPLACES`
    /// targets actually resolve against the class's own feature lines.
    ListVariants {
        #[arg(long)]
        pcc: PathBuf,
        /// Case-insensitive substring filter on variant id or name.
        #[arg(long)]
        search: Option<String>,
        /// Only variants applying to this class (case-insensitive).
        #[arg(long = "class")]
        class_id: Option<String>,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Parse a `.pcg` file into draft JSON.
    ParsePcg {
        pcg: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Recalculate a `.pcg` against a `.pcc` campaign.
    Recalculate {
        #[arg(long)]
        pcc: PathBuf,
        pcg: PathBuf,
        #[arg(long)]
        summary: bool,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Export a `.pcg` character to text or JSON.
    Export {
        pcg: PathBuf,
        #[arg(long)]
        pcc: Option<PathBuf>,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Evaluate a draft `.pcg` file.
    EvaluatePcg {
        pcg: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Evaluate a CharacterDraft JSON file.
    Evaluate {
        draft: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Create a new character and optionally write a native save envelope.
    Create {
        #[arg(long)]
        pcc: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        race: String,
        #[arg(long = "class")]
        class_id: String,
        #[arg(long, default_value_t = 1)]
        level: u32,
        #[arg(long, default_value = "")]
        alignment: String,
        /// Apply a creature template by id or name; repeat per template.
        #[arg(long = "template")]
        templates: Vec<String>,
        /// Stat assignment, e.g. `STR=14`; repeat per ability.
        #[arg(long = "stat")]
        stats: Vec<String>,
        /// Validate stats against the 3.5e DMG point-buy budget.
        #[arg(long)]
        point_buy: Option<u32>,
        /// Skill ranks, e.g. `Climb=4`; repeat per skill.
        #[arg(long = "skill")]
        skills: Vec<String>,
        /// Ability selection, e.g. `FEAT=Power Attack`.
        #[arg(long = "ability")]
        abilities: Vec<String>,
        /// Spell selection, e.g. `Magic Missile` or `Magic Missile=1`.
        #[arg(long = "spell")]
        spells: Vec<String>,
        #[arg(long = "equipment")]
        equipment: Vec<String>,
        /// Starting gold budget; equipment cost must fit.
        #[arg(long)]
        equipment_budget: Option<f64>,
        /// Use the class's deterministic average starting gold as budget.
        #[arg(long)]
        starting_gold: bool,
        #[arg(long)]
        deity: Option<String>,
        #[arg(long = "domain")]
        domains: Vec<String>,
        #[arg(long, default_value = "json")]
        format: String,
        /// Write a native save envelope instead of printing the snapshot.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Apply one level to a native save envelope.
    LevelUp {
        /// Native save envelope JSON written by `pcgen create`.
        draft: PathBuf,
        #[arg(long)]
        pcc: PathBuf,
        #[arg(long = "class")]
        class_id: String,
        /// Explicit HP gained; when omitted, use the class HD average.
        #[arg(long)]
        hp: Option<i32>,
        #[arg(long = "skill")]
        skills: Vec<String>,
        #[arg(long = "ability")]
        abilities: Vec<String>,
        #[arg(long = "spell")]
        spells: Vec<String>,
        /// Apply an additional creature template by id or name; repeat per template.
        #[arg(long = "template")]
        templates: Vec<String>,
        /// Replace the template list with `--template` instead of appending.
        #[arg(long)]
        replace_templates: bool,
        /// Archetype / alternate ability variant id applied to this level.
        #[arg(long)]
        archetype: Option<String>,
        /// Substitution-level variant id applied to this level.
        #[arg(long)]
        substitution_level: Option<String>,
        /// Subclass / specialty selection applied to this level.
        #[arg(long)]
        subclass: Option<String>,
        /// Extra Gestalt/multiclass lane, e.g. `--lane Wizard`; repeatable.
        #[arg(long = "lane")]
        lanes: Vec<String>,
        /// HP gained for the matching `--lane` (same order); repeatable.
        #[arg(long = "lane-hp")]
        lane_hp: Vec<i32>,
        /// Archetype id for the matching `--lane` (same order); repeatable.
        #[arg(long = "lane-archetype")]
        lane_archetype: Vec<String>,
        /// Substitution-level variant id for the matching `--lane`; repeatable.
        #[arg(long = "lane-substitution")]
        lane_substitution: Vec<String>,
        /// Subclass selection for the matching `--lane`; repeatable.
        #[arg(long = "lane-subclass")]
        lane_subclass: Vec<String>,
        /// Print the resulting snapshot without writing the save.
        #[arg(long)]
        preview: bool,
        /// Write the updated envelope here; otherwise print JSON to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Restore the version of a native character saved before its last write.
    Undo {
        /// Native save envelope JSON (id is read from the character).
        draft: PathBuf,
        /// Print the restored envelope instead of writing it back.
        #[arg(long)]
        dry_run: bool,
    },
    /// Re-apply the most recently undone version of a native character.
    Redo {
        /// Native save envelope JSON (id is read from the character).
        draft: PathBuf,
        /// Print the re-applied envelope instead of writing it back.
        #[arg(long)]
        dry_run: bool,
    },
    /// List the saved revisions available to `undo` / `redo`.
    History {
        /// Native save envelope JSON (id is read from the character).
        draft: PathBuf,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Bundle every native character under a directory into one JSON file.
    ExportAll {
        /// Directory holding `<id>.json` character saves.
        dir: PathBuf,
        /// Bundle path to write (defaults to stdout).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Import a character bundle produced by `export-all`.
    ImportBundle {
        /// Bundle JSON file written by `export-all`.
        bundle: PathBuf,
        /// Directory to write `<id>.json` saves into.
        dir: PathBuf,
        /// Only import characters matching this `.pcc`'s rules fingerprint.
        #[arg(long)]
        pcc: Option<PathBuf>,
        /// Print the fingerprint groups in the bundle instead of importing.
        #[arg(long)]
        list_groups: bool,
    },
    /// Delete one or more native characters from a directory.
    Delete {
        /// Directory holding `<id>.json` character saves.
        dir: PathBuf,
        /// Character id to delete; repeatable.
        #[arg(long = "id")]
        ids: Vec<String>,
        /// Delete every character in the directory.
        #[arg(long)]
        all: bool,
        /// List what would be deleted without removing anything.
        #[arg(long)]
        dry_run: bool,
    },
}

fn demo_engine() -> Engine {
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["demo".to_string()], &[]);
    let mut progressions = BTreeMap::new();
    progressions.insert(
        "Fighter".to_string(),
        ClassProgress {
            base_attack_bonus: vec![1, 2, 3, 4, 5],
            fortitude: vec![2, 3, 3, 4, 4],
            reflex: vec![0, 0, 1, 1, 1],
            will: vec![0, 0, 1, 1, 1],
            skill_points_per_level: 2,
            first_level_skill_points: 8,
            class_skills: vec!["Climb".to_string()],
            ..ClassProgress::default()
        },
    );
    progressions.insert(
        "Wizard".to_string(),
        ClassProgress {
            base_attack_bonus: vec![0, 1, 1, 2, 2],
            fortitude: vec![0, 0, 1, 1, 1],
            reflex: vec![0, 3, 3, 4, 4],
            will: vec![2, 3, 3, 4, 4],
            skill_points_per_level: 2,
            first_level_skill_points: 8,
            class_skills: vec!["Spellcraft".to_string()],
            ..ClassProgress::default()
        },
    );
    Engine::new(pack).with_progressions(progressions)
}

fn demo_draft() -> CharacterDraft {
    CharacterDraft {
        id: "demo".to_string(),
        name: "Gestalt Demo".to_string(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        alignment: "N".to_string(),
        gender: String::new(),
        character_type: pcgen_engine::default_character_type(),
        stats: BTreeMap::from([
            ("STR".to_string(), 14),
            ("DEX".to_string(), 12),
            ("CON".to_string(), 14),
            ("INT".to_string(), 14),
            ("WIS".to_string(), 10),
            ("CHA".to_string(), 10),
        ]),
        advancement: vec![AdvancementFrame::new(
            1,
            vec![
                AdvancementLane::new("a", "Fighter", 1, 10, 0),
                AdvancementLane::new("b", "Wizard", 1, 4, 0),
            ],
        )],
        skills: BTreeMap::new(),
        spells: Vec::new(),
        equipment: Vec::new(),
        equipment_modifiers: BTreeMap::new(),
        equipment_base_items: BTreeMap::new(),
        equipment_display_names: BTreeMap::new(),
        equip_slots: BTreeMap::new(),
        abilities: Vec::new(),
        templates: Vec::new(),
        active_states: Vec::new(),
        choices: BTreeMap::new(),
        companions: Vec::new(),
        class_subclasses: BTreeMap::new(),
        deity: None,
        domains: Vec::new(),
        deity_domains: Vec::new(),
        ..CharacterDraft::default()
    }
}

/// Write a native save through the repository so the previous version is
/// backed up and `pcgen undo` can restore it.
/// Anchors the dataset opens for a class, for `REPLACES` targets that missed.
///
/// Guessing similarity between two names proved unreliable (`MonkAC` vs
/// `MonkBonusAC` have no useful edit distance, and word-overlap rules started
/// matching unrelated anchors), so instead of inventing a "near miss" verdict
/// this reports what the class actually offers.  A human can then decide
/// whether the target is a spelling disagreement or an anchor the dataset
/// never opened.
fn available_anchors(features: &[String], class_id: &str) -> Vec<String> {
    let prefix = pcgen_core::feature_match_key(class_id);
    let mut anchors: Vec<String> = Vec::new();
    for line in features {
        for segment in line.split([',', '|']) {
            let segment = segment.trim();
            let Some(tag) = segment
                .strip_prefix("TYPE.")
                .or_else(|| segment.strip_prefix("type."))
            else {
                continue;
            };
            // Only class-namespaced anchors are interesting here; drop the
            // generic quality words (`SpecialQuality`, `Extraordinary`).
            let lower = tag.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "specialquality"
                    | "specialattack"
                    | "extraordinary"
                    | "supernatural"
                    | "spelllike"
                    | "classfeature"
                    | "classfeatures"
                    | "acf"
                    | "internal"
            ) {
                continue;
            }
            // Only anchors belonging to this class: the raw sweep sees every
            // class's guarded lines, and listing all of them buries the answer.
            if !prefix.is_empty() && !tag.to_ascii_lowercase().starts_with(&prefix) {
                continue;
            }
            if !anchors.iter().any(|existing| existing == tag) {
                anchors.push(tag.to_string());
            }
        }
    }
    anchors.sort();
    anchors
}

fn save_envelope(path: &std::path::Path, envelope: &NativeSaveEnvelope) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let root = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    CharacterRepository::new(root)
        .with_live_path(path)
        .save(envelope)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Pair repeatable `--lane` classes with repeatable `--lane-hp` values.
fn build_lanes(
    classes: &[String],
    hit_points: &[i32],
    archetypes: &[String],
    substitutions: &[String],
    subclasses: &[String],
) -> Vec<LevelUpLane> {
    let non_empty = |values: &[String], index: usize| {
        values
            .get(index)
            .filter(|value| !value.trim().is_empty())
            .cloned()
    };
    classes
        .iter()
        .enumerate()
        .map(|(index, class_id)| LevelUpLane {
            class_id: class_id.clone(),
            hp_gained: hit_points.get(index).copied(),
            archetype: non_empty(archetypes, index),
            substitution_level: non_empty(substitutions, index),
            subclass: non_empty(subclasses, index),
        })
        .collect()
}

fn parse_pairs(values: &[String]) -> Result<BTreeMap<String, i32>> {
    let mut out = BTreeMap::new();
    for value in values {
        let Some((key, raw)) = value.split_once('=') else {
            bail!("expected KEY=VALUE, got {value:?}");
        };
        let key = key.trim().to_ascii_uppercase();
        let score: i32 = raw
            .trim()
            .parse()
            .with_context(|| format!("invalid stat value in {value:?}"))?;
        out.insert(key, score);
    }
    Ok(out)
}

fn parse_skill_pairs(values: &[String]) -> Result<BTreeMap<String, f64>> {
    let mut out = BTreeMap::new();
    for value in values {
        let Some((key, raw)) = value.split_once('=') else {
            bail!("expected SKILL=RANKS, got {value:?}");
        };
        let ranks: f64 = raw
            .trim()
            .parse()
            .with_context(|| format!("invalid skill rank in {value:?}"))?;
        out.insert(key.trim().to_string(), ranks);
    }
    Ok(out)
}

fn parse_key_value_pairs(values: &[String]) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for value in values {
        let Some((key, raw)) = value.split_once('=') else {
            bail!("expected CATEGORY=KEY, got {value:?}");
        };
        if key.trim().is_empty() || raw.trim().is_empty() {
            bail!("empty category or key in {value:?}");
        }
        out.push((key.trim().to_string(), raw.trim().to_string()));
    }
    Ok(out)
}

fn parse_spells(values: &[String]) -> Vec<CreateSpell> {
    values
        .iter()
        .map(|value| {
            let (name, level) = match value.split_once('=') {
                Some((name, raw)) => (name.trim(), raw.trim().parse::<u32>().unwrap_or(0)),
                None => (value.trim(), 0),
            };
            CreateSpell {
                name: name.to_string(),
                level,
                times_memorized: 1,
                book: None,
            }
        })
        .collect()
}

/// Restore the default `SIGPIPE` behaviour on Unix so that piping output into
/// e.g. `head` terminates quietly instead of panicking on a failed `println!`.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: `signal` with `SIG_DFL` is async-signal-safe and has no Rust-level
    // invariants; it only restores the process default for `SIGPIPE`.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn main() -> Result<()> {
    reset_sigpipe();
    let cli = Cli::parse();
    match cli.command {
        Command::Demo => {
            let engine = demo_engine();
            let snapshot = engine.evaluate(&demo_draft());
            print!("{}", render_text(&snapshot));
        }
        Command::Transpile { pcc, out } => {
            let loaded = pcgen_import::load_pcc(&pcc)
                .with_context(|| format!("failed to import {}", pcc.display()))?;
            let patched = pcgen_import::apply_record_patches(&loaded.records);
            let (pack, _) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            let manifest =
                pcgen_transpile::write_pack(&out, &pack, &patched, &pcc.display().to_string())
                    .with_context(|| format!("failed to write pack to {}", out.display()))?;
            println!(
                "wrote pack `{}` to {} (format {})",
                manifest.name,
                out.display(),
                manifest.format
            );
            for (kind, count) in &manifest.counts {
                println!("  {kind:12} {count}");
            }
        }
        Command::PackInfo { pack, json } => {
            let loaded = pcgen_transpile::read_pack(&pack)
                .with_context(|| format!("failed to load pack {}", pack.display()))?;
            let runtime = &loaded.pack;
            if json {
                let value = serde_json::json!({
                    "manifest": loaded.manifest,
                    "entities": runtime.entities.len(),
                    "classes": runtime.classes.len(),
                    "races": runtime.races.len(),
                    "spells": runtime.spells.len(),
                    "abilities": runtime.abilities.len(),
                    "equipment": runtime.equipment.len(),
                    "variants": runtime.variants.len(),
                    "warnings": loaded.warnings.len(),
                });
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "pack      {} (format {})",
                    loaded.manifest.name, loaded.manifest.format
                );
                println!("source    {}", loaded.manifest.source.entry);
                println!("entities  {}", runtime.entities.len());
                println!("classes   {}", runtime.classes.len());
                println!("races     {}", runtime.races.len());
                println!("spells    {}", runtime.spells.len());
                println!("abilities {}", runtime.abilities.len());
                println!("equipment {}", runtime.equipment.len());
                println!("variants  {}", runtime.variants.len());
                println!("warnings  {}", loaded.warnings.len());
            }
        }
        Command::ImportReport {
            pcc,
            json,
            histogram,
            top,
        } => {
            let (pack, report) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            if histogram {
                let limit = if top == 0 { usize::MAX } else { top };
                let coverage = &report.coverage;
                println!(
                    "Semantic completion: {:.1}%  (lowered {} / (lowered {} + pending {}))",
                    coverage.semantic_completion() * 100.0,
                    coverage.lowered_effects,
                    coverage.lowered_effects,
                    coverage.semantic_pending(),
                );

                println!(
                    "\nPending legacy tags ({} distinct, {} occurrences):",
                    coverage.legacy_pending_by_tag.len(),
                    coverage.legacy_pending()
                );
                for (tag, count) in top_counts(&coverage.legacy_pending_by_tag, limit) {
                    println!("  {count:>6}  {tag}");
                }

                println!(
                    "\nPending unsupported tags ({} distinct, {} occurrences):",
                    coverage.unsupported_pending_by_tag.len(),
                    coverage.unsupported_pending()
                );
                for (tag, count) in top_counts(&coverage.unsupported_pending_by_tag, limit) {
                    println!("  {count:>6}  {tag}");
                }

                println!(
                    "\nAllowlisted tags ({} distinct, {} occurrences):",
                    coverage.legacy_allowlisted_by_tag.len()
                        + coverage.unsupported_allowlisted_by_tag.len(),
                    coverage.legacy_allowlisted() + coverage.unsupported_allowlisted()
                );
                for (tag, count) in top_counts(&coverage.legacy_allowlisted_by_tag, limit) {
                    println!("  {count:>6}  {tag}");
                }
                for (tag, count) in top_counts(&coverage.unsupported_allowlisted_by_tag, limit) {
                    println!("  {count:>6}  {tag}  (unsupported)");
                }
            } else if json {
                let value = serde_json::json!({
                    "provenance": {
                        "rules_fingerprint": pack.header.rules_fingerprint,
                        "data_ref": pack.header.data_ref,
                        "game_mode": pack.header.game_mode,
                    },
                    "coverage": report.coverage,
                    "patch_operations": report.patch_operations,
                    "diagnostics": report.diagnostics,
                    "error_diagnostics": report.error_diagnostics,
                    "races": pack.races.len(),
                    "classes": pack.classes.len(),
                    "skills": pack.skills.len(),
                    "spells": pack.spells.len(),
                    "equipment": pack.equipment.len(),
                    "equipment_modifiers": pack.equipment_modifiers.len(),
                    "deities": pack.deities.len(),
                    "domains": pack.domains.len(),
                    "variants": pack.variants.len(),
                    "kits": pack.kits.len(),
                    "templates": pack.templates.len(),
                    "abilities": pack.abilities.len(),
                    "ability_categories": pack.ability_categories.len(),
                    "creature_trait_records": pack.creature_trait_records.len(),
                    "companion_mods": pack
                        .companion_mods
                        .values()
                        .map(|rows| rows.len())
                        .sum::<usize>(),
                    "sizes": pack.sizes.len(),
                    "stats": pack.stats.len(),
                    "alignments": pack.alignments.len(),
                    "spell_effects": report.spell_effects,
                    "psionic_effects": report.psionic_effects,
                    "progressions": pack.progressions.len(),
                });
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("Rules fingerprint:   {}", pack.header.rules_fingerprint);
                if let Some(data_ref) = &pack.header.data_ref {
                    println!("Data ref:            {data_ref}");
                }
                println!("Entities:            {}", report.coverage.entities);
                println!("Lowered effects:     {}", report.coverage.lowered_effects);
                println!("Legacy tags:         {}", report.coverage.legacy_tags);
                println!("Unsupported tags:    {}", report.coverage.unsupported_tags);
                println!("Tag occurrences:     {}", report.coverage.tag_occurrences);
                println!(
                    "Known tag occurrences: {}",
                    report.coverage.known_tag_occurrences
                );
                println!("Pack entities:       {}", pack.entities.len());
                println!("Races:               {}", pack.races.len());
                println!("Classes:             {}", pack.classes.len());
                println!("Skills:              {}", pack.skills.len());
                println!("Spells:              {}", pack.spells.len());
                println!("Equipment:           {}", pack.equipment.len());
                println!("Equipment modifiers: {}", pack.equipment_modifiers.len());
                println!("Deities:             {}", pack.deities.len());
                println!("Domains:             {}", pack.domains.len());
                println!("Variants:            {}", pack.variants.len());
                println!("Progressions:        {}", pack.progressions.len());
                println!();
                for (category, coverage) in &report.coverage.by_category {
                    println!(
                        "  {category:<16} entities={:<6} tags={:<6} known={:<6} legacy={:<6}",
                        coverage.entities,
                        coverage.tag_occurrences,
                        coverage.known_tag_occurrences,
                        coverage.legacy_tags
                    );
                }
            }
        }
        Command::ListVariants {
            pcc,
            search,
            class_id,
            format,
        } => {
            let (pack, _) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            let needle = search.as_deref().map(str::to_lowercase);
            let matches: Vec<_> = pack
                .variants
                .values()
                .filter(|variant| {
                    if let Some(needle) = &needle {
                        if !variant.id.to_lowercase().contains(needle)
                            && !variant.name.to_lowercase().contains(needle)
                        {
                            return false;
                        }
                    }
                    if let Some(class_id) = &class_id {
                        if !variant
                            .applies_to
                            .as_deref()
                            .is_some_and(|applies| applies.eq_ignore_ascii_case(class_id))
                        {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            // Resolve each `REPLACES` target against the anchors the pack
            // actually exposes.  This is the check that catches a variant which
            // parses fine but silently removes nothing.
            //
            // Anchors come from two sources:
            //   * the lowered pack (class level-ability rows and progressions,
            //     e.g. `Fighter ~ Bonus Feats`);
            //   * the raw records *before* `.MOD` merging, because PCGen
            //     attaches extra replaceable sub-features that way — the Bard's
            //     Bardic Music sub-abilities carry `TYPE.BardInspire*` on an
            //     `ABILITY:` tag of a `...Bardic Music.MOD` record, and those
            //     tags do not survive into the pack.
            // Raw `ABILITY:` tags only matter as anchors when they carry a
            // `TYPE.` guard, which is how PCGen marks a feature replaceable.
            let raw_anchor_lines: Vec<String> = {
                let records = pcgen_import::load_pcc(&pcc)
                    .map(|loaded| pcgen_import::apply_record_patches(&loaded.records))
                    .unwrap_or_default();
                records
                    .iter()
                    .flat_map(|record| {
                        record
                            .tags
                            .iter()
                            .filter(|tag| {
                                tag.name.eq_ignore_ascii_case("ABILITY")
                                    || tag.name.eq_ignore_ascii_case("AUTO")
                            })
                            .map(|tag| tag.value.clone())
                    })
                    // Keep only guarded lines: an unguarded `ABILITY:` is not
                    // an anchor, and its trailing predicate fragments were
                    // leaking into the anchor list as noise.
                    .filter(|value| {
                        value.contains("CATEGORY=ACF,TYPE.") || value.contains("TYPE.Archetype")
                    })
                    .collect()
            };
            let mut features_cache: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            let mut resolve = |variant: &pcgen_core::VariantDefinition| {
                let features = variant
                    .applies_to
                    .as_deref()
                    .map(|class_id| {
                        features_cache
                            .entry(class_id.to_ascii_lowercase())
                            .or_insert_with(|| {
                                let mut lines = Vec::new();
                                for key in [class_id.to_string(), format!("CLASS:{class_id}")] {
                                    if let Some(class) = pack.classes.get(&key) {
                                        // The grants live here and nowhere else;
                                        // the progression used to carry a copy.
                                        lines.extend(
                                            class
                                                .level_abilities
                                                .values()
                                                .flatten()
                                                .map(|grant| grant.render()),
                                        );
                                    }
                                }
                                // Anchors that only exist as raw `ABILITY:` tags
                                // (Bardic Music sub-features and friends).
                                lines.extend(raw_anchor_lines.iter().cloned());
                                lines
                            })
                            .clone()
                    })
                    .unwrap_or_default();
                let anchors =
                    available_anchors(&features, variant.applies_to.as_deref().unwrap_or_default());
                let targets = variant
                    .replace_targets()
                    .into_iter()
                    .map(|target| {
                        let resolved = features
                            .iter()
                            .any(|line| target.matches_feature_line(line));
                        let label = match &target {
                            pcgen_core::ReplaceTarget::TypeTag(tag) => format!("TYPE={tag}"),
                            pcgen_core::ReplaceTarget::Name(name) => name.clone(),
                        };
                        // Distinguish "this dataset never opened that anchor"
                        // from "an anchor exists that is nearly this name".
                        // The 3.5e supplement and the RSRD genuinely disagree
                        // on some spellings (`WizardBonusFeats` vs
                        // `WizardBonusFeat`), and lumping both under
                        // "unresolved" hides which is which.
                        let status = if resolved { "resolved" } else { "missing" };
                        (label, status)
                    })
                    .collect::<Vec<_>>();
                (targets, anchors)
            };

            if format.eq_ignore_ascii_case("json") {
                let rows: Vec<serde_json::Value> = matches
                    .iter()
                    .map(|variant| {
                        let (targets, anchors) = resolve(variant);
                        serde_json::json!({
                            "id": variant.id,
                            "name": variant.name,
                            "applies_to": variant.applies_to,
                            "replaces": variant.replaces,
                            "grants": variant.grants,
                            "grant_entries": variant
                                .grant_entries()
                                .iter()
                                .map(|grant| serde_json::json!({
                                    "name": grant.name,
                                    "level": grant.level,
                                    "class_id": grant.class_id,
                                }))
                                .collect::<Vec<_>>(),
                            "class_skills": variant.class_skills,
                            "replace_targets": targets
                                .iter()
                                .map(|(label, status)| serde_json::json!({
                                    "target": label,
                                    "resolved": *status == "resolved",
                                }))
                                .collect::<Vec<_>>(),
                            "all_targets_resolved": targets
                                .iter()
                                .all(|(_, status)| *status == "resolved"),
                            // When something missed, show what the class does
                            // offer so the difference is inspectable.
                            "available_anchors": if targets
                                .iter()
                                .any(|(_, status)| *status != "resolved")
                            {
                                serde_json::Value::Array(
                                    anchors
                                        .iter()
                                        .cloned()
                                        .map(serde_json::Value::String)
                                        .collect(),
                                )
                            } else {
                                serde_json::Value::Null
                            },
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!("variants: {}", matches.len());
                for variant in matches {
                    let (targets, anchors) = resolve(variant);
                    let missing: Vec<&String> = targets
                        .iter()
                        .filter(|(_, status)| *status != "resolved")
                        .map(|(label, _)| label)
                        .collect();
                    let status = if missing.is_empty() {
                        "ok".to_string()
                    } else {
                        // Say what the class does offer, rather than guessing
                        // which anchor was meant: the supplement and the RSRD
                        // disagree on some spellings, and a bare "unresolved"
                        // hides whether that is the case.
                        format!(
                            "MISSING({}) [class offers: {}]",
                            missing
                                .iter()
                                .map(|label| label.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                            anchors.join(" ")
                        )
                    };
                    println!(
                        "{}\t{}\tapplies_to={}\treplaces={}\tgrants={}\t{}",
                        variant.id,
                        variant.name,
                        variant.applies_to.as_deref().unwrap_or("-"),
                        variant.replaces.len(),
                        // Show each grant with the level it is gained at, so an
                        // ungated grant (level 1) is visible at a glance.
                        variant
                            .grant_entries()
                            .iter()
                            .map(|grant| match grant.level {
                                Some(level) => format!("{}@{}", grant.name, level),
                                None => format!("{}@1", grant.name),
                            })
                            .collect::<Vec<_>>()
                            .join(","),
                        status
                    );
                }
            }
        }
        Command::ListTemplates {
            pcc,
            search,
            type_tags,
            with_details,
            format,
        } => {
            let (pack, _) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            let needle = search.as_deref().map(str::to_lowercase);
            let type_filters: Vec<String> =
                type_tags.iter().map(|tag| tag.to_lowercase()).collect();
            let matches: Vec<_> = pack
                .templates
                .values()
                .filter(|template| {
                    if let Some(needle) = &needle {
                        if !template.id.to_lowercase().contains(needle)
                            && !template.name.to_lowercase().contains(needle)
                        {
                            return false;
                        }
                    }
                    if !type_filters.is_empty()
                        && !template.type_tags.iter().any(|tag| {
                            type_filters
                                .iter()
                                .any(|filter| tag.eq_ignore_ascii_case(filter))
                        })
                    {
                        return false;
                    }
                    true
                })
                .collect();
            if format.eq_ignore_ascii_case("json") {
                let rows: Vec<serde_json::Value> = matches
                    .iter()
                    .map(|template| {
                        serde_json::json!({
                            "id": template.id,
                            "name": template.name,
                            "type_tags": template.type_tags,
                            "hit_die_override": template.hit_die_override,
                            "add_levels": template.add_levels,
                            "challenge_rating": template.traits.challenge_rating,
                            "damage_reduction": template.traits.damage_reduction,
                            "spell_resistance": template.traits.spell_resistance,
                            "stat_minimums": template.stat_minimums,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!("templates: {}", matches.len());
                for template in matches {
                    if !with_details {
                        println!("{}\t{}", template.id, template.name);
                        continue;
                    }
                    let mut details = Vec::new();
                    if let Some(hit_die) = &template.hit_die_override {
                        details.push(format!("hitdie={hit_die}"));
                    }
                    if let Some(cr) = &template.traits.challenge_rating {
                        details.push(format!("cr={cr}"));
                    }
                    if !template.add_levels.is_empty() {
                        let levels = template
                            .add_levels
                            .iter()
                            .map(|(class, count)| format!("{class}+{count}"))
                            .collect::<Vec<_>>()
                            .join(",");
                        details.push(format!("addlevel={levels}"));
                    }
                    if !template.type_tags.is_empty() {
                        details.push(format!("tags={}", template.type_tags.join(",")));
                    }
                    if details.is_empty() {
                        println!("{}\t{}", template.id, template.name);
                    } else {
                        println!("{}\t{}\t{}", template.id, template.name, details.join(" "));
                    }
                }
            }
        }
        Command::ParsePcg { pcg, format } => {
            let draft = parse_pcg_draft_file(&pcg)
                .with_context(|| format!("failed to read {}", pcg.display()))?;
            if format.eq_ignore_ascii_case("text") {
                println!("{:#?}", draft);
            } else {
                println!("{}", serde_json::to_string_pretty(&draft)?);
            }
        }
        Command::Recalculate {
            pcc,
            pcg,
            summary,
            format,
        } => {
            let draft = parse_pcg_draft_file(&pcg)
                .with_context(|| format!("failed to read {}", pcg.display()))?;
            let (pack, _) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            let snapshot = Engine::new(pack).evaluate(&draft_from_pcg(draft));
            let output_format = OutputFormat::from_name(&format).unwrap_or(OutputFormat::Json);
            if summary && output_format == OutputFormat::Text {
                print!("{}", render_text(&snapshot));
            } else {
                print!("{}", render(&snapshot, output_format)?);
            }
        }
        Command::Export { pcg, pcc, format } => {
            let draft = parse_pcg_draft_file(&pcg)
                .with_context(|| format!("failed to read {}", pcg.display()))?;
            let snapshot = if let Some(pcc) = pcc {
                let (pack, _) = compile_pcc(&pcc)
                    .with_context(|| format!("failed to import {}", pcc.display()))?;
                Engine::new(pack).evaluate(&draft_from_pcg(draft))
            } else {
                demo_engine().evaluate(&draft_from_pcg(draft))
            };
            let output_format = OutputFormat::from_name(&format).unwrap_or(OutputFormat::Text);
            print!("{}", render(&snapshot, output_format)?);
        }
        Command::EvaluatePcg { pcg, format } => {
            let draft = parse_pcg_draft_file(&pcg)
                .with_context(|| format!("failed to read {}", pcg.display()))?;
            let snapshot = demo_engine().evaluate(&draft_from_pcg(draft));
            if format.eq_ignore_ascii_case("json") {
                println!("{}", render_json(&snapshot)?);
            } else {
                print!("{}", render_text(&snapshot));
            }
        }
        Command::Evaluate { draft, format } => {
            let text = std::fs::read_to_string(&draft)
                .with_context(|| format!("failed to read {}", draft.display()))?;
            let draft: CharacterDraft = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse {}", draft.display()))?;
            let engine = demo_engine();
            let snapshot = engine.evaluate(&draft);
            if format.eq_ignore_ascii_case("json") {
                println!("{}", render_json(&snapshot)?);
            } else {
                print!("{}", render_text(&snapshot));
            }
        }
        Command::Create {
            pcc,
            name,
            race,
            class_id,
            level,
            alignment,
            templates,
            stats,
            point_buy,
            skills,
            abilities,
            spells,
            equipment,
            equipment_budget,
            starting_gold,
            deity,
            domains,
            format,
            out,
        } => {
            let (pack, _) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            let class_gold = starting_gold.then(|| starting_gold_for_class(&class_id));
            let request = CreateRequest {
                id: String::new(),
                name,
                race,
                class_id,
                level,
                alignment,
                campaign: pcc.display().to_string(),
                stats: parse_pairs(&stats)?,
                point_buy_budget: point_buy,
                skills: parse_skill_pairs(&skills)?,
                abilities: parse_key_value_pairs(&abilities)?,
                spells: parse_spells(&spells),
                equipment,
                equipment_budget: equipment_budget.or(class_gold),
                advancement: Vec::new(),
                templates,
                deity,
                domains,
            };
            let draft = create_draft(&pack, &request)?;
            if let Some(path) = out {
                let envelope = NativeSaveEnvelope::new(
                    draft,
                    pack.header.rules_fingerprint.clone(),
                    pack.header.data_ref.clone(),
                )
                .with_pcc(Some(pcc.display().to_string()));
                save_envelope(&path, &envelope)?;
                println!("{}", path.display());
            } else {
                let snapshot = Engine::new(pack).evaluate(&draft);
                let output_format = OutputFormat::from_name(&format).unwrap_or(OutputFormat::Json);
                print!("{}", render(&snapshot, output_format)?);
            }
        }
        Command::LevelUp {
            draft,
            pcc,
            class_id,
            hp,
            skills,
            abilities,
            spells,
            templates,
            replace_templates,
            archetype,
            substitution_level,
            subclass,
            lanes,
            lane_hp,
            lane_archetype,
            lane_substitution,
            lane_subclass,
            preview,
            out,
        } => {
            let text = std::fs::read_to_string(&draft)
                .with_context(|| format!("failed to read {}", draft.display()))?;
            let mut envelope: NativeSaveEnvelope = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse {}", draft.display()))?;
            let (pack, _) =
                compile_pcc(&pcc).with_context(|| format!("failed to import {}", pcc.display()))?;
            let request = LevelUpRequest {
                class_id,
                hp_gained: hp,
                skills: parse_skill_pairs(&skills)?,
                abilities: parse_key_value_pairs(&abilities)?,
                spells: parse_spells(&spells),
                templates,
                replace_templates,
                archetype,
                substitution_level,
                subclass,
                lanes: build_lanes(
                    &lanes,
                    &lane_hp,
                    &lane_archetype,
                    &lane_substitution,
                    &lane_subclass,
                ),
            };
            if preview {
                let engine = Engine::new(pack.clone());
                let before = engine.evaluate(&envelope.character);
                let preview_draft = preview_level_up(&pack, &envelope.character, &request, None)?;
                let after = engine.evaluate(&preview_draft);
                println!("{}", render_json(&after)?);
                eprintln!(
                    "delta: level {:+}  hp {:+}  bab {:+}  ac {:+}",
                    after.total_level as i64 - before.total_level as i64,
                    after.hit_points - before.hit_points,
                    after.base_attack_bonus - before.base_attack_bonus,
                    after.armor_class - before.armor_class
                );
                return Ok(());
            }
            level_up_draft(&pack, &mut envelope.character, &request)?;
            envelope.pcc = Some(pcc.display().to_string());
            envelope.rules_fingerprint = pack.header.rules_fingerprint.clone();
            envelope.data_ref = pack.header.data_ref.clone();
            if let Some(path) = out {
                save_envelope(&path, &envelope)?;
                println!("{}", path.display());
            } else {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            }
        }
        Command::Undo { draft, dry_run } => {
            let text = std::fs::read_to_string(&draft)
                .with_context(|| format!("failed to read {}", draft.display()))?;
            let envelope: NativeSaveEnvelope = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse {}", draft.display()))?;
            let repository = CharacterRepository::new(
                draft.parent().unwrap_or_else(|| std::path::Path::new(".")),
            )
            .with_live_path(&draft);
            let id = envelope.character.id.clone();
            if !repository.can_undo(&id)? {
                bail!("no previous version for character: {id}");
            }
            let restored = repository.undo(&id)?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&restored)?);
            } else {
                let remaining = repository.history_depth(&id).unwrap_or(0);
                println!(
                    "restored {} to level {} ({} more step(s) available)",
                    restored.character.name,
                    restored.character.total_level(),
                    remaining
                );
            }
        }
        Command::Redo { draft, dry_run } => {
            let text = std::fs::read_to_string(&draft)
                .with_context(|| format!("failed to read {}", draft.display()))?;
            let envelope: NativeSaveEnvelope = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse {}", draft.display()))?;
            let repository = CharacterRepository::new(
                draft.parent().unwrap_or_else(|| std::path::Path::new(".")),
            )
            .with_live_path(&draft);
            let id = envelope.character.id.clone();
            if !repository.can_redo(&id)? {
                bail!("nothing to redo for character: {id}");
            }
            let reapplied = repository.redo(&id)?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&reapplied)?);
            } else {
                let remaining = repository.redo_depth(&id).unwrap_or(0);
                println!(
                    "re-applied {} at level {} ({} more redo step(s) available)",
                    reapplied.character.name,
                    reapplied.character.total_level(),
                    remaining
                );
            }
        }
        Command::History { draft, format } => {
            let text = std::fs::read_to_string(&draft)
                .with_context(|| format!("failed to read {}", draft.display()))?;
            let envelope: NativeSaveEnvelope = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse {}", draft.display()))?;
            let repository = CharacterRepository::new(
                draft.parent().unwrap_or_else(|| std::path::Path::new(".")),
            )
            .with_live_path(&draft);
            let id = envelope.character.id.clone();
            let versions = repository.history(&id)?;
            if format.eq_ignore_ascii_case("json") {
                println!("{}", serde_json::to_string_pretty(&versions)?);
            } else {
                println!(
                    "{}: {} revision(s) (undo {}, redo {})",
                    id,
                    versions.len(),
                    repository.history_depth(&id)?,
                    repository.redo_depth(&id)?
                );
                for version in &versions {
                    let summary = &version.summary;
                    let mut delta = Vec::new();
                    if summary.level_delta != 0 {
                        delta.push(format!("level {:+}", summary.level_delta));
                    }
                    if !summary.classes_removed.is_empty() {
                        delta.push(format!("-class {}", summary.classes_removed.join(",")));
                    }
                    if !summary.classes_added.is_empty() {
                        delta.push(format!("+class {}", summary.classes_added.join(",")));
                    }
                    if !summary.abilities_added.is_empty() {
                        delta.push(format!("+ability {}", summary.abilities_added.join("; ")));
                    }
                    if !summary.abilities_removed.is_empty() {
                        delta.push(format!("-ability {}", summary.abilities_removed.join("; ")));
                    }
                    let delta = if delta.is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", delta.join(" "))
                    };
                    println!(
                        "  #{:<3} L{:<3} {:<5} {}{}",
                        version.index, version.total_level, version.stack, version.name, delta
                    );
                }
            }
        }
        Command::ExportAll { dir, out } => {
            let repository = CharacterRepository::new(&dir);
            let summaries = repository.list()?;
            let mut characters = Vec::new();
            for summary in &summaries {
                characters.push(repository.load(&summary.id)?);
            }
            let bundle = serde_json::json!({
                "schema": pcgen_builder::SAVE_SCHEMA,
                "application_version": pcgen_builder::APPLICATION_VERSION,
                "characters": characters,
            });
            let text = serde_json::to_string_pretty(&bundle)?;
            match out {
                Some(path) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("failed to create {}", parent.display()))?;
                    }
                    std::fs::write(&path, text)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    println!(
                        "exported {} character(s) to {}",
                        characters.len(),
                        path.display()
                    );
                }
                None => println!("{text}"),
            }
        }
        Command::ImportBundle {
            bundle,
            dir,
            pcc,
            list_groups,
        } => {
            let text = std::fs::read_to_string(&bundle)
                .with_context(|| format!("failed to read {}", bundle.display()))?;
            let value: serde_json::Value = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse {}", bundle.display()))?;
            let characters = value
                .get("characters")
                .and_then(|value| value.as_array())
                .with_context(|| "bundle has no `characters` array".to_string())?;
            let repository = CharacterRepository::new(&dir);
            let mut imported = 0usize;
            let mut envelopes = Vec::new();
            for character in characters {
                let envelope: NativeSaveEnvelope = serde_json::from_value(character.clone())
                    .with_context(|| {
                        format!("failed to parse a character in {}", bundle.display())
                    })?;
                envelopes.push(envelope);
            }
            if list_groups {
                for (fingerprint, ids) in pcgen_builder::group_by_fingerprint(&envelopes) {
                    println!("{fingerprint}: {}", ids.join(", "));
                }
                return Ok(());
            }
            if let Some(pcc) = &pcc {
                let (pack, _) = compile_pcc(pcc)
                    .with_context(|| format!("failed to import {}", pcc.display()))?;
                let before = envelopes.len();
                envelopes =
                    pcgen_builder::filter_by_fingerprint(envelopes, &pack.header.rules_fingerprint);
                println!(
                    "fingerprint filter kept {} of {} character(s)",
                    envelopes.len(),
                    before
                );
            }
            let report = pcgen_builder::validate_bundle(&envelopes);
            for warning in report.warnings() {
                eprintln!("warning: {warning}");
            }
            for envelope in &envelopes {
                repository.save(envelope)?;
                imported += 1;
            }
            println!("imported {imported} character(s) into {}", dir.display());
        }
        Command::Delete {
            dir,
            ids,
            all,
            dry_run,
        } => {
            let repository = CharacterRepository::new(&dir);
            let targets: Vec<String> = if all {
                repository
                    .list()?
                    .into_iter()
                    .map(|summary| summary.id)
                    .collect()
            } else {
                ids
            };
            if targets.is_empty() {
                bail!("nothing to delete: pass --id <id> or --all");
            }
            let mut deleted = 0usize;
            for id in &targets {
                if dry_run {
                    println!("would delete {id}");
                    continue;
                }
                match repository.delete(id) {
                    Ok(()) => {
                        deleted += 1;
                        println!("deleted {id}");
                    }
                    Err(pcgen_builder::BuilderError::NotFound(_)) => {
                        eprintln!("warning: no such character: {id}");
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            if dry_run {
                println!("{} character(s) would be deleted", targets.len());
            } else {
                println!("deleted {deleted} character(s)");
            }
        }
    }
    Ok(())
}

fn top_counts(
    histogram: &std::collections::BTreeMap<String, usize>,
    limit: usize,
) -> Vec<(&str, usize)> {
    let mut entries: Vec<(&str, usize)> = histogram
        .iter()
        .map(|(tag, count)| (tag.as_str(), *count))
        .collect();
    // Deterministic: sort by count desc, then tag name asc.
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    entries.truncate(limit);
    entries
}
