use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use domain::{BusinessId, BusinessName, BusinessType, TenantId};

use crate::dto::{BusinessResponse, CreateBusinessRequest, DeleteRequest, RenameRequest};
use crate::error::ApiError;
use crate::state::AppState;

pub async fn create_business(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
    Json(payload): Json<CreateBusinessRequest>,
) -> Result<(StatusCode, Json<BusinessResponse>), ApiError> {
    let tenant_id: TenantId = tenant_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let tenant = state.tenant_service.get_tenant(tenant_id)?;

    let name = BusinessName::new(payload.name).map_err(application::ApplicationError::from)?;
    let business_type =
        BusinessType::new(payload.business_type).map_err(application::ApplicationError::from)?;

    let business = state
        .business_service
        .create_business(&tenant, name, business_type)?;
    Ok((StatusCode::CREATED, Json(BusinessResponse::from(&business))))
}

pub async fn rename_business(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<RenameRequest>,
) -> Result<Json<BusinessResponse>, ApiError> {
    let id: BusinessId = id.parse().map_err(application::ApplicationError::from)?;
    let name = BusinessName::new(payload.name).map_err(application::ApplicationError::from)?;
    let business = state
        .business_service
        .rename_business(id, name, payload.expected_version)?;
    Ok(Json(BusinessResponse::from(&business)))
}

pub async fn delete_business(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError> {
    let id: BusinessId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .business_service
        .delete_business(id, payload.expected_version)?;
    Ok(StatusCode::NO_CONTENT)
}
