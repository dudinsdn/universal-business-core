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

/// Helper: buat Tenant lalu Business aktif di bawahnya, kembalikan business_id.
/// Setiap test Customer butuh Business yang sudah ada — disatukan di sini
/// supaya tidak diulang di setiap test.
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
async fn full_flow_create_rename_update_phone_delete() {
    let app = app();
    let business_id = setup_business(&app).await;

    // Buat customer dengan nama + telepon.
    let (status, customer) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi", "phone": "081234567890" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(customer["version"], 0);
    assert_eq!(customer["phone"], "081234567890");
    let customer_id = customer["id"].as_str().unwrap().to_string();

    // Rename.
    let (status, renamed) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/customers/{customer_id}"),
            json!({ "name": "Budi Santoso", "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "Budi Santoso");
    assert_eq!(renamed["version"], 1);

    // Ganti nomor telepon lewat endpoint terpisah.
    let (status, updated) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/customers/{customer_id}/phone"),
            json!({ "phone": "089999999999", "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["phone"], "089999999999");
    assert_eq!(updated["version"], 2);

    // Hapus nomor telepon (kirim phone: null).
    let (status, cleared) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/customers/{customer_id}/phone"),
            json!({ "phone": null, "expected_version": 2 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cleared["phone"], Value::Null);

    // Hapus customer.
    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/customers/{customer_id}"),
            json!({ "expected_version": 3 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_customer_allows_missing_phone() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (status, customer) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(customer["phone"], Value::Null);
}

#[tokio::test]
async fn create_customer_rejects_empty_name() {
    let app = app();
    let business_id = setup_business(&app).await;

    let (status, err) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "   " }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("kosong"));
}

#[tokio::test]
async fn create_customer_with_same_id_is_idempotent() {
    let app = app();
    let business_id = setup_business(&app).await;
    let customer_id = domain::CustomerId::new().to_string();

    let (status, first) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "id": customer_id, "name": "Budi" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry dengan Id sama, nama beda — harus 200 OK, mengembalikan
    // customer PERTAMA, bukan dibuat duplikat.
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "id": customer_id, "name": "Budi Lain" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["name"], first["name"]);
}

#[tokio::test]
async fn list_customers_only_returns_changes_after_cursor() {
    let app = app();
    let business_id = setup_business(&app).await;

    send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi Lama" }),
        ),
    )
    .await;

    let cursor_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let (_, baru) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi Baru" }),
        ),
    )
    .await;

    let (status, changed) = send(
        &app,
        get_request(&format!(
            "/businesses/{business_id}/customers?updated_since={cursor_time}"
        )),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let changed = changed.as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["id"], baru["id"]);
}

#[tokio::test]
async fn create_customer_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{random_id}/customers"),
            json!({ "name": "Budi" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_customer_rejects_when_business_is_deleted() {
    let app = app();
    let business_id = setup_business(&app).await;

    // Business belum punya customer aktif, jadi boleh langsung dihapus.
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

    let (status, err) = send(
        &app,
        json_request(
            "POST",
            &format!("/businesses/{business_id}/customers"),
            json!({ "name": "Budi" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        err["error"]
            .as_str()
            .unwrap()
            .contains("business sudah dihapus")
    );
}

#[tokio::test]
async fn rename_customer_rejects_stale_version() {
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

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/customers/{customer_id}"),
            json!({ "name": "Budi Baru", "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn list_customers_scoped_to_business_and_includes_soft_deleted() {
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

    // Tanpa cursor -> full sync.
    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/customers")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Hapus, lalu sync lagi -> harus tetap muncul dengan is_deleted=true,
    // supaya client offline tahu harus menghapus salinan lokalnya.
    send(
        &app,
        json_request(
            "DELETE",
            &format!("/customers/{customer_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    let (status, list) = send(
        &app,
        get_request(&format!("/businesses/{business_id}/customers")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["is_deleted"], true);
}

#[tokio::test]
async fn list_customers_returns_404_when_business_not_found() {
    let app = app();
    let random_id = domain::BusinessId::new().to_string();

    let (status, _) = send(
        &app,
        get_request(&format!("/businesses/{random_id}/customers")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
