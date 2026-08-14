//! Route HTTP Capability Workshop + fungsi perakit `Router` mandiri.
//!
//! `build_workshop_router` TIDAK di-`.with_state()` di sini — dikembalikan
//! sebagai `Router` polos supaya pemanggil (`main.rs`/test) bebas
//! `.merge()` dengan `Router` Core tanpa konflik tipe state antar
//! sub-router (masing-masing sudah `.with_state()` sendiri sebelum
//! di-merge, itulah kenapa signature di bawah menerima `WorkshopState`
//! LANGSUNG, bukan lewat parameter route builder Core).

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{patch, post};
use chrono::{DateTime, Utc};

use application::{BusinessRepository, CustomerRepository, TransactionRepository};
use domain::{BusinessId, CustomerId, DomainError, TransactionId};

use crate::dto::{
    CompleteServiceOrderRequest, CreateServiceOrderRequest, ServiceOrderActionRequest,
    ServiceOrderResponse, SyncQuery,
};
use crate::http_error::WorkshopApiError;
use crate::repository::ServiceOrderRepository;
use crate::service_order::{ServiceOrderDescription, ServiceOrderId};
use crate::state::{SharedWorkshopState, WorkshopState};

/// Parse query param `updated_since`. Duplikat kecil dari
/// `api::sync_routes::parse_updated_since` — satu fungsi pendek, tidak
/// sepadan dipakai ulang lintas crate (arah dependency-nya juga tidak
/// mengizinkan).
fn parse_updated_since(raw: Option<String>) -> Result<DateTime<Utc>, DomainError> {
    match raw {
        None => Ok(DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch selalu valid")),
        Some(raw) => DateTime::parse_from_rfc3339(&raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| DomainError::InvalidTimestamp),
    }
}

pub async fn create_service_order<BR, CR, TxR, SR>(
    State(state): State<SharedWorkshopState<BR, CR, TxR, SR>>,
    Path(business_id): Path<String>,
    Json(payload): Json<CreateServiceOrderRequest>,
) -> Result<(StatusCode, Json<ServiceOrderResponse>), WorkshopApiError>
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id.parse()?;
    let business = state.business_service.get_business(business_id).await?;

    let id: ServiceOrderId = match payload.id {
        Some(raw) => raw.parse()?,
        None => ServiceOrderId::new(),
    };
    let customer_id: CustomerId = payload.customer_id.parse()?;
    // Ambil Customer utuh — dibutuhkan ServiceOrderService untuk
    // memvalidasi Customer ini benar-benar milik Business yang sama
    // (lihat domain::rules::customer_belongs_to_business, gap #3).
    let customer = state.customer_service.get_customer(customer_id).await?;
    let description = ServiceOrderDescription::new(payload.description)?;

    let (order, created) = state
        .service_order_service
        .create_service_order(&business, id, &customer, description)
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(ServiceOrderResponse::from(&order))))
}

pub async fn list_service_orders_updated_since<BR, CR, TxR, SR>(
    State(state): State<SharedWorkshopState<BR, CR, TxR, SR>>,
    Path(business_id): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<ServiceOrderResponse>>, WorkshopApiError>
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id.parse()?;
    let business = state.business_service.get_business(business_id).await?;

    let since = parse_updated_since(query.updated_since)?;
    let orders = state
        .service_order_service
        .list_updated_since(business.id(), since)
        .await?;
    Ok(Json(
        orders.iter().map(ServiceOrderResponse::from).collect(),
    ))
}

pub async fn start_service_order<BR, CR, TxR, SR>(
    State(state): State<SharedWorkshopState<BR, CR, TxR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<ServiceOrderActionRequest>,
) -> Result<Json<ServiceOrderResponse>, WorkshopApiError>
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    let order = state
        .service_order_service
        .start_service_order(id, payload.expected_version)
        .await?;
    Ok(Json(ServiceOrderResponse::from(&order)))
}

pub async fn complete_service_order<BR, CR, TxR, SR>(
    State(state): State<SharedWorkshopState<BR, CR, TxR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<CompleteServiceOrderRequest>,
) -> Result<Json<ServiceOrderResponse>, WorkshopApiError>
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    let transaction_id: Option<TransactionId> =
        payload.transaction_id.map(|raw| raw.parse()).transpose()?;
    // Ambil Transaction utuh (bukan cuma parse Id) — dibutuhkan
    // ServiceOrderService untuk memvalidasi Transaction ini benar-benar
    // milik Business yang sama sebelum ditautkan (lihat
    // rules::transaction_belongs_to_business).
    let transaction = match transaction_id {
        Some(transaction_id) => Some(
            state
                .transaction_service
                .get_transaction(transaction_id)
                .await?,
        ),
        None => None,
    };
    let order = state
        .service_order_service
        .complete_service_order(id, payload.expected_version, transaction.as_ref())
        .await?;
    Ok(Json(ServiceOrderResponse::from(&order)))
}

pub async fn cancel_service_order<BR, CR, TxR, SR>(
    State(state): State<SharedWorkshopState<BR, CR, TxR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<ServiceOrderActionRequest>,
) -> Result<Json<ServiceOrderResponse>, WorkshopApiError>
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    let order = state
        .service_order_service
        .cancel_service_order(id, payload.expected_version)
        .await?;
    Ok(Json(ServiceOrderResponse::from(&order)))
}

pub async fn delete_service_order<BR, CR, TxR, SR>(
    State(state): State<SharedWorkshopState<BR, CR, TxR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<ServiceOrderActionRequest>,
) -> Result<StatusCode, WorkshopApiError>
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    state
        .service_order_service
        .delete_service_order(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Merakit seluruh route Capability Workshop jadi satu `Router` MANDIRI,
/// lengkap dengan `.with_state()`-nya sendiri. Pemanggil (`main.rs`/test)
/// tinggal `.merge()` hasilnya dengan `Router` Core — TIDAK ADA lagi
/// parameter generik gabungan Core+Workshop di titik mana pun.
pub fn build_workshop_router<BR, CR, TxR, SR>(state: WorkshopState<BR, CR, TxR, SR>) -> Router
where
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    Router::new()
        .route(
            "/businesses/{business_id}/service-orders",
            post(create_service_order::<BR, CR, TxR, SR>)
                .get(list_service_orders_updated_since::<BR, CR, TxR, SR>),
        )
        .route(
            "/service-orders/{id}/start",
            patch(start_service_order::<BR, CR, TxR, SR>),
        )
        .route(
            "/service-orders/{id}/complete",
            patch(complete_service_order::<BR, CR, TxR, SR>),
        )
        .route(
            "/service-orders/{id}/cancel",
            patch(cancel_service_order::<BR, CR, TxR, SR>),
        )
        .route(
            "/service-orders/{id}",
            axum::routing::delete(delete_service_order::<BR, CR, TxR, SR>),
        )
        .with_state(Arc::new(state))
}
