//! Minimal local Web API for PCGen-Rust.
//!
//! This crate intentionally only depends on the compatibility layer
//! (`pcgen-import`) at the application boundary: it compiles a `.pcc` and
//! evaluates a `.pcg` through the public engine API, exactly like the CLI.

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use pcgen_engine::{CharacterDraft, DraftClassLevel, DraftSpell, Engine};
use pcgen_import::{compile_pcc, parse_pcg_draft_file, PcgDraft};
use pcgen_output::{render_html, render_json};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct WebConfig {
    /// Root directory used to resolve relative `.pcc` / `.pcg` paths.
    pub data_root: PathBuf,
    /// Root directory holding `tests/fixtures/differential/broad_cohort.json`.
    /// Defaults to the workspace root so the curated list works while
    /// `data_root` points at the read-only PCGen snapshot.
    pub cohort_root: PathBuf,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            data_root: PathBuf::from("."),
            cohort_root: PathBuf::from("."),
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<WebConfig>,
}

#[derive(Debug, Deserialize)]
pub struct RecalculateRequest {
    pub pcc: String,
    pub pcg: String,
    /// Optional draft overrides applied after `.pcg` parsing.
    #[serde(default)]
    pub overrides: DraftOverrides,
}

/// Draft fields the Web UI may override after `.pcg` parsing.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DraftOverrides {
    #[serde(default)]
    pub choices: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub abilities: Vec<(String, String)>,
    #[serde(default)]
    pub active_states: Vec<String>,
    #[serde(default)]
    pub class_subclasses: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub deity: Option<String>,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CuratedCase {
    pub label: String,
    pub pcc: String,
    pub pcg: String,
}

#[derive(Debug, Serialize)]
pub struct CuratedResponse {
    pub cases: Vec<CuratedCase>,
    /// Ability ids flagged `MULT` / `STACK`, offered by the pool editor.
    pub repeatable_abilities: Vec<String>,
    /// Subclass choices captured from the campaign, keyed by class id.
    pub class_choices: std::collections::BTreeMap<String, String>,
    /// Ability-category membership and pool metadata, for the pool editor.
    pub ability_candidates: std::collections::BTreeMap<String, AbilityCandidates>,
}

#[derive(Debug, Serialize)]
pub struct AbilityCandidates {
    pub pool: Option<String>,
    pub category: Option<String>,
    pub members: Vec<String>,
    pub repeatable: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UnresolvedChoiceHint {
    pub spec: String,
    pub kind: String,
    pub options: Vec<String>,
    pub hint: String,
}

#[derive(Debug, Serialize)]
pub struct RecalculateResponse {
    pub snapshot: serde_json::Value,
    pub choices: serde_json::Value,
    pub unresolved_choices: Vec<String>,
    pub unresolved_hints: Vec<UnresolvedChoiceHint>,
    pub ability_pool_usage: serde_json::Value,
    pub diagnostics: Vec<String>,
    pub html: String,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub pcc: String,
    pub pcg: String,
    #[serde(default)]
    pub overrides: DraftOverrides,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    /// Lossless `.pcg` text produced from the draft after overrides.
    pub pcg: String,
    /// Unresolved choices still present after evaluation.
    pub unresolved_choices: Vec<String>,
    pub unresolved_hints: Vec<UnresolvedChoiceHint>,
    /// Ability pool usage, so the UI can highlight remaining budget.
    pub ability_pool_usage: serde_json::Value,
    /// Diagnostics produced by the evaluation.
    pub diagnostics: Vec<String>,
}

/// Build the axum router for the Web API.
pub fn app(config: WebConfig) -> Router {
    let state = AppState {
        config: Arc::new(config),
    };
    Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/curated", get(curated))
        .route("/api/recalculate", post(recalculate))
        .route("/api/export", post(export))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// Curated catalogue assembled from the checked-in cohort or built-in defaults.
struct CuratedCatalog {
    cases: Vec<CuratedCase>,
    repeatable_abilities: Vec<String>,
    class_choices: std::collections::BTreeMap<String, String>,
    ability_candidates: std::collections::BTreeMap<String, AbilityCandidates>,
}

/// Curated case list.  When the data root contains the PCGen test suite the
/// standard cohort is offered; otherwise a small built-in list is used.
async fn curated(State(state): State<AppState>) -> Json<CuratedResponse> {
    let catalog = curated_cases(&state.config.data_root, &state.config.cohort_root);
    Json(CuratedResponse {
        cases: catalog.cases,
        repeatable_abilities: catalog.repeatable_abilities,
        class_choices: catalog.class_choices,
        ability_candidates: catalog.ability_candidates,
    })
}

fn curated_cases(data_root: &Path, cohort_root: &Path) -> CuratedCatalog {
    let cohort = cohort_root
        .join("tests")
        .join("fixtures")
        .join("differential")
        .join("broad_cohort.json");
    if let Ok(text) = std::fs::read_to_string(&cohort) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let mut cases = Vec::new();
            if let Some(map) = value.get("cases").and_then(|cases| cases.as_object()) {
                for (label, case) in map {
                    let pcc = case.get("pcc").and_then(|value| value.as_str());
                    let pcg = case.get("pcg").and_then(|value| value.as_str());
                    if let (Some(pcc), Some(pcg)) = (pcc, pcg) {
                        cases.push(CuratedCase {
                            label: label.clone(),
                            pcc: relativize(data_root, pcc),
                            pcg: relativize(data_root, pcg),
                        });
                    }
                }
            }
            cases.sort_by(|left, right| left.label.cmp(&right.label));
            if !cases.is_empty() {
                let (repeatable, candidates) = campaign_candidates(data_root, &cases);
                return CuratedCatalog {
                    cases,
                    repeatable_abilities: repeatable,
                    class_choices: std::collections::BTreeMap::new(),
                    ability_candidates: candidates,
                };
            }
        }
    }
    CuratedCatalog {
        cases: vec![
            CuratedCase {
                label: "35e Bard".to_string(),
                pcc: "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc".to_string(),
                pcg: "code/testsuite/PCGfiles/35e_Bard.pcg".to_string(),
            },
            CuratedCase {
                label: "3e FigFae".to_string(),
                pcc: "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc".to_string(),
                pcg: "code/testsuite/PCGfiles/3e_FigFae.pcg".to_string(),
            },
        ],
        repeatable_abilities: Vec::new(),
        class_choices: std::collections::BTreeMap::new(),
        ability_candidates: std::collections::BTreeMap::new(),
    }
}

/// Best-effort ability-category candidates for the first curated campaign.
fn campaign_candidates(
    data_root: &Path,
    cases: &[CuratedCase],
) -> (
    Vec<String>,
    std::collections::BTreeMap<String, AbilityCandidates>,
) {
    let mut candidates = std::collections::BTreeMap::new();
    let Some(case) = cases.first() else {
        return (Vec::new(), candidates);
    };
    let Ok((pack, _)) = compile_pcc(resolve_path(data_root, &case.pcc)) else {
        return (Vec::new(), candidates);
    };
    let mut repeatable_set = std::collections::BTreeSet::new();
    for (id, ability) in &pack.abilities {
        if ability.stack == Some(true) || ability.multi == Some(true) {
            repeatable_set.insert(id.clone());
        }
    }
    let mut repeatable: Vec<String> = repeatable_set.iter().cloned().collect();
    repeatable.sort();
    for (id, category) in &pack.ability_categories {
        let members: Vec<String> = if category.ability_list.is_empty() {
            pack.abilities
                .values()
                .filter(|ability| {
                    ability.category.as_deref().is_some_and(|ability_category| {
                        category.category.as_deref().is_some_and(|pool_category| {
                            ability_category.eq_ignore_ascii_case(pool_category)
                        })
                    })
                })
                .map(|ability| ability.name.clone())
                .collect()
        } else {
            category.ability_list.clone()
        };
        let mut members = members;
        members.sort();
        members.dedup();
        members.truncate(60);
        let repeatable_members: Vec<String> = members
            .iter()
            .filter(|name| {
                let key = name.trim().to_ascii_lowercase().replace(' ', "_");
                repeatable_set.contains(&key)
            })
            .cloned()
            .collect();
        candidates.insert(
            id.clone(),
            AbilityCandidates {
                pool: category.pool.clone(),
                category: category.category.clone(),
                members,
                repeatable: repeatable_members,
            },
        );
    }
    (repeatable, candidates)
}

fn relativize(root: &Path, path: &str) -> String {
    let path = Path::new(path);
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

async fn recalculate(
    State(state): State<AppState>,
    Json(request): Json<RecalculateRequest>,
) -> impl IntoResponse {
    match run_recalculate(&state.config, &request) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn export(
    State(state): State<AppState>,
    Json(request): Json<ExportRequest>,
) -> impl IntoResponse {
    match run_export(&state.config, &request) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

fn run_export(config: &WebConfig, request: &ExportRequest) -> anyhow::Result<ExportResponse> {
    let pcc = resolve_path(&config.data_root, &request.pcc);
    let pcg = resolve_path(&config.data_root, &request.pcg);
    let (pack, _report) = compile_pcc(&pcc)?;
    let mut draft = draft_from_pcg(parse_pcg_draft_file(&pcg)?);
    apply_overrides(&mut draft, &request.overrides);
    let snapshot = Engine::new(pack.clone()).evaluate(&draft);
    let exported = pcg_from_draft(&draft);
    let unresolved_choices = snapshot.unresolved_choices.clone();
    let hints = unresolved_hints(&draft, &unresolved_choices, &pack);
    Ok(ExportResponse {
        pcg: pcgen_import::write_pcg_draft_string(&exported)?,
        unresolved_choices,
        unresolved_hints: hints,
        ability_pool_usage: serde_json::to_value(&snapshot.ability_pool_usage)?,
        diagnostics: snapshot.diagnostics.clone(),
    })
}

/// Convert an engine draft back into the `.pcg` compatibility type.
pub fn pcg_from_draft(draft: &CharacterDraft) -> PcgDraft {
    PcgDraft {
        name: draft.name.clone(),
        race_id: draft.race_id.clone(),
        campaign: draft.campaign.clone(),
        alignment: draft.alignment.clone(),
        stats: draft.stats.clone(),
        class_levels: draft
            .class_levels
            .iter()
            .map(|entry| pcgen_import::PcgClassLevel {
                class_id: entry.class_id.clone(),
                level: entry.level,
                hit_points_gained: entry.hit_points_gained,
                hit_points_by_level: entry.hit_points_by_level.clone(),
                experience_gained: entry.experience_gained,
            })
            .collect(),
        skills: draft.skills.clone(),
        spells: draft
            .spells
            .iter()
            .map(|spell| pcgen_import::PcgSpell {
                name: spell.name.clone(),
                level: spell.level,
                class_id: spell.class_id.clone(),
                times_memorized: spell.times_memorized,
                book: spell.book.clone(),
            })
            .collect(),
        equipment: draft.equipment.clone(),
        equipment_modifiers: draft.equipment_modifiers.clone(),
        equipment_base_items: draft.equipment_base_items.clone(),
        equipment_display_names: draft.equipment_display_names.clone(),
        abilities: draft.abilities.clone(),
        templates: draft.templates.clone(),
        advancement: draft.advancement.clone(),
        choices: draft.choices.clone(),
        class_subclasses: draft.class_subclasses.clone(),
        deity: draft.deity.clone(),
        domains: draft.domains.clone(),
        deity_domains: draft.deity_domains.clone(),
    }
}

fn run_recalculate(
    config: &WebConfig,
    request: &RecalculateRequest,
) -> anyhow::Result<RecalculateResponse> {
    let pcc = resolve_path(&config.data_root, &request.pcc);
    let pcg = resolve_path(&config.data_root, &request.pcg);
    let (pack, _report) = compile_pcc(&pcc)?;
    let mut draft = draft_from_pcg(parse_pcg_draft_file(&pcg)?);
    apply_overrides(&mut draft, &request.overrides);
    let snapshot = Engine::new(pack.clone()).evaluate(&draft);
    let snapshot_json: serde_json::Value = serde_json::from_str(&render_json(&snapshot)?)?;
    let choices = serde_json::to_value(&snapshot.choices)?;
    let ability_pool_usage = serde_json::to_value(&snapshot.ability_pool_usage)?;
    let unresolved_choices = snapshot.unresolved_choices.clone();
    let hints = unresolved_hints(&draft, &unresolved_choices, &pack);
    Ok(RecalculateResponse {
        snapshot: snapshot_json,
        choices,
        unresolved_choices,
        unresolved_hints: hints,
        ability_pool_usage,
        diagnostics: snapshot.diagnostics.clone(),
        html: render_html(&snapshot),
    })
}

/// Explain each unresolved choice with its kind, candidates and a hint.
fn unresolved_hints(
    draft: &CharacterDraft,
    unresolved: &[String],
    pack: &pcgen_import::CompiledRulePack,
) -> Vec<UnresolvedChoiceHint> {
    unresolved
        .iter()
        .map(|spec| {
            let kind = spec
                .split('|')
                .next()
                .unwrap_or(spec)
                .trim()
                .to_ascii_uppercase();
            let options: Vec<String> = match kind.as_str() {
                "SKILL" => draft
                    .skills
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .chain(pack.skills.values().map(|skill| skill.name.clone()))
                    .collect(),
                "CLASS" => pack
                    .classes
                    .values()
                    .map(|class| class.name.clone())
                    .collect(),
                "SCHOOL" => pack
                    .spells
                    .values()
                    .map(|spell| spell.school.clone())
                    .filter(|school| !school.is_empty())
                    .collect(),
                "DOMAIN" => pack
                    .domains
                    .values()
                    .map(|domain| domain.name.clone())
                    .collect(),
                "SPELLS" | "SPELL" => pack
                    .spells
                    .values()
                    .map(|spell| spell.name.clone())
                    .collect(),
                "ABILITY" | "FEAT" => pack
                    .abilities
                    .values()
                    .map(|ability| ability.name.clone())
                    .collect(),
                _ => Vec::new(),
            };
            let mut options = options;
            options.sort();
            options.dedup();
            let hint = if options.is_empty() {
                format!("provide draft.choices[\"{spec}\"]; no built-in candidate list for {kind}")
            } else {
                format!(
                    "provide draft.choices[\"{spec}\"]; {} candidate(s) available",
                    options.len()
                )
            };
            UnresolvedChoiceHint {
                spec: spec.clone(),
                kind,
                options: options.into_iter().take(200).collect(),
                hint,
            }
        })
        .collect()
}

fn resolve_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Apply Web UI overrides on top of a parsed `.pcg` draft.
pub fn apply_overrides(draft: &mut CharacterDraft, overrides: &DraftOverrides) {
    for (key, value) in &overrides.choices {
        draft.choices.insert(key.clone(), value.clone());
    }
    for (category, key) in &overrides.abilities {
        if !draft
            .abilities
            .iter()
            .any(|(existing_category, existing_key)| {
                existing_category.eq_ignore_ascii_case(category)
                    && existing_key.eq_ignore_ascii_case(key)
            })
        {
            draft.abilities.push((category.clone(), key.clone()));
        }
    }
    for state in &overrides.active_states {
        if !draft
            .active_states
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(state))
        {
            draft.active_states.push(state.clone());
        }
    }
    for (class_id, subclass) in &overrides.class_subclasses {
        draft
            .class_subclasses
            .insert(class_id.clone(), subclass.clone());
    }
    if let Some(deity) = &overrides.deity {
        if !deity.trim().is_empty() {
            draft.deity = Some(deity.clone());
        }
    }
    for domain in &overrides.domains {
        if !draft
            .domains
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(domain))
        {
            draft.domains.push(domain.clone());
        }
    }
}

/// Convert a parsed `.pcg` draft into the engine draft.
///
/// Mirrors the CLI conversion so the Web API and CLI stay consistent.
pub fn draft_from_pcg(draft: PcgDraft) -> CharacterDraft {
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
