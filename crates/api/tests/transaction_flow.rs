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

/// Helper: buat Tenant lalu Business aktif di bawahnya, kembalikan
/// business_id. Pola sama seperti `customer_flow.rs`.
async fn setup_business(app: &Router) -> String {
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
            json!({ "name": "Toko Baju", "business_type": "retail" }),
        ),
    )
    .await;
    business["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn full_flow_create_and_delete() {
    let app = app();
    let business_id = setup_business(&app).await;

    // Buat transaction tanpa customer, tanpa occurred_at (default now()).
    let (status, transaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "sale", "amount": 50000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(transaction["version"], 0);
    assert_eq!(transaction["kind"], "sale");
    assert_eq!(transaction["amount"], 50000);
    assert_eq!(transaction["customer_id"], Value::Null);
    assert!(transaction["occurred_at"].as_str().unwrap().ends_with('Z'));
    let transaction_id = transaction["id"].as_str().unwrap().to_string();

    // Hapus transaction.
    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/transactions/{transaction_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_transaction_can_be_linked_to_customer() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (_, customer) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi" }),
        ),
    )
    .await;
    let customer_id = customer["id"].as_str().unwrap().to_string();

    let (status, transaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "customer_id": customer_id, "kind": "sale", "amount": 25000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(transaction["customer_id"], customer_id);
}

#[tokio::test]
async fn create_transaction_accepts_explicit_occurred_at() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (status, transaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({
                "kind": "sale",
                "amount": 10000,
                "occurred_at": "2026-01-01T00:00:00Z"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        transaction["occurred_at"]
            .as_str()
            .unwrap()
            .starts_with("2026-01-01T00:00:00")
    );
}

#[tokio::test]
async fn create_transaction_rejects_zero_amount() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (status, err) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "sale", "amount": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        err["error"]
            .as_str()
            .unwrap()
            .contains("lebih besar dari nol")
    );
}

#[tokio::test]
async fn create_transaction_rejects_invalid_kind() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "sale online!", "amount": 10000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_transaction_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{random_id}/transactions"),
            json!({ "kind": "sale", "amount": 10000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_transaction_rejects_when_business_is_deleted() {
    let app = app();
    let business_id = setup_business(&app).await;

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
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "sale", "amount": 10000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_transaction_with_same_id_is_idempotent() {
    let app = app();
    let business_id = setup_business(&app).await;
    let transaction_id = domain::TransactionId::new().to_string();

    let (status, first) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "id": transaction_id, "kind": "sale", "amount": 10000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry dengan Id sama, amount beda — harus 200 OK, mengembalikan
    // transaction PERTAMA.
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "id": transaction_id, "kind": "sale", "amount": 99999 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["amount"], first["amount"]);
}

#[tokio::test]
async fn delete_transaction_rejects_stale_version() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (_, transaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "sale", "amount": 10000 }),
        ),
    )
    .await;
    let transaction_id = transaction["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/transactions/{transaction_id}"),
            json!({ "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn list_transactions_scoped_to_business_and_includes_soft_deleted() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (_, transaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "sale", "amount": 10000 }),
        ),
    )
    .await;
    let transaction_id = transaction["id"].as_str().unwrap().to_string();

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/transactions")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    send(
        &app,
        json_request(
            "DELETE",
            &format!("/transactions/{transaction_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/transactions")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["is_deleted"], true);
}

#[tokio::test]
async fn list_transactions_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        get_request(&format!("/businesses/{random_id}/transactions")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
