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
    assert!(body.contains("Rules workbench"));
    assert!(!body.contains("<script"));
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
async fn preview_returns_ability_pool_usage() {
    let body = serde_json::json!({
        "pcc": "campaign-mini/mini.pcc",
        "request": {
            "id": "",
            "name": "Preview Hero",
            "race": "Human",
            "class": "Fighter",
            "level": 1,
            "alignment": "NG",
            "campaign": "campaign-mini/mini.pcc",
            "stats": {},
            "point_buy_budget": null,
            "skills": {},
            "abilities": [],
            "spells": [],
            "equipment": [],
            "deity": null,
            "domains": []
        }
    });
    let response = app(mini_config())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/preview")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["ability_pool_usage"].is_object());
    // The preview also reports the derived skill-point budget for validation.
    assert!(value["preview"]["skill_points"].is_number());
}

#[tokio::test]
async fn structured_option_search_endpoints_respond() {
    let app = app(mini_config());
    for uri in [
        "/api/options/spells?pcc=campaign-mini/mini.pcc",
        "/api/options/equipment?pcc=campaign-mini/mini.pcc",
        "/api/options/abilities?pcc=campaign-mini/mini.pcc",
        "/api/options/races?pcc=campaign-mini/mini.pcc",
        "/api/options/templates?pcc=campaign-mini/mini.pcc",
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
        if uri.contains("/abilities?") {
            for item in value["items"].as_array().unwrap() {
                assert!(item["requires"].is_array(), "ability item missing requires");
            }
        }
    }
}

#[tokio::test]
async fn builder_create_accepts_class_and_race_uuids() {
    let config = character_config("uuid-class");
    let characters_root = config.characters_root.clone();
    let app = app(config);

    let options_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/pack-options?pcc=campaign-mini/mini.pcc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(options_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let options: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let class = options["classes"][0].clone();
    let class_uuid = class["uuid"].as_str().expect("class uuid").to_string();
    let class_name = class["name"].as_str().expect("class name").to_string();

    let race_uuid = options["races"]
        .as_array()
        .unwrap()
        .iter()
        .find(|race| race["name"] == "Human")
        .unwrap()["uuid"]
        .clone();

    let body = serde_json::json!({
        "pcc": "campaign-mini/mini.pcc",
        "request": {
            "id": "",
            "name": "UUID Hero",
            "race": race_uuid,
            "class": class_uuid,
            "level": 1,
            "alignment": "NG",
            "campaign": "campaign-mini/mini.pcc",
            "stats": {},
            "point_buy_budget": null,
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
    if response.status() != StatusCode::CREATED {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        eprintln!("create response body: {}", String::from_utf8_lossy(&bytes));
        panic!("expected CREATED, got {:?}", status);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/uuid-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let saved_class = project_classes(&detail["envelope"]["character"])[0]["class_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        saved_class.eq_ignore_ascii_case(&class_name)
            || class_name.eq_ignore_ascii_case(&saved_class)
    );

    let _ = std::fs::remove_dir_all(characters_root);
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
    assert!(detail["envelope"]["character"]
        .get("class_levels")
        .is_none());
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

#[tokio::test]
async fn level_up_character_applies_class_and_reports_errors() {
    let config = character_config("levelup");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"mini-hero"}"#,
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
                .method("POST")
                .uri("/api/characters/mini-hero/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"Fighter","hp_gained":7}}"#,
                ))
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
        project_classes(&detail["envelope"]["character"])[0]["level"],
        3
    );
    assert!(detail["snapshot"].is_object());

    // Persisted: reopening the character keeps the new level.
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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        project_classes(&detail["envelope"]["character"])[0]["level"],
        3
    );

    // Unknown class is a bad request and must not mutate the save.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/mini-hero/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"NoSuchClass"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Missing character is a 404.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/ghost/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"Fighter"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn level_up_preview_does_not_persist_and_lanes_advance() {
    let config = character_config("levelup-preview");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"preview-hero"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // preview:true computes a level-up but must not touch the saved character.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/preview-hero/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","preview":true,"request":{"class":"Fighter","hp_gained":7}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(detail["preview"], true);
    assert_eq!(project_classes(&detail["character"])[0]["level"], 3);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/preview-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        project_classes(&detail["envelope"]["character"])[0]["level"],
        2
    );

    // Applying the same level with an extra lane advances both classes.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/preview-hero/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"Fighter","hp_gained":7,"lanes":[{"class":"Wizard","hp_gained":3}]}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let projected = project_classes(&detail["envelope"]["character"]);
    let classes = projected.as_array().unwrap();
    assert!(classes.iter().any(|entry| entry["class_id"] == "Wizard"));

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn undo_restores_previous_character_version() {
    let config = character_config("undo");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"undo-hero"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Level up once, then undo and check the level goes back.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/undo-hero/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"Fighter","hp_gained":7}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/undo-hero/undo")
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
        project_classes(&detail["envelope"]["character"])[0]["level"],
        2
    );

    // Only one revision was saved, so the stack is empty now.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/undo-hero/undo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn redo_reapplies_and_history_reports_depths() {
    let config = character_config("redo");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"redo-hero"}"#,
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
                .method("POST")
                .uri("/api/characters/redo-hero/level-up")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"Fighter","hp_gained":7}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Undo, then check history reports a redo step.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/redo-hero/undo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/redo-hero/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let history: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(history["undo_depth"], 0);
    assert_eq!(history["redo_depth"], 1);

    // Redo brings the level back up.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/redo-hero/redo")
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
        project_classes(&detail["envelope"]["character"])[0]["level"],
        3
    );
    assert_eq!(detail["redo_depth"], 0);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn history_lists_revisions_and_restore_jumps_to_one() {
    let config = character_config("history");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"hist-hero"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/characters/hist-hero/level-up")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"pcc":"campaign-mini/mini.pcc","request":{"class":"Fighter","hp_gained":7}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/characters/hist-hero/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let history: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let versions = history["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["total_level"], 2);
    assert_eq!(versions[1]["total_level"], 3);

    // Jump to the oldest revision (level 2).
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/hist-hero/history/1/restore")
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
        project_classes(&detail["envelope"]["character"])[0]["level"],
        2
    );

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn bundle_export_import_round_trip() {
    let config = character_config("bundle");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"bundled"}"#,
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
                .uri("/api/characters/bundle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let bundle: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(bundle["characters"].as_array().unwrap().len(), 1);

    // Re-importing the bundle is idempotent.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/bundle")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "characters": bundle["characters"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

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
    assert_eq!(list.as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn bundle_import_rejects_newer_schema() {
    let config = character_config("bundle-schema");
    let characters_root = config.characters_root.clone();
    let app = app(config);

    // Create a real character, export it, then bump its schema.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"future-hero"}"#,
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
                .uri("/api/characters/bundle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let mut bundle: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    bundle["characters"][0]["schema"] = serde_json::json!(999);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters/bundle")
                .header("content-type", "application/json")
                .body(Body::from(bundle.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn bundle_delete_removes_selected_ids() {
    let config = character_config("bundle-delete");
    let characters_root = config.characters_root.clone();
    let app = app(config);

    for id in ["keep-me", "drop-me"] {
        let body = serde_json::json!({
            "pcc": "campaign-mini/mini.pcc",
            "pcg": "campaign-mini/mini_character.pcg",
            "id": id,
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/characters")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/characters/bundle")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "ids": ["drop-me", "missing-id"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result["deleted"], serde_json::json!(["drop-me"]));
    assert_eq!(result["missing"], serde_json::json!(["missing-id"]));

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
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], "keep-me");

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn characters_endpoint_filters_by_pcc_fingerprint() {
    let config = character_config("filter");
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
                    r#"{"pcc":"campaign-mini/mini.pcc","pcg":"campaign-mini/mini_character.pcg","id":"match"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // A character saved against a different campaign has a different fingerprint.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/characters")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"pcc":"pcc/root.pcc","pcg":"campaign-mini/mini_character.pcg","id":"other"}"#,
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
                .uri("/api/characters?pcc=campaign-mini/mini.pcc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], "match");

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
    assert_eq!(list.as_array().unwrap().len(), 2);

    let _ = std::fs::remove_dir_all(characters_root);
}

#[tokio::test]
async fn characters_endpoint_supports_search_filters() {
    let config = character_config("search");
    let characters_root = config.characters_root.clone();
    let app = app(config);

    // Both import the same mini .pcg, so they share its level (2).
    for (id, name) in [("fighter-one", "Aria"), ("wizard-two", "Borin")] {
        let body = serde_json::json!({
            "pcc": "campaign-mini/mini.pcc",
            "pcg": "campaign-mini/mini_character.pcg",
            "id": id,
            "name": name,
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/characters")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let cases = [
        ("/api/characters?q=aria", vec!["fighter-one"]),
        ("/api/characters?q=BORIN", vec!["wizard-two"]),
        (
            "/api/characters?class=Fighter",
            vec!["fighter-one", "wizard-two"],
        ),
        ("/api/characters?min_level=3", vec![]),
        (
            "/api/characters?max_level=3",
            vec!["fighter-one", "wizard-two"],
        ),
    ];
    for (uri, expected) in cases {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let mut ids: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        ids.sort();
        let mut expected = expected;
        expected.sort();
        assert_eq!(ids, expected, "{uri}");
    }

    let _ = std::fs::remove_dir_all(characters_root);
}

fn project_classes(character: &serde_json::Value) -> serde_json::Value {
    let draft: pcgen_core::CharacterDraft = serde_json::from_value(character.clone()).unwrap();
    serde_json::json!(draft
        .class_levels()
        .iter()
        .map(|entry| serde_json::json!({"class_id": entry.class_id, "level": entry.level}))
        .collect::<Vec<_>>())
}

#[tokio::test]
async fn html_workbench_routes_render_zero_js_and_support_gui_workflows() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = WebConfig {
        data_root: workspace_root.clone(),
        cohort_root: workspace_root,
        characters_root: temp_characters_root("gui-workflows"),
    };
    let characters_root = config.characters_root.clone();
    let app = app(config);

    // 1. Create a character directly from /build with initial BuildHistory.
    let create_form = "op=create&id=studio-hero&name=Studio+Hero&race_id=Human&alignment=NG&class=Warmage&hp=6&STR=14&DEX=12&CON=14&INT=14&WIS=10&CHA=10";
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/build")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(create_form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("Created character with explicit acquisition history."));
    assert!(body.contains("Studio Hero"));
    assert!(body.contains("id=\"build-skills\""));
    assert!(!body.contains("<script"));

    // 2. Character Overview page renders 6-ability grid and combat strip.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/?id=studio-hero")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("Abilities &amp; combat statistics"));
    assert!(body.contains("Armor Class"));
    assert!(!body.contains("<script"));

    // 3. Encounter desk supports focused sub-view filtering.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/class-play?id=studio-hero&view=scene")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("id=\"scene\""));
    assert!(!body.contains("id=\"resources\""));
    assert!(!body.contains("id=\"air\""));
    assert!(!body.contains("<script"));

    // 4. Rules library supports class filtering and preserves character id.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/classes?id=studio-hero&class=Warmage")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("Warmage"));
    assert!(body.contains("id=studio-hero"));
    assert!(body.contains(r#"<form class="library-filter" method="get" action="/classes">"#));
    assert!(body.contains(r#"<input type="hidden" name="id" value="studio-hero">"#));
    assert!(body.contains(r#"<option value="Warmage" selected>Warmage</option>"#));
    assert!(body.contains(r#"<details class="library-more"><summary>"#));
    assert!(!body.contains(r#"<details class="library-more" open"#));
    // No second wall of class anchors above the compact filter.
    assert!(!body.contains(r#"aria-label="On this page""#));
    assert!(body.contains(r#"<section id="warmage">"#));
    assert!(!body.contains(r#"<section id="fighter">"#));
    let disclosure = body
        .split(r#"<details class="library-more">"#)
        .nth(1)
        .unwrap()
        .split("</details>")
        .next()
        .unwrap();
    assert!(disclosure.contains("Filter by class"));
    assert!(disclosure.contains("/martial?id=studio-hero"));
    assert!(disclosure.contains("/incarnum?id=studio-hero"));
    assert!(disclosure.contains("/joker?id=studio-hero"));
    assert!(!body.contains("<script"));

    // Unknown filters remain visible and cannot masquerade as "All classes".
    let response = app
        .oneshot(
            Request::builder()
                .uri("/classes?id=studio-hero&class=Unknown%3Cclass%3E")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("No matching class"));
    assert!(body.contains(r#"<option value="Unknown&lt;class&gt;" selected>Unknown class: Unknown&lt;class&gt;</option>"#));
    assert!(body.contains(r#"name="id" value="studio-hero""#));

    let _ = std::fs::remove_dir_all(characters_root);
}
