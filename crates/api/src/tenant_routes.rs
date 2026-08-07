use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use application::{BusinessRepository, CustomerRepository, TenantRepository};
use domain::{TenantId, TenantName};

use crate::dto::{CreateTenantRequest, DeleteRequest, RenameRequest, TenantResponse};
use crate::error::ApiError;
use crate::state::AppState;

pub async fn create_tenant<TR, BR, CR>(
    State(state): State<Arc<AppState<TR, BR, CR>>>,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<(StatusCode, Json<TenantResponse>), ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
{
    // Idempotency: kalau client kirim `id` sendiri, dipakai apa adanya
    // (retry request dengan `id` yang sama tidak akan membuat duplikat —
    // lihat TenantService::create_tenant). Kalau tidak dikirim, server
    // generate Id baru seperti sebelumnya.
    let id = match payload.id {
        Some(raw) => raw.parse().map_err(application::ApplicationError::from)?,
        None => TenantId::new(),
    };
    let name = TenantName::new(payload.name).map_err(application::ApplicationError::from)?;

    let (tenant, created) = state.tenant_service.create_tenant(id, name).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(TenantResponse::from(&tenant))))
}

pub async fn get_tenant<TR, BR, CR>(
    State(state): State<Arc<AppState<TR, BR, CR>>>,
    Path(id): Path<String>,
) -> Result<Json<TenantResponse>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
{
    let id: TenantId = id.parse().map_err(application::ApplicationError::from)?;
    let tenant = state.tenant_service.get_tenant(id).await?;
    Ok(Json(TenantResponse::from(&tenant)))
}

pub async fn rename_tenant<TR, BR, CR>(
    State(state): State<Arc<AppState<TR, BR, CR>>>,
    Path(id): Path<String>,
    Json(payload): Json<RenameRequest>,
) -> Result<Json<TenantResponse>, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
{
    let id: TenantId = id.parse().map_err(application::ApplicationError::from)?;
    let name = TenantName::new(payload.name).map_err(application::ApplicationError::from)?;
    let tenant = state
        .tenant_service
        .rename_tenant(id, name, payload.expected_version)
        .await?;
    Ok(Json(TenantResponse::from(&tenant)))
}

pub async fn delete_tenant<TR, BR, CR>(
    State(state): State<Arc<AppState<TR, BR, CR>>>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError>
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
{
    let id: TenantId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .tenant_service
        .delete_tenant(id, payload.expected_version, &state.business_repository)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
