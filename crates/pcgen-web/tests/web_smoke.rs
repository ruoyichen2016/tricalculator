//! Web API smoke tests that do not require external PCGen data.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pcgen_web::{app, WebConfig};
use tower::ServiceExt;

fn mini_config() -> WebConfig {
    WebConfig {
        data_root: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures"),
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
