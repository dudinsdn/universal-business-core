use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use application::{BusinessRepository, TenantRepository};
use domain::{BusinessId, BusinessName, BusinessType, TenantId};

use crate::dto::{BusinessResponse, CreateBusinessRequest, DeleteRequest, RenameRequest};
use crate::error::ApiError;
use crate::state::AppState;

pub async fn create_business<TR, BR>(
    State(state): State<Arc<AppState<TR, BR>>>,
    Path(tenant_id): Path<String>,
    Json(payload): Json<CreateBusinessRequest>,
) -> Result<(StatusCode, Json<BusinessResponse>), ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
{
    let tenant_id: TenantId = tenant_id
        .parse()
        .map_err(application::ApplicationError::from)?;
    let tenant = state.tenant_service.get_tenant(tenant_id).await?;

    // Idempotency: sama seperti create_tenant — lihat komentar di sana.
    let id = match payload.id {
        Some(raw) => raw.parse().map_err(application::ApplicationError::from)?,
        None => BusinessId::new(),
    };
    let name = BusinessName::new(payload.name).map_err(application::ApplicationError::from)?;
    let business_type =
        BusinessType::new(payload.business_type).map_err(application::ApplicationError::from)?;

    let (business, created) = state
        .business_service
        .create_business(&tenant, id, name, business_type)
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(BusinessResponse::from(&business))))
}

pub async fn rename_business<TR, BR>(
    State(state): State<Arc<AppState<TR, BR>>>,
    Path(id): Path<String>,
    Json(payload): Json<RenameRequest>,
) -> Result<Json<BusinessResponse>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
{
    let id: BusinessId = id.parse().map_err(application::ApplicationError::from)?;
    let name = BusinessName::new(payload.name).map_err(application::ApplicationError::from)?;
    let business = state
        .business_service
        .rename_business(id, name, payload.expected_version)
        .await?;
    Ok(Json(BusinessResponse::from(&business)))
}

pub async fn delete_business<TR, BR>(
    State(state): State<Arc<AppState<TR, BR>>>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
{
    let id: BusinessId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .business_service
        .delete_business(id, payload.expected_version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
