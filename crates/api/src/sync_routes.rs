use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};

use application::{BusinessRepository, TenantRepository};
use domain::{DomainError, TenantId};

use crate::dto::{BusinessResponse, SyncQuery, TenantResponse};
use crate::error::ApiError;
use crate::state::AppState;

/// Parse query param `updated_since` (RFC 3339, mis. "2026-08-01T00:00:00Z").
/// Kosong berarti "sejak awal waktu" — dipakai client untuk full sync
/// pertama kali (belum pernah sinkron sebelumnya).
fn parse_updated_since(raw: Option<String>) -> Result<DateTime<Utc>, DomainError> {
    match raw {
        None => Ok(DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch selalu valid")),
        Some(raw) => DateTime::parse_from_rfc3339(&raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| DomainError::InvalidTimestamp),
    }
}

/// `GET /tenants?updated_since=<RFC3339>` — endpoint incremental sync.
///
/// Mengembalikan semua Tenant yang berubah (dibuat/diubah/dihapus) sejak
/// waktu tersebut, TERMASUK yang sudah di-soft-delete, supaya client
/// offline tahu harus menghapus salinan lokalnya juga — bukan cuma
/// menerima entity yang masih aktif. Tanpa parameter, mengembalikan semua
/// Tenant (dipakai untuk full sync pertama kali).
///
/// Client menyimpan `updated_at` terbesar dari response sebagai cursor
/// untuk request `updated_since` berikutnya.
pub async fn list_tenants_updated_since<TR, BR>(
    State(state): State<Arc<AppState<TR, BR>>>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<TenantResponse>>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
{
    let since =
        parse_updated_since(query.updated_since).map_err(application::ApplicationError::from)?;
    let tenants = state.tenant_service.list_updated_since(since).await?;
    Ok(Json(tenants.iter().map(TenantResponse::from).collect()))
}

/// `GET /tenants/{tenant_id}/businesses?updated_since=<RFC3339>` — sama
/// seperti `list_tenants_updated_since`, tapi untuk Business di bawah satu
/// Tenant tertentu (konsisten dengan resource path create business).
pub async fn list_businesses_updated_since<TR, BR>(
    State(state): State<Arc<AppState<TR, BR>>>,
    Path(tenant_id): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<BusinessResponse>>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
{
    let tenant_id: TenantId = tenant_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    // Pastikan Tenant-nya ada dulu (404 kalau tidak) — konsisten dengan
    // create_business, bukan diam-diam kembalikan list kosong untuk
    // tenant_id yang salah/typo.
    let tenant = state.tenant_service.get_tenant(tenant_id).await?;

    let since =
        parse_updated_since(query.updated_since).map_err(application::ApplicationError::from)?;
    let businesses = state
        .business_service
        .list_updated_since(tenant.id(), since)
        .await?;
    Ok(Json(
        businesses.iter().map(BusinessResponse::from).collect(),
    ))
}
