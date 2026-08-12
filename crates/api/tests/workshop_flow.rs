use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use api::{AppState, build_router};
use capability_workshop::{InMemoryServiceOrderRepository, WorkshopState, build_workshop_router};

/// Menggabungkan Core router + Workshop router — pola sama seperti
/// `main.rs`. `business_service`/`customer_service` disalin dari
/// `core_state` SEBELUM `build_router` memindahnya, supaya Workshop
/// memakai instance `BusinessRepository`/`CustomerRepository` YANG SAMA
/// dengan Core (bukan repository in-memory terpisah yang datanya beda).
fn app() -> Router {
    let core_state = AppState::new_in_memory();
    let business_service_for_workshop = core_state.business_service.clone();
    let customer_service_for_workshop = core_state.customer_service.clone();
    let core_router = build_router(core_state);

    let workshop_state = WorkshopState::new(
        business_service_for_workshop,
        customer_service_for_workshop,
        InMemoryServiceOrderRepository::new(),
    );
    let workshop_router = build_workshop_router(workshop_state);

    core_router.merge(workshop_router)
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

/// Helper: buat Tenant, Business (jenis "workshop"), dan satu Customer di
/// bawahnya. Kembalikan (business_id, customer_id).
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
            json!({ "name": "Bengkel Jaya", "business_type": "workshop" }),
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
async fn full_flow_create_start_complete() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, order) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli dan servis rem" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(order["status"], "received");
    assert_eq!(order["business_id"], business_id);
    assert_eq!(order["customer_id"], customer_id);
    assert_eq!(order["transaction_id"], Value::Null);
    let order_id = order["id"].as_str().unwrap().to_string();

    let (status, started) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/start"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(started["status"], "in_progress");

    let (status, completed) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/complete"),
            json!({ "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["transaction_id"], Value::Null);
}

#[tokio::test]
async fn complete_service_order_can_link_a_transaction() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, transaction) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/transactions"),
            json!({ "kind": "service", "amount": 150000 }),
        ),
    )
    .await;
    let transaction_id = transaction["id"].as_str().unwrap().to_string();

    let (_, order) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti kampas rem" }),
        ),
    )
    .await;
    let order_id = order["id"].as_str().unwrap().to_string();

    send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/start"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    let (status, completed) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/complete"),
            json!({ "expected_version": 1, "transaction_id": transaction_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(completed["transaction_id"], transaction_id);
}

#[tokio::test]
async fn complete_service_order_rejects_directly_from_received() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, order) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    let order_id = order["id"].as_str().unwrap().to_string();

    let (status, err) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/complete"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        err["error"]
            .as_str()
            .unwrap()
            .contains("tidak bisa mengubah status")
    );
}

#[tokio::test]
async fn start_service_order_rejects_stale_version() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, order) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    let order_id = order["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/start"),
            json!({ "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn cancel_service_order_marks_cancelled() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, order) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    let order_id = order["id"].as_str().unwrap().to_string();

    let (status, cancelled) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/service-orders/{order_id}/cancel"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["status"], "cancelled");
}

#[tokio::test]
async fn create_service_order_rejects_empty_description() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (status, err) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "   " }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("kosong"));
}

#[tokio::test]
async fn create_service_order_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();
    let customer_id = domain::CustomerId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{random_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_service_order_rejects_when_business_is_deleted() {
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
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_service_order_with_same_id_is_idempotent() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;
    let order_id = "0198c000-0000-7000-8000-000000000001".to_string();

    let (status, first) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "id": order_id, "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry dengan Id sama, deskripsi beda — harus 200 OK, mengembalikan
    // service order PERTAMA.
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "id": order_id, "customer_id": customer_id, "description": "Deskripsi lain" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["description"], first["description"]);
}

#[tokio::test]
async fn list_service_orders_scoped_to_business_and_includes_soft_deleted() {
    let app = app();
    let (business_id, customer_id) = setup_business_with_customer(&app).await;

    let (_, order) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/service-orders"),
            json!({ "customer_id": customer_id, "description": "Ganti oli" }),
        ),
    )
    .await;
    let order_id = order["id"].as_str().unwrap().to_string();

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/service-orders")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/service-orders/{order_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/service-orders")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["is_deleted"], true);
}

#[tokio::test]
async fn list_service_orders_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        get_request(&format!("/businesses/{random_id}/service-orders")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
