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

#[tokio::test]
async fn full_flow_create_rename_delete() {
    let app = app();

    // Buat tenant.
    let (status, tenant) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let tenant_id = tenant["id"].as_str().unwrap().to_string();
    assert_eq!(tenant["version"], 0);

    // Buat business di bawah tenant tsb.
    let (status, business) = send(
        &app,
        json_request(
            "POST",
            &format!("/tenants/{tenant_id}/businesses"),
            json!({ "name": "Toko Baju", "business_type": "retail" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let business_id = business["id"].as_str().unwrap().to_string();

    // Nama business duplikat pada tenant yang sama harus ditolak (409).
    let (status, err) = send(
        &app,
        json_request(
            "POST",
            &format!("/tenants/{tenant_id}/businesses"),
            json!({ "name": "Toko Baju", "business_type": "retail" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(err["error"].as_str().unwrap().contains("nama business"));

    // Tenant tidak bisa dihapus selagi masih ada business aktif.
    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/tenants/{tenant_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Hapus business-nya dulu.
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

    // Barulah tenant bisa dihapus.
    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/tenants/{tenant_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn get_tenant_returns_404_when_not_found() {
    let app = app();
    let random_id = domain::TenantId::new().to_string();

    let (status, _) = send(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/tenants/{random_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_tenant_rejects_stale_version() {
    let app = app();

    let (_, tenant) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    let tenant_id = tenant["id"].as_str().unwrap().to_string();

    // expected_version salah (tenant masih di versi 0) harus ditolak 409,
    // bukan diam-diam berhasil menghapus.
    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/tenants/{tenant_id}"),
            json!({ "expected_version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Tenant tetap ada dan belum terhapus.
    let (status, fetched) = send(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/tenants/{tenant_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["is_deleted"], false);
}

#[tokio::test]
async fn create_tenant_rejects_empty_name() {
    let app = app();

    let (status, err) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "   " })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("kosong"));
}
