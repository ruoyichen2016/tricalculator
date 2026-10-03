//! Web API smoke tests that do not require external PCGen data.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pcgen_web::{app, WebConfig};
use tower::ServiceExt;

fn fixtures_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

fn temp_characters_root(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("pcgen-web-smoke-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn mini_config() -> WebConfig {
    WebConfig {
        data_root: fixtures_root(),
        cohort_root: std::path::PathBuf::from("."),
        characters_root: temp_characters_root("mini"),
    }
}

fn character_config(label: &str) -> WebConfig {
    WebConfig {
        data_root: fixtures_root(),
        cohort_root: std::path::PathBuf::from("."),
        characters_root: temp_characters_root(label),
    }
}

#[tokio::test]
async fn health_endpoint_responds_ok() {
    let response = app(mini_config())
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn index_serves_html() {
    let response = app(mini_config())
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("PCGen-Rust Web"));
}

#[tokio::test]
async fn recalculate_missing_campaign_is_bad_request() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/recalculate")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"pcc":"missing.pcc","pcg":"missing.pcg"}"#))
        .unwrap();
    let response = app(mini_config()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn curated_endpoint_lists_cases() {
    let data_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("../../raw/pcgen-master");
    let response = app(WebConfig {
        data_root,
        cohort_root: std::path::PathBuf::from("."),
        characters_root: temp_characters_root("curated"),
    })
    .oneshot(
        Request::builder()
            .uri("/api/curated")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["cases"]
        .as_array()
        .is_some_and(|cases| !cases.is_empty()));
}

#[tokio::test]
async fn recalculate_applies_choice_overrides() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/recalculate")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{
                "pcc":"campaign-mini/mini.pcc",
                "pcg":"campaign-mini/mini_character.pcg",
                "overrides":{"choices":{"Test Choice":"Climb"}}
            }"#,
        ))
        .unwrap();
    let response = app(mini_config()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["snapshot"]["choices"].is_object());
}

#[tokio::test]
async fn recalculate_mini_campaign_returns_snapshot() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/recalculate")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg"}"#,
        ))
        .unwrap();
    let response = app(mini_config()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["snapshot"]["total_level"].is_number());
    assert!(value["html"].as_str().unwrap().contains("<!doctype html>"));
}

#[tokio::test]
async fn export_returns_lossless_pcg() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/export")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg"}"#,
        ))
        .unwrap();
    let response = app(mini_config()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let pcg = value["pcg"].as_str().unwrap();
    assert!(pcg.contains("CHARACTERNAME:"));
    // Round-trip: the exported text parses again.
    let round = pcgen_import::parse_pcg_draft_str(pcg).unwrap();
    assert!(!round.name.is_empty());
}

#[tokio::test]
async fn curated_response_includes_ability_candidates() {
    let data_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("../../raw/pcgen-master");
    let response = app(WebConfig {
        data_root,
        cohort_root: std::path::PathBuf::from("."),
        characters_root: temp_characters_root("curated"),
    })
    .oneshot(
        Request::builder()
            .uri("/api/curated")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["ability_candidates"].is_object());
}

#[tokio::test]
async fn recalculate_reports_unresolved_hints() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/recalculate")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg"}"#,
        ))
        .unwrap();
    let response = app(mini_config()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["unresolved_hints"].is_array());
}

#[tokio::test]
async fn native_character_crud_round_trip() {
    let config = character_config("crud");
    let characters_root = config.characters_root.clone();
    let app = app(config);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"mini-hero","name":"Mini Hero"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], "mini-hero");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/mini-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(detail["envelope"]["character"]["id"], "mini-hero");
    assert!(detail["snapshot"].is_object());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/characters/mini-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/characters")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn pack_options_endpoint_lists_definitions() {
    let response = app(mini_config())
        .oneshot(
            Request::builder()
                .uri("/api/pack-options?pcc=campaign-mini/mini.pcc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!value["races"].as_array().unwrap().is_empty());
    assert!(!value["classes"].as_array().unwrap().is_empty());
    assert!(value["variants"].as_array().is_some());
    assert_eq!(value["standard_point_buy_budget"], 25);
}

#[tokio::test]
async fn structured_option_search_endpoints_respond() {
    let app = app(mini_config());
    for uri in [
        "/api/options/spells?pcc=campaign-mini/mini.pcc",
        "/api/options/equipment?pcc=campaign-mini/mini.pcc",
        "/api/options/abilities?pcc=campaign-mini/mini.pcc",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "endpoint {uri}");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value["meta"].is_object(), "endpoint {uri} missing meta");
        assert!(value["items"].is_array(), "endpoint {uri} missing items");
    }
}

#[tokio::test]
async fn builder_create_endpoint_round_trip() {
    let config = character_config("builder");
    let characters_root = config.characters_root.clone();
    let app = app(config);
    let body = serde_json::json!({
        "pcc": "campaign-mini/mini.pcc",
        "request": {
            "id": "",
            "name": "Builder Hero",
            "race": "Human",
            "class": "Fighter",
            "level": 1,
            "alignment": "NG",
            "campaign": "campaign-mini/mini.pcc",
            "stats": {"STR": 12, "DEX": 12, "CON": 12, "INT": 10, "WIS": 10, "CHA": 8},
            "point_buy_budget": 25,
            "skills": {},
            "abilities": [],
            "spells": [],
            "equipment": [],
            "deity": null,
            "domains": []
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/create")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/builder-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(detail["envelope"]["character"]["id"], "builder-hero");
    assert!(detail["html"].as_str().is_some());

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn builder_create_advancement_multiclass() {
    let config = character_config("multiclass");
    let characters_root = config.characters_root.clone();
    let app = app(config);
    let body = serde_json::json!({
        "pcc": "campaign-mini/mini.pcc",
        "request": {
            "id": "",
            "name": "Multi Hero",
            "race": "Human",
            "class": "Fighter",
            "level": 2,
            "advancement": [
                {"character_level": 1, "lanes": [{"class": "Fighter"}]},
                {"character_level": 2, "lanes": [{"class": "Wizard"}]}
            ],
            "alignment": "NG",
            "campaign": "campaign-mini/mini.pcc",
            "stats": {"STR": 12, "DEX": 12, "CON": 12, "INT": 10, "WIS": 10, "CHA": 8},
            "point_buy_budget": 25,
            "skills": {},
            "abilities": [],
            "spells": [],
            "equipment": [],
            "deity": null,
            "domains": []
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/create")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/multi-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        detail["envelope"]["character"]["class_levels"]
            .as_array()
            .map(|v| v.len()),
        Some(0)
    );
    assert_eq!(
        detail["envelope"]["character"]["advancement"]
            .as_array()
            .map(|v| v.len()),
        Some(2)
    );
    assert_eq!(
        detail["envelope"]["character"]["advancement"][1]["lanes"][0]["class_id"],
        "Wizard"
    );
    assert!(detail["html"].as_str().is_some());

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn builder_create_gestalt_advancement() {
    let config = character_config("gestalt");
    let characters_root = config.characters_root.clone();
    let app = app(config);
    let body = serde_json::json!({
        "pcc": "campaign-mini/mini.pcc",
        "request": {
            "id": "",
            "name": "Gestalt Hero",
            "race": "Human",
            "class": "Fighter",
            "level": 2,
            "advancement": [
                {
                    "character_level": 1,
                    "lanes": [{"class": "Fighter"}, {"class": "Wizard"}]
                },
                {
                    "character_level": 2,
                    "lanes": [{"class": "Fighter"}, {"class": "Wizard"}]
                }
            ],
            "alignment": "NG",
            "campaign": "campaign-mini/mini.pcc",
            "stats": {"STR": 12, "DEX": 12, "CON": 12, "INT": 10, "WIS": 10, "CHA": 8},
            "point_buy_budget": 25,
            "skills": {},
            "abilities": [],
            "spells": [],
            "equipment": [],
            "deity": null,
            "domains": []
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/create")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/gestalt-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        detail["envelope"]["character"]["advancement"]
            .as_array()
            .map(|v| v.len()),
        Some(2)
    );
    assert_eq!(
        detail["envelope"]["character"]["advancement"][0]["lanes"]
            .as_array()
            .map(|v| v.len()),
        Some(2)
    );

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn builder_create_wizard_with_spell() {
    let config = character_config("wizard");
    let characters_root = config.characters_root.clone();
    let app = app(config);
    let body = serde_json::json!({
        "pcc": "campaign-mini/mini.pcc",
        "request": {
            "id": "",
            "name": "Wizard Test",
            "race": "Human",
            "class": "Wizard",
            "level": 1,
            "alignment": "NG",
            "campaign": "campaign-mini/mini.pcc",
            "stats": {},
            "point_buy_budget": 25,
            "skills": {},
            "abilities": [],
            "spells": [{"name": "Magic Missile", "level": 0, "times_memorized": 1}],
            "equipment": [],
            "deity": null,
            "domains": []
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/create")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/wizard-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        detail["envelope"]["character"]["spells"][0]["name"],
        "Magic Missile"
    );
    assert_eq!(detail["envelope"]["character"]["spells"][0]["level"], 1);

    let _ = std::fs::remove_dir_all(characters_root);
}
