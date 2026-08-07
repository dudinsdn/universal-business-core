use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};

use application::{
    BusinessRepository, CustomerRepository, TenantRepository, TransactionRepository,
};
use domain::{BusinessId, DomainError, TenantId};

use crate::dto::{
    BusinessResponse, CustomerResponse, SyncQuery, TenantResponse, TransactionResponse,
};
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
pub async fn list_tenants_updated_since<TR, BR, CR, TxR>(
    State(state): State<Arc<AppState<TR, BR, CR, TxR>>>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<TenantResponse>>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
{
    let since =
        parse_updated_since(query.updated_since).map_err(application::ApplicationError::from)?;
    let tenants = state.tenant_service.list_updated_since(since).await?;
    Ok(Json(tenants.iter().map(TenantResponse::from).collect()))
}

/// `GET /tenants/{tenant_id}/businesses?updated_since=<RFC3339>` — sama
/// seperti `list_tenants_updated_since`, tapi untuk Business di bawah satu
/// Tenant tertentu (konsisten dengan resource path create business).
pub async fn list_businesses_updated_since<TR, BR, CR, TxR>(
    State(state): State<Arc<AppState<TR, BR, CR, TxR>>>,
    Path(tenant_id): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<BusinessResponse>>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
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

/// `GET /businesses/{business_id}/customers?updated_since=<RFC3339>` —
/// sama seperti `list_businesses_updated_since`, tapi untuk Customer di
/// bawah satu Business tertentu (Customer bernaung di bawah Business,
/// bukan langsung di bawah Tenant — lihat `domain::Customer`).
pub async fn list_customers_updated_since<TR, BR, CR, TxR>(
    State(state): State<Arc<AppState<TR, BR, CR, TxR>>>,
    Path(business_id): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<CustomerResponse>>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    // Pastikan Business-nya ada dulu (404 kalau tidak) — konsisten dengan
    // create_customer dan list_businesses_updated_since.
    let business = state.business_service.get_business(business_id).await?;

    let since =
        parse_updated_since(query.updated_since).map_err(application::ApplicationError::from)?;
    let customers = state
        .customer_service
        .list_updated_since(business.id(), since)
        .await?;
    Ok(Json(customers.iter().map(CustomerResponse::from).collect()))
}

/// `GET /businesses/{business_id}/transactions?updated_since=<RFC3339>` —
/// sama seperti `list_customers_updated_since`, tapi untuk Transaction di
/// bawah satu Business tertentu (Transaction bernaung di bawah Business,
/// bukan langsung di bawah Tenant — lihat `domain::Transaction`).
pub async fn list_transactions_updated_since<TR, BR, CR, TxR>(
    State(state): State<Arc<AppState<TR, BR, CR, TxR>>>,
    Path(business_id): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<TransactionResponse>>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    // Pastikan Business-nya ada dulu (404 kalau tidak) — konsisten dengan
    // create_transaction dan list_customers_updated_since.
    let business = state.business_service.get_business(business_id).await?;

    let since =
        parse_updated_since(query.updated_since).map_err(application::ApplicationError::from)?;
    let transactions = state
        .transaction_service
        .list_updated_since(business.id(), since)
        .await?;
    Ok(Json(
        transactions.iter().map(TransactionResponse::from).collect(),
    ))
}
