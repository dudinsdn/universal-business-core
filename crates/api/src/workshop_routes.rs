use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};

use application::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    TenantRepository, TransactionRepository,
};
use capability_workshop::{ServiceOrderDescription, ServiceOrderId, ServiceOrderRepository};
use domain::{BusinessId, CustomerId, DomainError, TransactionId};

use crate::dto::{
    CompleteServiceOrderRequest, CreateServiceOrderRequest, ServiceOrderActionRequest,
    ServiceOrderResponse, SyncQuery,
};
use crate::state::SharedState;
use crate::workshop_error::WorkshopApiError;

/// Parse query param `updated_since`. Duplikat kecil dari
/// `sync_routes::parse_updated_since` (private di modul itu) — sama
/// alasannya dengan duplikasi kecil di `capability-workshop::rules`:
/// satu fungsi pendek, tidak sepadan untuk diekspos lintas modul demi
/// dipakai ulang satu kali.
fn parse_updated_since(raw: Option<String>) -> Result<DateTime<Utc>, DomainError> {
    match raw {
        None => Ok(DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch selalu valid")),
        Some(raw) => DateTime::parse_from_rfc3339(&raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| DomainError::InvalidTimestamp),
    }
}

pub async fn create_service_order<TR, BR, CR, TxR, RR, IR, SR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR, SR>>,
    Path(business_id): Path<String>,
    Json(payload): Json<CreateServiceOrderRequest>,
) -> Result<(StatusCode, Json<ServiceOrderResponse>), WorkshopApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id.parse()?;
    let business = state.business_service.get_business(business_id).await?;

    // Idempotency: pola sama seperti create_interaction di Core.
    let id: ServiceOrderId = match payload.id {
        Some(raw) => raw.parse()?,
        None => ServiceOrderId::new(),
    };
    let customer_id: CustomerId = payload.customer_id.parse()?;
    let description = ServiceOrderDescription::new(payload.description)?;

    let (order, created) = state
        .service_order_service
        .create_service_order(&business, id, customer_id, description)
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(ServiceOrderResponse::from(&order))))
}

/// `GET /businesses/{business_id}/service-orders?updated_since=<RFC3339>`
/// — endpoint incremental sync, pola sama persis seperti
/// `sync_routes::list_customers_updated_since`.
pub async fn list_service_orders_updated_since<TR, BR, CR, TxR, RR, IR, SR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR, SR>>,
    Path(business_id): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<ServiceOrderResponse>>, WorkshopApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id.parse()?;
    // Pastikan Business-nya ada dulu (404 kalau tidak) — konsisten
    // dengan endpoint sync Core lainnya.
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

pub async fn start_service_order<TR, BR, CR, TxR, RR, IR, SR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<ServiceOrderActionRequest>,
) -> Result<Json<ServiceOrderResponse>, WorkshopApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    let order = state
        .service_order_service
        .start_service_order(id, payload.expected_version)
        .await?;
    Ok(Json(ServiceOrderResponse::from(&order)))
}

pub async fn complete_service_order<TR, BR, CR, TxR, RR, IR, SR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<CompleteServiceOrderRequest>,
) -> Result<Json<ServiceOrderResponse>, WorkshopApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    let transaction_id: Option<TransactionId> =
        payload.transaction_id.map(|raw| raw.parse()).transpose()?;
    let order = state
        .service_order_service
        .complete_service_order(id, payload.expected_version, transaction_id)
        .await?;
    Ok(Json(ServiceOrderResponse::from(&order)))
}

pub async fn cancel_service_order<TR, BR, CR, TxR, RR, IR, SR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<ServiceOrderActionRequest>,
) -> Result<Json<ServiceOrderResponse>, WorkshopApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    let order = state
        .service_order_service
        .cancel_service_order(id, payload.expected_version)
        .await?;
    Ok(Json(ServiceOrderResponse::from(&order)))
}

pub async fn delete_service_order<TR, BR, CR, TxR, RR, IR, SR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR, SR>>,
    Path(id): Path<String>,
    Json(payload): Json<ServiceOrderActionRequest>,
) -> Result<StatusCode, WorkshopApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    let id: ServiceOrderId = id.parse()?;
    state
        .service_order_service
        .delete_service_order(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
