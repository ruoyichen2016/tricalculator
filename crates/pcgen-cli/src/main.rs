use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, ClassProgress, DraftClassLevel, DraftSpell, Engine};
use pcgen_import::PcgDraft;
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

#[derive(Debug, Subcommand)]
enum Command {
    /// Render a built-in Gestalt demo snapshot.
    Demo,
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
            features_by_level: BTreeMap::new(),
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
            features_by_level: BTreeMap::new(),
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
        stats: BTreeMap::from([
            ("STR".to_string(), 14),
            ("DEX".to_string(), 12),
            ("CON".to_string(), 14),
            ("INT".to_string(), 14),
            ("WIS".to_string(), 10),
            ("CHA".to_string(), 10),
        ]),
        class_levels: Vec::new(),
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
        abilities: Vec::new(),
        templates: Vec::new(),
        choices: BTreeMap::new(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Demo => {
            let engine = demo_engine();
            let snapshot = engine.evaluate(&demo_draft());
            print!("{}", render_text(&snapshot));
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

/// Convert a parsed PCGen `.pcg` draft into the engine's `CharacterDraft`.
///
/// This used to live in `pcgen-engine`; it moved to the CLI adapter so the
/// engine no longer depends on `pcgen-import`.  PCGen `PcgDraft` is a
/// compatibility type, so the conversion is an application-boundary concern.
fn draft_from_pcg(draft: PcgDraft) -> CharacterDraft {
    CharacterDraft {
        id: format!("pcg:{}", draft.name),
        name: draft.name,
        race_id: draft.race_id,
        campaign: draft.campaign,
        alignment: draft.alignment,
        stats: draft.stats,
        class_levels: draft
            .class_levels
            .into_iter()
            .map(|entry| DraftClassLevel {
                class_id: entry.class_id,
                level: entry.level,
                hit_points_gained: entry.hit_points_gained,
                hit_points_by_level: entry.hit_points_by_level,
                experience_gained: entry.experience_gained,
            })
            .collect(),
        advancement: draft.advancement,
        skills: draft.skills,
        spells: draft
            .spells
            .into_iter()
            .map(|spell| DraftSpell {
                name: spell.name,
                level: spell.level,
                class_id: spell.class_id,
                times_memorized: spell.times_memorized,
            })
            .collect(),
        equipment: draft.equipment,
        abilities: draft.abilities,
        templates: draft.templates,
        choices: BTreeMap::new(),
    }
}
