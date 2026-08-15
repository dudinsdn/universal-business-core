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

/// Helper: buat Tenant, Business aktif, dan dua Customer di bawahnya.
/// Kembalikan (business_id, customer_a_id, customer_b_id).
async fn setup_business_with_two_customers(app: &Router) -> (String, String, String) {
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

    let (_, customer_a) = send(
        app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi" }),
        ),
    )
    .await;
    let customer_a_id = customer_a["id"].as_str().unwrap().to_string();

    let (_, customer_b) = send(
        app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Ani" }),
        ),
    )
    .await;
    let customer_b_id = customer_b["id"].as_str().unwrap().to_string();

    (business_id, customer_a_id, customer_b_id)
}

#[tokio::test]
async fn full_flow_create_and_delete() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;

    let (status, relationship) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(relationship["version"], 0);
    assert_eq!(relationship["relationship_type"], "sibling");
    assert_eq!(relationship["from_customer_id"], customer_a_id);
    assert_eq!(relationship["to_customer_id"], customer_b_id);
    let relationship_id = relationship["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/relationships/{relationship_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_relationship_rejects_self_relationship() {
    let app = app();
    let (business_id, customer_a_id, _) = setup_business_with_two_customers(&app).await;

    let (status, err) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_a_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        err["error"]
            .as_str()
            .unwrap()
            .contains("tidak bisa berelasi dengan dirinya sendiri")
    );
}

#[tokio::test]
async fn create_relationship_rejects_invalid_type() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "family member!"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_relationship_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();
    let from = domain::CustomerId::new().to_string();
    let to = domain::CustomerId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{random_id}/relationships"),
            json!({
                "from_customer_id": from,
                "to_customer_id": to,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_relationship_rejects_when_business_is_deleted() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;

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
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_relationship_with_same_id_is_idempotent() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;
    let relationship_id = domain::RelationshipId::new().to_string();

    let (status, first) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "id": relationship_id,
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry dengan Id sama, jenis beda — harus 200 OK, mengembalikan
    // relationship PERTAMA.
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "id": relationship_id,
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "referral"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["relationship_type"], first["relationship_type"]);
}

#[tokio::test]
async fn delete_relationship_rejects_stale_version() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;

    let (_, relationship) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    let relationship_id = relationship["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/relationships/{relationship_id}"),
            json!({ "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn list_relationships_scoped_to_business_and_includes_soft_deleted() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;

    let (_, relationship) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;
    let relationship_id = relationship["id"].as_str().unwrap().to_string();

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/relationships")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    send(
        &app,
        json_request(
            "DELETE",
            &format!("/relationships/{relationship_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/relationships")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["is_deleted"], true);
}

#[tokio::test]
async fn list_relationships_only_returns_changes_after_cursor() {
    let app = app();
    let (business_id, customer_a_id, customer_b_id) = setup_business_with_two_customers(&app).await;

    send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "sibling"
            }),
        ),
    )
    .await;

    let cursor_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let (_, baru) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/relationships"),
            json!({
                "from_customer_id": customer_a_id,
                "to_customer_id": customer_b_id,
                "relationship_type": "referral"
            }),
        ),
    )
    .await;

    let (status, changed) = send(
        &app,
        get_request(&format!(
            "/businesses/{business_id}/relationships?updated_since={cursor_time}"
        )),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let changed = changed.as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["id"], baru["id"]);
}

#[tokio::test]
async fn list_relationships_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        get_request(&format!("/businesses/{random_id}/relationships")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
