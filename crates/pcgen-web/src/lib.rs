//! Minimal local Web API for PCGen-Rust.
//!
//! This crate intentionally only depends on the compatibility layer
//! (`pcgen-import`) at the application boundary: it compiles a `.pcc` and
//! evaluates a `.pcg` through the public engine API, exactly like the CLI.

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use pcgen_builder::{
    create_draft, level_up_draft, preview_level_up, starting_gold_for_class, BuilderError,
    CharacterRepository, CharacterSummary, CreateRequest, LevelUpRequest, NativeSaveEnvelope,
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
    /// Directory holding native character saves.
    pub characters_root: PathBuf,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            data_root: PathBuf::from("."),
            cohort_root: PathBuf::from("."),
            characters_root: PathBuf::from("characters"),
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<WebConfig>,
    repository: CharacterRepository,
    /// Cache of compiled rule packs keyed by absolute `.pcc` path.
    pack_cache: Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<Engine>>>>,
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
pub struct CreateCharacterRequest {
    pub pcc: String,
    pub pcg: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CharacterDetail {
    pub envelope: NativeSaveEnvelope,
    pub snapshot: Option<serde_json::Value>,
    pub html: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NamedOption {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_gp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_gold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_levels: Option<std::collections::BTreeMap<String, u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serves_as: Option<String>,
    /// Stable UI-facing identifier; internal PCGen key stays in `id`.
    pub uuid: String,
    #[serde(default)]
    pub is_prestige: bool,
    #[serde(default)]
    pub skill_points_per_level: u32,
    #[serde(default)]
    pub skill_multiplier: u32,
    #[serde(default)]
    pub type_tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct VariantOption {
    pub id: String,
    pub name: String,
    pub applies_to: Option<String>,
    pub kind: String,
    /// For substitution-level variants, the class level this variant targets.
    pub level: Option<u32>,
    pub replaces: Vec<String>,
    pub grants: Vec<String>,
    /// True when the variant's `replaces` intersects the class's known features.
    pub matches_features: bool,
}

#[derive(Debug, Serialize)]
pub struct PageMeta {
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpellOption {
    pub id: String,
    pub name: String,
    pub school: String,
    pub class_levels: std::collections::BTreeMap<String, u32>,
    pub domains: Vec<String>,
    pub is_psionic: bool,
}

#[derive(Debug, Serialize)]
pub struct SpellOptionsResponse {
    pub items: Vec<SpellOption>,
    pub meta: PageMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct EquipmentOption {
    pub id: String,
    pub name: String,
    pub type_tags: Vec<String>,
    pub slot: String,
    pub base_item: Option<String>,
    pub cost_gp: f64,
}

#[derive(Debug, Serialize)]
pub struct EquipmentOptionsResponse {
    pub items: Vec<EquipmentOption>,
    pub meta: PageMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbilityOptionDetail {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub type_tags: Vec<String>,
    /// Prerequisite ability/feat names extracted from `requirements`.
    pub requires: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AbilityOptionsResponse {
    pub items: Vec<AbilityOptionDetail>,
    pub meta: PageMeta,
}

#[derive(Debug, Deserialize)]
pub struct SpellOptionsQuery {
    pub pcc: String,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub level: Option<u32>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub school: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct EquipmentOptionsQuery {
    pub pcc: String,
    #[serde(default)]
    pub type_tag: Option<String>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub base_item: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateOptionsQuery {
    pub pcc: String,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateOptionDetail {
    pub uuid: String,
    pub id: String,
    pub name: String,
    pub type_tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TemplateOptionsResponse {
    pub items: Vec<TemplateOptionDetail>,
    pub meta: PageMeta,
}

#[derive(Debug, Deserialize)]
pub struct RaceOptionsQuery {
    pub pcc: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaceOptionDetail {
    pub uuid: String,
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub serves_as: Option<String>,
    pub type_tags: Vec<String>,
    pub size: String,
    pub speed: std::collections::BTreeMap<String, u32>,
    pub ability_adjustments: std::collections::BTreeMap<String, i32>,
    pub favored_class: Option<String>,
    pub level_adjustment: i32,
}

#[derive(Debug, Serialize)]
pub struct RaceOptionsResponse {
    pub categories: Vec<String>,
    pub items: Vec<RaceOptionDetail>,
    pub meta: PageMeta,
}

#[derive(Debug, Deserialize)]
pub struct AbilityOptionsQuery {
    pub pcc: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub type_tag: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct PackOptions {
    pub races: Vec<NamedOption>,
    pub classes: Vec<NamedOption>,
    pub skills: Vec<NamedOption>,
    pub ability_categories: Vec<NamedOption>,
    pub variants: Vec<VariantOption>,
    pub deities: Vec<NamedOption>,
    pub domains: Vec<NamedOption>,
    pub alignments: Vec<NamedOption>,
    pub standard_point_buy_budget: u32,
}

#[derive(Debug, Deserialize)]
pub struct PackOptionsQuery {
    pub pcc: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFromRequest {
    pub pcc: String,
    pub request: CreateRequest,
}

#[derive(Debug, Deserialize)]
pub struct LevelUpFromRequest {
    pub pcc: String,
    pub request: LevelUpRequest,
    /// When true, compute the level-up and return the result without saving.
    #[serde(default)]
    pub preview: bool,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub pcc: String,
    pub request: CreateRequest,
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
    let repository = CharacterRepository::new(config.characters_root.clone());
    let state = AppState {
        config: Arc::new(config),
        repository,
        pack_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };
    Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/curated", get(curated))
        .route("/api/recalculate", post(recalculate))
        .route("/api/export", post(export))
        .route("/api/pack-options", get(pack_options))
        .route("/api/options/spells", get(spell_search_options))
        .route("/api/options/equipment", get(equipment_search_options))
        .route("/api/options/abilities", get(ability_search_options))
        .route("/api/options/races", get(race_search_options))
        .route("/api/options/templates", get(template_search_options))
        .route("/api/preview", post(preview_character))
        .route("/api/characters", get(characters).post(create_character))
        .route(
            "/api/characters/create",
            post(create_character_from_request),
        )
        .route(
            "/api/characters/{id}",
            get(character).delete(delete_character),
        )
        .route("/api/characters/{id}/level-up", post(level_up_character))
        .with_state(state)
}

fn cached_engine(state: &AppState, pcc_path: &Path) -> Result<Arc<Engine>, String> {
    let key = pcc_path.to_string_lossy().to_string();
    if let Some(engine) = state
        .pack_cache
        .lock()
        .map_err(|_| "pack cache lock poisoned".to_string())?
        .get(&key)
        .cloned()
    {
        return Ok(engine);
    }
    let (pack, _) = compile_pcc(pcc_path).map_err(|error| error.to_string())?;
    let engine = Arc::new(Engine::new(pack));
    state
        .pack_cache
        .lock()
        .map_err(|_| "pack cache lock poisoned".to_string())?
        .insert(key, engine.clone());
    Ok(engine)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

fn api_error(status: StatusCode, message: impl Into<String>) -> axum::response::Response {
    (
        status,
        Json(ApiError {
            error: message.into(),
        }),
    )
        .into_response()
}

async fn characters(State(state): State<AppState>) -> impl IntoResponse {
    match state.repository.list() {
        Ok(list) => (StatusCode::OK, Json(list)).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

/// Evaluate a native save and build the response returned when opening a
/// character or applying a level-up.
fn evaluate_envelope(state: &AppState, envelope: NativeSaveEnvelope) -> CharacterDetail {
    let (snapshot, html, error) = match envelope.pcc.as_deref() {
        Some(pcc) => {
            let pcc_path = resolve_path(&state.config.data_root, pcc);
            match cached_engine(state, &pcc_path) {
                Ok(engine) => {
                    let evaluated = engine.evaluate(&envelope.character);
                    let snapshot = render_json(&evaluated)
                        .ok()
                        .and_then(|text| serde_json::from_str(&text).ok());
                    let html = Some(render_html(&evaluated));
                    (snapshot, html, None)
                }
                Err(error) => (None, None, Some(error)),
            }
        }
        None => (
            None,
            None,
            Some("native character has no pcc path".to_string()),
        ),
    };
    CharacterDetail {
        envelope,
        snapshot,
        html,
        error,
    }
}

async fn character(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    match state.repository.load(&id) {
        Ok(envelope) => (StatusCode::OK, Json(evaluate_envelope(&state, envelope))).into_response(),
        Err(BuilderError::NotFound(_)) => {
            api_error(StatusCode::NOT_FOUND, format!("character not found: {id}"))
        }
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

/// Summarise the derived changes between two evaluated snapshots.
fn snapshot_delta(
    before: &pcgen_engine::CharacterSnapshot,
    after: &pcgen_engine::CharacterSnapshot,
) -> serde_json::Value {
    use serde_json::json;

    let stat_delta: serde_json::Value = before
        .stats
        .iter()
        .filter_map(|(key, value)| {
            let next = after.stats.get(key).copied().unwrap_or(*value);
            (next != *value).then(|| (key.clone(), json!(next - value)))
        })
        .collect::<serde_json::Map<_, _>>()
        .into();

    let skill_delta: serde_json::Value = before
        .skills
        .iter()
        .chain(after.skills.iter())
        .map(|(key, _)| key)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|key| {
            let previous = before.skills.get(key).copied().unwrap_or(0);
            let next = after.skills.get(key).copied().unwrap_or(0);
            (next != previous).then(|| (key.clone(), json!(next - previous)))
        })
        .collect::<serde_json::Map<_, _>>()
        .into();

    json!({
        "total_level": after.total_level as i64 - before.total_level as i64,
        "hit_points": after.hit_points - before.hit_points,
        "base_attack_bonus": after.base_attack_bonus - before.base_attack_bonus,
        "armor_class": after.armor_class - before.armor_class,
        "skill_points": after.skill_points - before.skill_points,
        "feat_slots": after.feat_slots as i64 - before.feat_slots as i64,
        "saves": {
            "fortitude": after.saves.get("Fortitude").copied().unwrap_or(0)
                - before.saves.get("Fortitude").copied().unwrap_or(0),
            "reflex": after.saves.get("Reflex").copied().unwrap_or(0)
                - before.saves.get("Reflex").copied().unwrap_or(0),
            "will": after.saves.get("Will").copied().unwrap_or(0)
                - before.saves.get("Will").copied().unwrap_or(0),
        },
        "stats": stat_delta,
        "skills": skill_delta,
        "added_abilities": after
            .active_abilities
            .iter()
            .filter(|ability| !before.active_abilities.contains(ability))
            .cloned()
            .collect::<Vec<_>>(),
    })
}

/// Apply one level (and optional creature templates) to a saved character.
async fn level_up_character(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<LevelUpFromRequest>,
) -> impl IntoResponse {
    let mut envelope = match state.repository.load(&id) {
        Ok(envelope) => envelope,
        Err(BuilderError::NotFound(_)) => {
            return api_error(StatusCode::NOT_FOUND, format!("character not found: {id}"))
        }
        Err(error) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let pcc_path = resolve_path(&state.config.data_root, &body.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let mut request = body.request;
    map_level_up_request(pack, &mut request);
    if body.preview {
        let preview = match preview_level_up(pack, &envelope.character, &request, None) {
            Ok(preview) => preview,
            Err(error) => return api_error(StatusCode::BAD_REQUEST, error.to_string()),
        };
        let before = engine.evaluate(&envelope.character);
        let evaluated = engine.evaluate(&preview);
        let snapshot: Option<serde_json::Value> = render_json(&evaluated)
            .ok()
            .and_then(|text| serde_json::from_str(text.as_str()).ok());
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "preview": true,
                "character": preview,
                "snapshot": snapshot,
                "delta": snapshot_delta(&before, &evaluated),
                "html": render_html(&evaluated),
            })),
        )
            .into_response();
    }
    if let Err(error) = level_up_draft(pack, &mut envelope.character, &request) {
        return api_error(StatusCode::BAD_REQUEST, error.to_string());
    }
    envelope.pcc = Some(relativize(
        &state.config.data_root,
        &pcc_path.to_string_lossy(),
    ));
    envelope.rules_fingerprint = engine.pack.header.rules_fingerprint.clone();
    envelope.data_ref = engine.pack.header.data_ref.clone();
    if let Err(error) = state.repository.save(&envelope) {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    (StatusCode::OK, Json(evaluate_envelope(&state, envelope))).into_response()
}

/// Resolve class ids coming from the UI (uuid or key) across all lanes.
fn map_level_up_request(pack: &pcgen_ir::CompiledRulePack, request: &mut LevelUpRequest) {
    request.class_id = resolve_class_id(pack, &request.class_id);
    for lane in &mut request.lanes {
        lane.class_id = resolve_class_id(pack, &lane.class_id);
    }
}

async fn delete_character(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    match state.repository.delete(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(BuilderError::NotFound(_)) => {
            api_error(StatusCode::NOT_FOUND, format!("character not found: {id}"))
        }
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn create_character(
    State(state): State<AppState>,
    Json(request): Json<CreateCharacterRequest>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &request.pcc);
    let pcg_path = resolve_path(&state.config.data_root, &request.pcg);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let mut draft = match parse_pcg_draft_file(&pcg_path) {
        Ok(pcg) => draft_from_pcg(pcg),
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    if let Some(id) = request.id {
        draft.id = id;
    }
    if draft.id.trim().is_empty() {
        draft.id = pcg_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("character")
            .to_string();
    }
    if let Some(name) = request.name {
        draft.name = name;
    }
    let pcc_relative = relativize(&state.config.data_root, &pcc_path.to_string_lossy());
    let envelope = NativeSaveEnvelope::new(
        draft,
        engine.pack.header.rules_fingerprint.clone(),
        engine.pack.header.data_ref.clone(),
    )
    .with_pcc(Some(pcc_relative));
    let summary = summary_for(&envelope);
    match state.repository.save(&envelope) {
        Ok(()) => (StatusCode::CREATED, Json(summary)).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
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
    match run_recalculate(&state, &request) {
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
    match run_export(&state, &request) {
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

fn run_export(state: &AppState, request: &ExportRequest) -> anyhow::Result<ExportResponse> {
    let pcc = resolve_path(&state.config.data_root, &request.pcc);
    let pcg = resolve_path(&state.config.data_root, &request.pcg);
    let engine = cached_engine(state, &pcc).map_err(anyhow::Error::msg)?;
    let mut draft = draft_from_pcg(parse_pcg_draft_file(&pcg)?);
    apply_overrides(&mut draft, &request.overrides);
    let snapshot = engine.evaluate(&draft);
    let exported = pcg_from_draft(&draft);
    let unresolved_choices = snapshot.unresolved_choices.clone();
    let hints = unresolved_hints(&draft, &unresolved_choices, &engine.pack);
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
    state: &AppState,
    request: &RecalculateRequest,
) -> anyhow::Result<RecalculateResponse> {
    let pcc = resolve_path(&state.config.data_root, &request.pcc);
    let pcg = resolve_path(&state.config.data_root, &request.pcg);
    let engine = cached_engine(state, &pcc).map_err(anyhow::Error::msg)?;
    let mut draft = draft_from_pcg(parse_pcg_draft_file(&pcg)?);
    apply_overrides(&mut draft, &request.overrides);
    let snapshot = engine.evaluate(&draft);
    let snapshot_json: serde_json::Value = serde_json::from_str(&render_json(&snapshot)?)?;
    let choices = serde_json::to_value(&snapshot.choices)?;
    let ability_pool_usage = serde_json::to_value(&snapshot.ability_pool_usage)?;
    let unresolved_choices = snapshot.unresolved_choices.clone();
    let hints = unresolved_hints(&draft, &unresolved_choices, &engine.pack);
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

fn named_options<T>(
    values: impl Iterator<Item = T>,
    id: impl Fn(&T) -> String,
    name: impl Fn(&T) -> String,
) -> Vec<NamedOption> {
    let mut options: Vec<NamedOption> = values
        .map(|value| NamedOption {
            id: id(&value),
            name: name(&value),
            cost_gp: None,
            starting_gold: None,
            class_levels: None,
            serves_as: None,
            uuid: stable_id("option", &id(&value)),
            is_prestige: false,
            skill_points_per_level: 0,
            skill_multiplier: 1,
            type_tags: Vec::new(),
        })
        .collect();
    options.sort_by(|left, right| left.name.cmp(&right.name));
    options
}

fn race_options(pack: &pcgen_ir::CompiledRulePack) -> Vec<NamedOption> {
    let mut options: Vec<NamedOption> = pack
        .races
        .values()
        .map(|race| NamedOption {
            id: race.id.clone(),
            name: race.display_name.clone(),
            cost_gp: None,
            starting_gold: None,
            class_levels: None,
            serves_as: race.serves_as.clone(),
            uuid: stable_id("race", &race.id),
            is_prestige: false,
            skill_points_per_level: 0,
            skill_multiplier: 1,
            type_tags: race.type_tags.clone(),
        })
        .collect();
    options.sort_by(|left, right| left.name.cmp(&right.name));
    options
}

fn class_options(pack: &pcgen_ir::CompiledRulePack) -> Vec<NamedOption> {
    let mut options: Vec<NamedOption> = pack
        .classes
        .values()
        .map(|class| NamedOption {
            id: class.id.clone(),
            name: class.name.clone(),
            cost_gp: None,
            starting_gold: Some(starting_gold_for_class(&class.name)),
            class_levels: None,
            serves_as: class.serves_as.clone(),
            uuid: stable_id("class", &class.id),
            is_prestige: class.is_prestige,
            skill_points_per_level: class.skill_points_per_level,
            skill_multiplier: class.skill_multiplier.unwrap_or(4),
            type_tags: class.type_tags.clone(),
        })
        .collect();
    options.sort_by(|left, right| left.name.cmp(&right.name));
    options
}

fn summary_for(envelope: &NativeSaveEnvelope) -> CharacterSummary {
    let total_level = envelope.character.total_level();
    CharacterSummary {
        id: envelope.character.id.clone(),
        name: envelope.character.name.clone(),
        race_id: envelope.character.race_id.clone(),
        total_level,
        rules_fingerprint: envelope.rules_fingerprint.clone(),
    }
}

fn stable_id(kind: &str, value: &str) -> String {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x9e3779b97f4a7c15;
    let input = format!("{kind}:{value}");
    for byte in input.bytes() {
        h1 ^= u64::from(byte);
        h1 = h1.wrapping_mul(0x100000001b3);
    }
    for byte in input.bytes().rev() {
        h2 ^= u64::from(byte);
        h2 = h2.wrapping_mul(0x100000001b3);
    }
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (h1 >> 32) as u32,
        ((h1 >> 16) & 0xffff) as u16,
        (h1 & 0xfff) as u16,
        ((h2 >> 48) & 0xfff) as u16,
        h2 & 0xffff_ffff_ffff
    )
}

fn normalized_feature(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
}

fn class_feature_names(pack: &pcgen_ir::CompiledRulePack, class_id: &str) -> Vec<String> {
    let mut features = Vec::new();
    if let Some(class) = pack.classes.get(class_id) {
        for abilities in class.level_abilities.values() {
            features.extend(abilities.iter().cloned());
        }
    }
    if let Some(progression) = pack.progressions.get(class_id) {
        let max_level = progression
            .features_by_level
            .keys()
            .max()
            .copied()
            .unwrap_or(20);
        features.extend(progression.features_up_to(max_level));
    }
    features
}

async fn pack_options(
    State(state): State<AppState>,
    Query(query): Query<PackOptionsQuery>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &query.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let options = PackOptions {
        races: race_options(pack),
        classes: class_options(pack),
        skills: named_options(
            pack.skills.values(),
            |item| item.id.clone(),
            |item| item.name.clone(),
        ),
        ability_categories: named_options(
            pack.ability_categories.values(),
            |item| item.id.clone(),
            |item| item.name.clone(),
        ),
        variants: {
            let mut class_features_cache: std::collections::HashMap<
                String,
                std::collections::BTreeSet<String>,
            > = std::collections::HashMap::new();
            pack.variants
                .values()
                .map(|variant| {
                    let kind = if variant.id.contains(" ~ level ") {
                        "substitution"
                    } else {
                        "archetype"
                    };
                    let class_features: std::collections::BTreeSet<String> = variant
                        .applies_to
                        .as_deref()
                        .map(|class_id| {
                            class_features_cache
                                .entry(class_id.to_ascii_lowercase())
                                .or_insert_with(|| {
                                    class_feature_names(pack, class_id)
                                        .into_iter()
                                        .map(|feature| normalized_feature(&feature))
                                        .collect()
                                })
                                .clone()
                        })
                        .unwrap_or_default();
                    let matches_features = variant.replaces.is_empty()
                        || variant.replaces.iter().any(|replaced| {
                            let wanted = normalized_feature(replaced);
                            class_features.iter().any(|feature| {
                                feature == &wanted
                                    || feature.contains(&wanted)
                                    || wanted.contains(feature)
                            })
                        });
                    VariantOption {
                        id: variant.id.clone(),
                        name: variant.name.clone(),
                        applies_to: variant.applies_to.clone(),
                        kind: kind.to_string(),
                        level: variant
                            .id
                            .rsplit_once(" ~ level ")
                            .and_then(|(_, level)| level.trim().parse().ok()),
                        replaces: variant.replaces.clone(),
                        grants: variant.grants.clone(),
                        matches_features,
                    }
                })
                .collect()
        },
        deities: named_options(
            pack.deities.values(),
            |item| item.id.clone(),
            |item| item.name.clone(),
        ),
        domains: named_options(
            pack.domains.values(),
            |item| item.id.clone(),
            |item| item.name.clone(),
        ),
        alignments: named_options(
            pack.alignments.values(),
            |item| item.id.clone(),
            |item| item.name.clone(),
        ),
        standard_point_buy_budget: pcgen_builder::point_buy::STANDARD_BUDGET,
    };
    Json(options).into_response()
}

fn equip_slot_name(slot: pcgen_core::equipment::EquipSlot) -> &'static str {
    match slot {
        pcgen_core::equipment::EquipSlot::Head => "head",
        pcgen_core::equipment::EquipSlot::Body => "body",
        pcgen_core::equipment::EquipSlot::Arms => "arms",
        pcgen_core::equipment::EquipSlot::Hands => "hands",
        pcgen_core::equipment::EquipSlot::Feet => "feet",
        pcgen_core::equipment::EquipSlot::Waist => "waist",
        pcgen_core::equipment::EquipSlot::Neck => "neck",
        pcgen_core::equipment::EquipSlot::Wrist => "wrist",
        pcgen_core::equipment::EquipSlot::Weapon => "weapon",
        pcgen_core::equipment::EquipSlot::OffHand => "offhand",
        pcgen_core::equipment::EquipSlot::Shield => "shield",
        pcgen_core::equipment::EquipSlot::Special => "special",
        pcgen_core::equipment::EquipSlot::None => "none",
    }
}

async fn spell_search_options(
    State(state): State<AppState>,
    Query(query): Query<SpellOptionsQuery>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &query.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let search = query.search.as_deref().map(str::to_lowercase);

    let mut items: Vec<SpellOption> = pack
        .spells
        .values()
        .filter(|spell| {
            if let Some(class) = &query.class {
                if !spell
                    .class_levels
                    .keys()
                    .any(|candidate| candidate.eq_ignore_ascii_case(class))
                {
                    return false;
                }
            }
            if let Some(level) = query.level {
                let level_match = spell.class_levels.values().any(|value| *value == level)
                    || spell.level == level;
                if !level_match {
                    return false;
                }
            }
            if let Some(domain) = &query.domain {
                if !spell
                    .domains
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(domain))
                {
                    return false;
                }
            }
            if let Some(school) = &query.school {
                if !spell.school.eq_ignore_ascii_case(school) {
                    return false;
                }
            }
            if let Some(search) = &search {
                if !spell.name.to_lowercase().contains(search)
                    && !spell.id.to_lowercase().contains(search)
                {
                    return false;
                }
            }
            true
        })
        .map(|spell| SpellOption {
            id: spell.id.clone(),
            name: spell.name.clone(),
            school: spell.school.clone(),
            class_levels: spell.class_levels.clone(),
            domains: spell.domains.clone(),
            is_psionic: spell.is_psionic,
        })
        .collect();
    items.sort_by(|left, right| left.name.cmp(&right.name));
    let total = items.len();
    let start = ((page - 1) as usize * page_size as usize).min(total);
    let end = (start + page_size as usize).min(total);
    Json(SpellOptionsResponse {
        meta: PageMeta {
            page,
            page_size,
            total,
            total_pages: total.div_ceil(page_size as usize) as u32,
        },
        items: items[start..end].to_vec(),
    })
    .into_response()
}

async fn equipment_search_options(
    State(state): State<AppState>,
    Query(query): Query<EquipmentOptionsQuery>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &query.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let search = query.search.as_deref().map(str::to_lowercase);

    let mut items: Vec<EquipmentOption> = pack
        .equipment
        .values()
        .filter(|item| {
            if let Some(type_tag) = &query.type_tag {
                if !item
                    .type_tags
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(type_tag))
                {
                    return false;
                }
            }
            if let Some(slot) = &query.slot {
                if !equip_slot_name(item.slot).eq_ignore_ascii_case(slot) {
                    return false;
                }
            }
            if let Some(base_item) = &query.base_item {
                if !item
                    .base_item
                    .as_deref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(base_item))
                {
                    return false;
                }
            }
            if let Some(search) = &search {
                if !item.name.to_lowercase().contains(search)
                    && !item.id.to_lowercase().contains(search)
                {
                    return false;
                }
            }
            true
        })
        .map(|item| EquipmentOption {
            id: item.id.clone(),
            name: item.name.clone(),
            type_tags: item.type_tags.clone(),
            slot: equip_slot_name(item.slot).to_string(),
            base_item: item.base_item.clone(),
            cost_gp: item.cost_gp,
        })
        .collect();
    items.sort_by(|left, right| left.name.cmp(&right.name));
    let total = items.len();
    let start = ((page - 1) as usize * page_size as usize).min(total);
    let end = (start + page_size as usize).min(total);
    Json(EquipmentOptionsResponse {
        meta: PageMeta {
            page,
            page_size,
            total,
            total_pages: total.div_ceil(page_size as usize) as u32,
        },
        items: items[start..end].to_vec(),
    })
    .into_response()
}

fn collect_has_entity_keys(predicate: &pcgen_core::Predicate, out: &mut Vec<String>) {
    use pcgen_core::Predicate;
    match predicate {
        Predicate::HasEntity { entity } => out.push(entity.key.clone()),
        Predicate::All(items) | Predicate::Any(items) | Predicate::AtLeast { items, .. } => {
            for item in items {
                collect_has_entity_keys(item, out);
            }
        }
        Predicate::Not(item) => collect_has_entity_keys(item, out),
        _ => {}
    }
}

fn ability_requires(
    pack: &pcgen_ir::CompiledRulePack,
    ability: &pcgen_core::domain::AbilityDefinition,
) -> Vec<String> {
    let entity = pack.entities.iter().find(|entity| {
        entity.id.key.eq_ignore_ascii_case(&ability.id)
            || entity.name.eq_ignore_ascii_case(&ability.name)
    });
    let Some(entity) = entity else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for requirement in &entity.requirements {
        collect_has_entity_keys(requirement, &mut keys);
    }
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|key| {
            pack.abilities.contains_key(key)
                || pack
                    .abilities
                    .values()
                    .any(|candidate| candidate.name.eq_ignore_ascii_case(key))
        })
        .collect()
}

async fn race_search_options(
    State(state): State<AppState>,
    Query(query): Query<RaceOptionsQuery>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &query.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let search = query.search.as_deref().map(str::to_lowercase);
    let mut categories: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for race in pack.races.values() {
        for tag in &race.type_tags {
            categories.insert(tag.clone());
        }
    }
    let mut items: Vec<RaceOptionDetail> = pack
        .races
        .values()
        .filter(|race| {
            if let Some(category) = &query.category {
                if !race
                    .type_tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case(category))
                {
                    return false;
                }
            }
            if let Some(search) = &search {
                if !race.name.to_lowercase().contains(search)
                    && !race.display_name.to_lowercase().contains(search)
                    && !race.id.to_lowercase().contains(search)
                {
                    return false;
                }
            }
            true
        })
        .map(|race| RaceOptionDetail {
            uuid: stable_id("race", &race.id),
            id: race.id.clone(),
            name: race.name.clone(),
            display_name: race.display_name.clone(),
            serves_as: race.serves_as.clone(),
            type_tags: race.type_tags.clone(),
            size: race.size.clone(),
            speed: race.movement.clone(),
            ability_adjustments: race.ability_adjustments.clone(),
            favored_class: race.favored_class.clone(),
            level_adjustment: race.level_adjustment,
        })
        .collect();
    items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    let total = items.len();
    let start = ((page - 1) as usize * page_size as usize).min(total);
    let end = (start + page_size as usize).min(total);
    Json(RaceOptionsResponse {
        categories: categories.into_iter().collect(),
        meta: PageMeta {
            page,
            page_size,
            total,
            total_pages: total.div_ceil(page_size as usize) as u32,
        },
        items: items[start..end].to_vec(),
    })
    .into_response()
}

async fn template_search_options(
    State(state): State<AppState>,
    Query(query): Query<TemplateOptionsQuery>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &query.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let search = query.search.as_deref().map(str::to_lowercase);
    let mut items: Vec<TemplateOptionDetail> = pack
        .templates
        .values()
        .filter(|template| {
            if let Some(search) = &search {
                if !template.name.to_lowercase().contains(search)
                    && !template.id.to_lowercase().contains(search)
                {
                    return false;
                }
            }
            true
        })
        .map(|template| TemplateOptionDetail {
            uuid: stable_id("template", &template.id),
            id: template.id.clone(),
            name: template.name.clone(),
            type_tags: template.type_tags.clone(),
        })
        .collect();
    items.sort_by(|left, right| left.name.cmp(&right.name));
    let total = items.len();
    let start = ((page - 1) as usize * page_size as usize).min(total);
    let end = (start + page_size as usize).min(total);
    Json(TemplateOptionsResponse {
        meta: PageMeta {
            page,
            page_size,
            total,
            total_pages: total.div_ceil(page_size as usize) as u32,
        },
        items: items[start..end].to_vec(),
    })
    .into_response()
}

async fn ability_search_options(
    State(state): State<AppState>,
    Query(query): Query<AbilityOptionsQuery>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &query.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let search = query.search.as_deref().map(str::to_lowercase);

    let mut items: Vec<AbilityOptionDetail> = pack
        .abilities
        .values()
        .filter(|ability| {
            if let Some(category) = &query.category {
                if !ability
                    .category
                    .as_deref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(category))
                {
                    return false;
                }
            }
            if let Some(type_tag) = &query.type_tag {
                if !ability
                    .type_tags
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(type_tag))
                {
                    return false;
                }
            }
            if let Some(search) = &search {
                if !ability.name.to_lowercase().contains(search)
                    && !ability.id.to_lowercase().contains(search)
                {
                    return false;
                }
            }
            true
        })
        .map(|ability| AbilityOptionDetail {
            id: ability.id.clone(),
            name: ability.name.clone(),
            category: ability.category.clone(),
            type_tags: ability.type_tags.clone(),
            requires: ability_requires(pack, ability),
        })
        .collect();
    items.sort_by(|left, right| left.name.cmp(&right.name));
    let total = items.len();
    let start = ((page - 1) as usize * page_size as usize).min(total);
    let end = (start + page_size as usize).min(total);
    Json(AbilityOptionsResponse {
        meta: PageMeta {
            page,
            page_size,
            total,
            total_pages: total.div_ceil(page_size as usize) as u32,
        },
        items: items[start..end].to_vec(),
    })
    .into_response()
}

fn resolve_class_id(pack: &pcgen_ir::CompiledRulePack, value: &str) -> String {
    if pack.classes.contains_key(value) {
        return value.to_string();
    }
    // UI sends deterministic UUIDs; convert back to the internal PCGen key.
    let uuid = value;
    pack.classes
        .iter()
        .find(|(_, class)| stable_id("class", &class.id) == uuid)
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| value.to_string())
}

fn resolve_race_id(pack: &pcgen_ir::CompiledRulePack, value: &str) -> String {
    if pack.races.contains_key(value) {
        return value.to_string();
    }
    let uuid = value;
    pack.races
        .iter()
        .find(|(_, race)| stable_id("option", &race.id) == uuid)
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| value.to_string())
}

fn map_create_request_classes(pack: &pcgen_ir::CompiledRulePack, request: &mut CreateRequest) {
    request.class_id = resolve_class_id(pack, &request.class_id);
    request.race = resolve_race_id(pack, &request.race);
    for frame in &mut request.advancement {
        for lane in &mut frame.lanes {
            lane.class = resolve_class_id(pack, &lane.class);
        }
    }
}

async fn preview_character(
    State(state): State<AppState>,
    Json(body): Json<PreviewRequest>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &body.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let mut request = body.request;
    map_create_request_classes(pack, &mut request);
    let draft = match create_draft(pack, &request) {
        Ok(draft) => draft,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let snapshot = engine.evaluate(&draft);
    let ecl = snapshot.total_level as i32
        + engine
            .pack
            .races
            .get(&draft.race_id)
            .map(|race| race.level_adjustment)
            .unwrap_or(0);
    Json(serde_json::json!({
        "ability_pool_usage": snapshot.ability_pool_usage,
        "preview": {
            "total_level": snapshot.total_level,
            "ecl": ecl,
            "stats": snapshot.stats,
            "skill_points": snapshot.skill_points,
            "feat_slots": snapshot.feat_slots,
            "hit_points": snapshot.hit_points,
            "base_attack_bonus": snapshot.base_attack_bonus,
            "active_abilities": snapshot.active_abilities,
            "templates": draft.templates,
        },
        "diagnostics": snapshot.diagnostics,
    }))
    .into_response()
}

async fn create_character_from_request(
    State(state): State<AppState>,
    Json(body): Json<CreateFromRequest>,
) -> impl IntoResponse {
    let pcc_path = resolve_path(&state.config.data_root, &body.pcc);
    let engine = match cached_engine(&state, &pcc_path) {
        Ok(engine) => engine,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    let pack = &engine.pack;
    let mut request = body.request;
    map_create_request_classes(pack, &mut request);
    let draft = match create_draft(pack, &request) {
        Ok(draft) => draft,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let pcc_relative = relativize(&state.config.data_root, &pcc_path.to_string_lossy());
    let envelope = NativeSaveEnvelope::new(
        draft,
        engine.pack.header.rules_fingerprint.clone(),
        engine.pack.header.data_ref.clone(),
    )
    .with_pcc(Some(pcc_relative));
    let summary = summary_for(&envelope);
    match state.repository.save(&envelope) {
        Ok(()) => (StatusCode::CREATED, Json(summary)).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
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
