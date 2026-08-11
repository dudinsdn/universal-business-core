use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use application::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    TenantRepository, TransactionRepository,
};
use domain::{BusinessId, CustomerId, CustomerName, CustomerPhone};

use crate::dto::{
    CreateCustomerRequest, CustomerResponse, DeleteRequest, RenameRequest,
    UpdateCustomerPhoneRequest,
};
use crate::error::ApiError;
use crate::state::SharedState;

pub async fn create_customer<TR, BR, CR, TxR, RR, IR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR>>,
    Path(business_id): Path<String>,
    Json(payload): Json<CreateCustomerRequest>,
) -> Result<(StatusCode, Json<CustomerResponse>), ApiError>
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

    // Idempotency: sama seperti create_business — lihat komentar di sana.
    let id = match payload.id {
        Some(raw) => raw.parse().map_err(application::ApplicationError::from)?,
        None => CustomerId::new(),
    };
    let name = CustomerName::new(payload.name).map_err(application::ApplicationError::from)?;
    let phone = payload
        .phone
        .map(CustomerPhone::new)
        .transpose()
        .map_err(application::ApplicationError::from)?;

    let (customer, created) = state
        .customer_service
        .create_customer(&business, id, name, phone)
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(CustomerResponse::from(&customer))))
}

pub async fn rename_customer<TR, BR, CR, TxR, RR, IR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR>>,
    Path(id): Path<String>,
    Json(payload): Json<RenameRequest>,
) -> Result<Json<CustomerResponse>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
{
    let id: CustomerId = id.parse().map_err(application::ApplicationError::from)?;
    let name = CustomerName::new(payload.name).map_err(application::ApplicationError::from)?;
    let customer = state
        .customer_service
        .rename_customer(id, name, payload.expected_version)
        .await?;
    Ok(Json(CustomerResponse::from(&customer)))
}

pub async fn update_customer_phone<TR, BR, CR, TxR, RR, IR>(
    State(state): State<SharedState<TR, BR, CR, TxR, RR, IR>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateCustomerPhoneRequest>,
) -> Result<Json<CustomerResponse>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
{
    let id: CustomerId = id.parse().map_err(application::ApplicationError::from)?;
    let phone = payload
        .phone
        .map(CustomerPhone::new)
        .transpose()
        .map_err(application::ApplicationError::from)?;
    let customer = state
        .customer_service
        .update_customer_phone(id, phone, payload.expected_version)
        .await?;
    Ok(Json(CustomerResponse::from(&customer)))
}

pub async fn delete_customer<TR, BR, CR, TxR, RR, IR>(
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
    let id: CustomerId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .customer_service
        .delete_customer(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
