use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use pcgen_builder::{
    create_draft, level_up_draft, starting_gold_for_class, CreateRequest, CreateSpell,
    LevelUpRequest, NativeSaveEnvelope,
};
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
        /// Write the updated envelope here; otherwise print JSON to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
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
            features_by_level: BTreeMap::new(),
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
        equipment_modifiers: BTreeMap::new(),
        equipment_base_items: BTreeMap::new(),
        equipment_display_names: BTreeMap::new(),
        abilities: Vec::new(),
        templates: Vec::new(),
        active_states: Vec::new(),
        choices: BTreeMap::new(),
        companions: Vec::new(),
        class_subclasses: BTreeMap::new(),
        deity: None,
        domains: Vec::new(),
        deity_domains: Vec::new(),
    }
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
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                std::fs::write(&path, serde_json::to_vec_pretty(&envelope)?)
                    .with_context(|| format!("failed to write {}", path.display()))?;
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
            };
            level_up_draft(&pack, &mut envelope.character, &request)?;
            envelope.pcc = Some(pcc.display().to_string());
            envelope.rules_fingerprint = pack.header.rules_fingerprint.clone();
            envelope.data_ref = pack.header.data_ref.clone();
            if let Some(path) = out {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                std::fs::write(&path, serde_json::to_vec_pretty(&envelope)?)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                println!("{}", path.display());
            } else {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
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
                book: spell.book,
            })
            .collect(),
        equipment: draft.equipment,
        equipment_modifiers: draft.equipment_modifiers,
        equipment_base_items: draft.equipment_base_items,
        equipment_display_names: draft.equipment_display_names,
        abilities: draft.abilities,
        templates: draft.templates,
        active_states: Vec::new(),
        choices: draft.choices,
        companions: Vec::new(),
        class_subclasses: draft.class_subclasses,
        deity: draft.deity,
        domains: draft.domains,
        deity_domains: draft.deity_domains,
    }
}
