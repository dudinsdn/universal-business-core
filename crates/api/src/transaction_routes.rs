use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};

use application::{
    BusinessRepository, CustomerRepository, TenantRepository, TransactionRepository,
};
use domain::{
    BusinessId, CustomerId, DomainError, TransactionAmount, TransactionId, TransactionKind,
};

use crate::dto::{CreateTransactionRequest, DeleteRequest, TransactionResponse};
use crate::error::ApiError;
use crate::state::AppState;

/// Parse `occurred_at` (RFC 3339). Beda dari `parse_updated_since` di
/// `sync_routes` — default-nya waktu SEKARANG (bukan epoch), karena kalau
/// client tidak mengirim `occurred_at`, transaksi dianggap terjadi saat
/// request ini diterima server.
fn parse_occurred_at(raw: Option<String>) -> Result<DateTime<Utc>, DomainError> {
    match raw {
        None => Ok(Utc::now()),
        Some(raw) => DateTime::parse_from_rfc3339(&raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| DomainError::InvalidTimestamp),
    }
}

pub async fn create_transaction<TR, BR, CR, TxR>(
    State(state): State<Arc<AppState<TR, BR, CR, TxR>>>,
    Path(business_id): Path<String>,
    Json(payload): Json<CreateTransactionRequest>,
) -> Result<(StatusCode, Json<TransactionResponse>), ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let business = state.business_service.get_business(business_id).await?;

    // Idempotency: sama seperti create_business/create_customer — lihat
    // komentar di sana.
    let id = match payload.id {
        Some(raw) => raw.parse().map_err(application::ApplicationError::from)?,
        None => TransactionId::new(),
    };
    let customer_id: Option<CustomerId> = payload
        .customer_id
        .map(|raw| raw.parse())
        .transpose()
        .map_err(application::ApplicationError::from)?;
    let kind = TransactionKind::new(payload.kind).map_err(application::ApplicationError::from)?;
    let amount =
        TransactionAmount::new(payload.amount).map_err(application::ApplicationError::from)?;
    let occurred_at =
        parse_occurred_at(payload.occurred_at).map_err(application::ApplicationError::from)?;

    let (transaction, created) = state
        .transaction_service
        .create_transaction(&business, id, customer_id, kind, amount, occurred_at)
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(TransactionResponse::from(&transaction))))
}

pub async fn delete_transaction<TR, BR, CR, TxR>(
    State(state): State<Arc<AppState<TR, BR, CR, TxR>>>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
{
    let id: TransactionId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .transaction_service
        .delete_transaction(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
