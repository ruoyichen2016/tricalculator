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
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            data_root: PathBuf::from("."),
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
    /// Optional list of curated PCGs the UI can pass through unchanged.
    #[serde(default)]
    pub curated: bool,
}

#[derive(Debug, Serialize)]
pub struct RecalculateResponse {
    pub snapshot: serde_json::Value,
    pub choices: serde_json::Value,
    pub unresolved_choices: Vec<String>,
    pub ability_pool_usage: serde_json::Value,
    pub diagnostics: Vec<String>,
    pub html: String,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

/// Build the axum router for the Web API.
pub fn app(config: WebConfig) -> Router {
    let state = AppState {
        config: Arc::new(config),
    };
    Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/recalculate", post(recalculate))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
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

fn run_recalculate(
    config: &WebConfig,
    request: &RecalculateRequest,
) -> anyhow::Result<RecalculateResponse> {
    let pcc = resolve_path(&config.data_root, &request.pcc);
    let pcg = resolve_path(&config.data_root, &request.pcg);
    let (pack, _report) = compile_pcc(&pcc)?;
    let draft = draft_from_pcg(parse_pcg_draft_file(&pcg)?);
    let snapshot = Engine::new(pack).evaluate(&draft);
    let snapshot_json: serde_json::Value = serde_json::from_str(&render_json(&snapshot)?)?;
    let choices = serde_json::to_value(&snapshot.choices)?;
    let ability_pool_usage = serde_json::to_value(&snapshot.ability_pool_usage)?;
    Ok(RecalculateResponse {
        snapshot: snapshot_json,
        choices,
        unresolved_choices: snapshot.unresolved_choices.clone(),
        ability_pool_usage,
        diagnostics: snapshot.diagnostics.clone(),
        html: render_html(&snapshot),
    })
}

fn resolve_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
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
            })
            .collect(),
        equipment: draft.equipment,
        equipment_modifiers: draft.equipment_modifiers,
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
