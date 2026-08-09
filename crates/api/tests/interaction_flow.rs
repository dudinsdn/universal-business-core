use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use api::{AppState, build_router};

fn app() -> Router {
    build_router(AppState::new_in_memory())
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

/// Helper: buat Tenant, Business aktif, dan satu Customer di bawahnya.
/// Kembalikan (business_id, customer_id).
async fn setup_business_with_customer(app: &Router) -> (String, String) {
    let (_, tenant) = send(
        app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    let tenant_id = tenant["id"].as_str().unwrap().to_string();

    let (_, business) = send(
        app,
        json_request(
            "POST",
            &format!("/tenants/{tenant_id}/businesses"),
            json!({ "name": "Klinik A", "business_type": "clinic" }),
        ),
    )
    .await;
    let business_id = business["id"].as_str().unwrap().to_string();

    let (_, customer) = send(
        app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi" }),
        ),
    )
    .await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    (business_id, customer_id)
}

#[tokio::test]
async fn full_flow_create_and_delete() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, interaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({
                "customer_id": customer_id,
                "interaction_type": "call",
                "note": "Follow up jadwal kontrol"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(interaction["version"], 0);
    assert_eq!(interaction["interaction_type"], "call");
    assert_eq!(interaction["customer_id"], customer_id);
    assert_eq!(interaction["note"], "Follow up jadwal kontrol");
    assert!(interaction["occurred_at"].as_str().unwrap().ends_with('Z'));
    let interaction_id = interaction["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/interactions/{interaction_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_interaction_without_note_is_allowed() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, interaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "visit" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(interaction["note"], Value::Null);
}

#[tokio::test]
async fn create_interaction_accepts_explicit_occurred_at() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, interaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({
                "customer_id": customer_id,
                "interaction_type": "call",
                "occurred_at": "2026-01-01T00:00:00Z"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        interaction["occurred_at"]
            .as_str()
            .unwrap()
            .starts_with("2026-01-01T00:00:00")
    );
}

#[tokio::test]
async fn create_interaction_rejects_empty_note() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "call", "note": "   " }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_interaction_rejects_invalid_type() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "phone call!" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_interaction_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();
    let customer_id = domain::CustomerId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{random_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "call" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_interaction_rejects_when_business_is_deleted() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/businesses/{business_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "call" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_interaction_with_same_id_is_idempotent() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;
    let interaction_id = domain::InteractionId::new().to_string();

    let (status, first) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({
                "id": interaction_id,
                "customer_id": customer_id,
                "interaction_type": "call"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry dengan Id sama, jenis beda — harus 200 OK, mengembalikan
    // interaction PERTAMA.
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({
                "id": interaction_id,
                "customer_id": customer_id,
                "interaction_type": "visit"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["interaction_type"], first["interaction_type"]);
}

#[tokio::test]
async fn delete_interaction_rejects_stale_version() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, interaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "call" }),
        ),
    )
    .await;
    let interaction_id = interaction["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/interactions/{interaction_id}"),
            json!({ "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn list_interactions_scoped_to_business_and_includes_soft_deleted() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, interaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/interactions"),
            json!({ "customer_id": customer_id, "interaction_type": "call" }),
        ),
    )
    .await;
    let interaction_id = interaction["id"].as_str().unwrap().to_string();

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/interactions")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    send(
        &app,
        json_request(
            "DELETE",
            &format!("/interactions/{interaction_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/interactions")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["is_deleted"], true);
}

#[tokio::test]
async fn list_interactions_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        get_request(&format!("/businesses/{random_id}/interactions")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
