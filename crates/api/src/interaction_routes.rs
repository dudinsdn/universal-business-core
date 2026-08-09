use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};

use application::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    TenantRepository, TransactionRepository,
};
use domain::{
    BusinessId, CustomerId, DomainError, InteractionId, InteractionNote, InteractionType,
};

use crate::dto::{CreateInteractionRequest, DeleteRequest, InteractionResponse};
use crate::error::ApiError;
use crate::state::SharedState;

/// Parse `occurred_at` (RFC 3339). Sama seperti
/// `transaction_routes::parse_occurred_at` — default-nya waktu SEKARANG
/// (bukan epoch), karena kalau client tidak mengirim `occurred_at`,
/// kontak dianggap terjadi saat request ini diterima server.
fn parse_occurred_at(raw: Option<String>) -> Result<DateTime<Utc>, DomainError> {
    match raw {
        None => Ok(Utc::now()),
        Some(raw) => DateTime::parse_from_rfc3339(&raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| DomainError::InvalidTimestamp),
    }
}

pub async fn create_interaction<TR, BR, CR, TxR, RR, IR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR>>,
    Path(business_id): Path<String>,
    Json(payload): Json<CreateInteractionRequest>,
) -> Result<(StatusCode, Json<InteractionResponse>), ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let business = state.business_service.get_business(business_id).await?;

    // Idempotency: sama seperti create_relationship — lihat komentar di
    // sana.
    let id = match payload.id {
        Some(raw) => raw.parse().map_err(application::ApplicationError::from)?,
        None => InteractionId::new(),
    };
    let customer_id: CustomerId = payload
        .customer_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let interaction_type = InteractionType::new(payload.interaction_type)
        .map_err(application::ApplicationError::from)?;
    let note = payload
        .note
        .map(InteractionNote::new)
        .transpose()
        .map_err(application::ApplicationError::from)?;
    let occurred_at =
        parse_occurred_at(payload.occurred_at).map_err(application::ApplicationError::from)?;

    let (interaction, created) = state
        .interaction_service
        .create_interaction(
            &business,
            id,
            customer_id,
            interaction_type,
            note,
            occurred_at,
        )
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(InteractionResponse::from(&interaction))))
}

pub async fn delete_interaction<TR, BR, CR, TxR, RR, IR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR>>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
{
    let id: InteractionId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .interaction_service
        .delete_interaction(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
