use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use application::ApplicationError;
use domain::DomainError;

/// Pembungkus tipis di atas `ApplicationError` supaya bisa diubah jadi
/// HTTP response. Pemetaan status code:
/// - Domain: pelanggaran validasi -> 400, konflik/versi -> 409
/// - NotFound -> 404
/// - Repository (infrastruktur) -> 500
pub struct ApiError(ApplicationError);

impl From<ApplicationError> for ApiError {
    fn from(err: ApplicationError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();

        let status = match &self.0 {
            ApplicationError::Domain(domain_err) => match domain_err {
                DomainError::EmptyName
                | DomainError::NameTooLong { .. }
                | DomainError::EmptyBusinessType
                | DomainError::InvalidBusinessType
                | DomainError::InvalidId => StatusCode::BAD_REQUEST,
                DomainError::DuplicateBusinessName
                | DomainError::TenantHasActiveBusiness
                | DomainError::TenantIsDeleted
                | DomainError::VersionConflict => StatusCode::CONFLICT,
            },
            ApplicationError::TenantNotFound | ApplicationError::BusinessNotFound => {
                StatusCode::NOT_FOUND
            }
            ApplicationError::Repository(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
