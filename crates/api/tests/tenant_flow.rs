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

#[tokio::test]
async fn full_flow_create_delete() {
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

    let (status, _) = send(&app, get_request(&format!("/tenants/{random_id}"))).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rename_tenant_updates_name_and_version() {
    let app = app();

    let (_, tenant) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    let tenant_id = tenant["id"].as_str().unwrap().to_string();
    assert_eq!(tenant["version"], 0);

    let (status, renamed) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/tenants/{tenant_id}"),
            json!({
                "name": "Tenant B",
                "expected_version": 0
            }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["id"], tenant["id"]);
    assert_eq!(renamed["name"], "Tenant B");
    assert_eq!(renamed["version"], 1);
    assert_eq!(renamed["is_deleted"], false);
}

#[tokio::test]
async fn rename_tenant_returns_404_when_not_found() {
    let app = app();
    let random_id = domain::TenantId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/tenants/{random_id}"),
            json!({
                "name": "Tenant Baru",
                "expected_version": 0
            }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rename_tenant_rejects_stale_version() {
    let app = app();

    let (_, tenant) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    let tenant_id = tenant["id"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &format!("/tenants/{tenant_id}"),
            json!({
                "name": "Tenant B",
                "expected_version": 1
            }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);

    // Tenant tetap menggunakan data dan version sebelumnya.
    let (status, fetched) = send(&app, get_request(&format!("/tenants/{tenant_id}"))).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["name"], "Tenant A");
    assert_eq!(fetched["version"], 0);
    assert_eq!(fetched["is_deleted"], false);
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
    let (status, fetched) = send(&app, get_request(&format!("/tenants/{tenant_id}"))).await;
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

#[tokio::test]
async fn delete_tenant_returns_404_when_not_found() {
    let app = app();
    let random_id = domain::TenantId::new().to_string();

    let (status, _) = send(
        &app,
        json_request(
            "DELETE",
            &format!("/tenants/{random_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_tenant_with_same_id_is_idempotent() {
    let app = app();
    let id = domain::TenantId::new().to_string();

    let (status, first) = send(
        &app,
        json_request("POST", "/tenants", json!({ "id": id, "name": "Tenant A" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry: Id sama, nama beda (mensimulasikan client kirim ulang request
    // setelah timeout). Harus 200 OK, bukan 201, dan data yang dikembalikan
    // adalah Tenant PERTAMA — bukan duplikat, bukan ketimpa nama baru.
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            "/tenants",
            json!({ "id": id, "name": "Tenant A Lain" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["name"], first["name"]);
    assert_eq!(second["version"], 0);
}

#[tokio::test]
async fn create_business_with_same_id_is_idempotent() {
    let app = app();

    let (_, tenant) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    let tenant_id = tenant["id"].as_str().unwrap().to_string();
    let business_id = domain::BusinessId::new().to_string();

    let (status, first) = send(
        &app,
        json_request(
            "POST",
            &format!("/tenants/{tenant_id}/businesses"),
            json!({ "id": business_id, "name": "Toko Baju", "business_type": "retail" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Retry dengan Id sama, nama beda — harus 200 OK dan mengembalikan
    // business PERTAMA, bukan ditolak sebagai "nama duplikat".
    let (status, second) = send(
        &app,
        json_request(
            "POST",
            &format!("/tenants/{tenant_id}/businesses"),
            json!({ "id": business_id, "name": "Toko Baju Lain", "business_type": "retail" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["name"], first["name"]);
}

#[tokio::test]
async fn list_tenants_without_cursor_returns_everything() {
    let app = app();

    send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant B" })),
    )
    .await;

    // Tanpa `updated_since` -> full sync, kembalikan semua Tenant.
    let (status, tenants) = send(&app, get_request("/tenants")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(tenants.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn list_tenants_only_returns_changes_after_cursor() {
    let app = app();

    send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant Lama" })),
    )
    .await;

    let (_, marker) = send(&app, get_request("/tenants")).await;
    let cursor = marker.as_array().unwrap()[0]["id"].clone();
    // Ambil timestamp saat ini lewat sisi client sebagai cursor: cukup
    // pakai waktu sekarang, karena Tenant "Baru" pasti dibuat setelah ini.
    let _ = cursor; // hanya memastikan response awal memang berisi data.

    let cursor_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant Baru" })),
    )
    .await;

    let (status, changed) = send(
        &app,
        get_request(&format!("/tenants?updated_since={cursor_time}")),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let changed = changed.as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["name"], "Tenant Baru");
}

#[tokio::test]
async fn list_tenants_rejects_invalid_updated_since() {
    let app = app();

    let (status, err) = send(&app, get_request("/tenants?updated_since=bukan-tanggal")).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("RFC 3339"));
}

#[tokio::test]
async fn list_businesses_scoped_to_tenant_and_includes_soft_deleted() {
    let app = app();

    let (_, tenant) = send(
        &app,
        json_request("POST", "/tenants", json!({ "name": "Tenant A" })),
    )
    .await;
    let tenant_id = tenant["id"].as_str().unwrap().to_string();

    let (_, business) = send(
        &app,
        json_request(
            "POST",
            &format!("/tenants/{tenant_id}/businesses"),
            json!({ "name": "Toko Baju", "business_type": "retail" }),
        ),
    )
    .await;
    let business_id = business["id"].as_str().unwrap().to_string();

    // Tanpa cursor -> full sync, kembalikan semua business tenant ini.
    let (status, list) = send(
        &app,
        get_request(&format!("/tenants/{tenant_id}/businesses")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Hapus business-nya, lalu sync lagi -> harus tetap muncul (soft
    // deleted), supaya client offline tahu harus menghapus salinan lokal.
    send(
        &app,
        json_request(
            "DELETE",
            &format!("/businesses/{business_id}"),
            json!({ "expected_version": 0 }),
        ),
    )
    .await;

    let (status, list) = send(
        &app,
        get_request(&format!("/tenants/{tenant_id}/businesses")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["is_deleted"], true);
}

#[tokio::test]
async fn list_businesses_returns_404_when_tenant_not_found() {
    let app = app();
    let random_id = domain::TenantId::new().to_string();

    let (status, _) = send(
        &app,
        get_request(&format!("/tenants/{random_id}/businesses")),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
