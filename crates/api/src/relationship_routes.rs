use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use application::{
    BusinessRepository, CustomerRepository, RelationshipRepository, TenantRepository,
    TransactionRepository,
};
use domain::{BusinessId, CustomerId, RelationshipId, RelationshipType};

use crate::dto::{CreateRelationshipRequest, DeleteRequest, RelationshipResponse};
use crate::error::ApiError;
use crate::state::SharedState;

pub async fn create_relationship<TR, BR, CR, TxR, RR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR>>,
    Path(business_id): Path<String>,
    Json(payload): Json<CreateRelationshipRequest>,
) -> Result<(StatusCode, Json<RelationshipResponse>), ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
{
    let business_id: BusinessId = business_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let business = state.business_service.get_business(business_id).await?;

    // Idempotency: sama seperti create_transaction — lihat komentar di
    // sana.
    let id = match payload.id {
        Some(raw) => raw.parse().map_err(application::ApplicationError::from)?,
        None => RelationshipId::new(),
    };
    let from_customer_id: CustomerId = payload
        .from_customer_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let to_customer_id: CustomerId = payload
        .to_customer_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let relationship_type = RelationshipType::new(payload.relationship_type)
        .map_err(application::ApplicationError::from)?;

    let (relationship, created) = state
        .relationship_service
        .create_relationship(
            &business,
            id,
            from_customer_id,
            to_customer_id,
            relationship_type,
        )
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(RelationshipResponse::from(&relationship))))
}

pub async fn delete_relationship<TR, BR, CR, TxR, RR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR>>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
{
    let id: RelationshipId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .relationship_service
        .delete_relationship(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
