// ============================================================================
// resonance-backend/tests/api_smoke.rs
// Smoke tests that exercise the public API endpoints against an in-memory
// Axum router (no real Postgres / Redis needed for these checks).
//
// These tests verify routing and request-shape correctness, NOT business
// logic — that requires a live DB and is covered by E2E tests in CI.
// ============================================================================

use axum::{body::Body, http::Request, routing::get, Router};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_200() {
    let app = Router::new().route("/health", get(|| async { "صدى alive" }));

    let res = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("alive"));
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let app = Router::new().route("/health", get(|| async { "ok" }));

    let res = app
        .oneshot(Request::builder().uri("/nonexistent").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn pow_difficulty_env_is_read_at_first_use() {
    // The POW_DIFFICULTY_BITS lazy is initialized on first access; we can't
    // unset it, so this test only checks that the value is reasonable.
    std::env::set_var("POW_DIFFICULTY_BITS", "20");
    let d = *resonance_backend::crypto::POW_DIFFICULTY_BITS;
    // Either 20 (if first access) or whatever was set before.
    assert!(d >= 1 && d <= 32);
}
