use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use domain::{TenantId, TenantName};

use crate::dto::{CreateTenantRequest, DeleteRequest, RenameRequest, TenantResponse};
use crate::error::ApiError;
use crate::state::AppState;

pub async fn create_tenant(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<(StatusCode, Json<TenantResponse>), ApiError> {
    let name = TenantName::new(payload.name).map_err(application::ApplicationError::from)?;
    let tenant = state.tenant_service.create_tenant(name).await?;
    Ok((StatusCode::CREATED, Json(TenantResponse::from(&tenant))))
}

pub async fn get_tenant(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TenantResponse>, ApiError> {
    let id: TenantId = id.parse().map_err(application::ApplicationError::from)?;
    let tenant = state.tenant_service.get_tenant(id).await?;
    Ok(Json(TenantResponse::from(&tenant)))
}

pub async fn rename_tenant(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<RenameRequest>,
) -> Result<Json<TenantResponse>, ApiError> {
    let id: TenantId = id.parse().map_err(application::ApplicationError::from)?;
    let name = TenantName::new(payload.name).map_err(application::ApplicationError::from)?;
    let tenant = state
        .tenant_service
        .rename_tenant(id, name, payload.expected_version)
        .await?;
    Ok(Json(TenantResponse::from(&tenant)))
}

pub async fn delete_tenant(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(_payload): Json<DeleteRequest>,
) -> Result<StatusCode, ApiError> {
    // `expected_version` belum dipakai: `TenantService::delete_tenant` saat
    // ini memang belum mengecek versi (lihat catatan di tenant_service.rs).
    // Body tetap diminta di sini demi konsistensi bentuk request dengan
    // delete_business, dan supaya siap begitu pengecekan versi ditambahkan.
    let id: TenantId = id.parse().map_err(application::ApplicationError::from)?;
    state
        .tenant_service
        .delete_tenant(id, &state.business_repository)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
