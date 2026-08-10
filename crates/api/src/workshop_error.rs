use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use application::ApplicationError;
use capability_workshop::{ServiceOrderError, WorkshopError};
use domain::DomainError;

use crate::error::ApiError;

/// Pembungkus error untuk route Capability Workshop. Route-nya sering
/// perlu mengambil `Business` dari Core dulu (bisa gagal dengan
/// `ApplicationError`/`DomainError`) SEBELUM memanggil `ServiceOrderService`
/// (bisa gagal dengan `ServiceOrderError`) — jadi satu tipe error di sini
/// perlu bisa menampung keduanya.
///
/// SENGAJA tipe terpisah dari `ApiError`, bukan menambah varian baru ke
/// `ApiError` — supaya `ApiError` (murni Core) tidak perlu "mengenal"
/// `ServiceOrderError` (Workshop). Untuk error yang berasal dari Core,
/// pemetaan status HTTP-nya didelegasikan ke `ApiError` yang sudah ada
/// (bukan diduplikasi).
pub enum WorkshopApiError {
    Core(ApplicationError),
    Workshop(ServiceOrderError),
}

impl From<ApplicationError> for WorkshopApiError {
    fn from(err: ApplicationError) -> Self {
        WorkshopApiError::Core(err)
    }
}

/// `BusinessId`/`CustomerId`/`TransactionId` (dari Core) di-parse lewat
/// `FromStr` yang errornya `DomainError`, bukan `ApplicationError` —
/// dipetakan sebagai kesalahan validasi Core (400 lewat `ApiError`),
/// konsisten dengan bagaimana route Core lain menangani id tidak valid.
impl From<DomainError> for WorkshopApiError {
    fn from(err: DomainError) -> Self {
        WorkshopApiError::Core(ApplicationError::Domain(err))
    }
}

impl From<ServiceOrderError> for WorkshopApiError {
    fn from(err: ServiceOrderError) -> Self {
        WorkshopApiError::Workshop(err)
    }
}

/// `ServiceOrderId::from_str`, `ServiceOrderDescription::new`, dan
/// transisi status (`start`/`complete`/`cancel`) mengembalikan
/// `WorkshopError` langsung (bukan `ServiceOrderError`) — konversi ini
/// menghindari route harus membungkusnya manual satu-satu.
impl From<WorkshopError> for WorkshopApiError {
    fn from(err: WorkshopError) -> Self {
        WorkshopApiError::Workshop(ServiceOrderError::from(err))
    }
}

impl IntoResponse for WorkshopApiError {
    fn into_response(self) -> Response {
        match self {
            WorkshopApiError::Core(err) => ApiError::from(err).into_response(),
            WorkshopApiError::Workshop(err) => {
                let message = err.to_string();
                let status = match &err {
                    ServiceOrderError::Workshop(workshop_err) => match workshop_err {
                        WorkshopError::EmptyDescription
                        | WorkshopError::DescriptionTooLong { .. }
                        | WorkshopError::InvalidId => StatusCode::BAD_REQUEST,
                        WorkshopError::InvalidTransition { .. }
                        | WorkshopError::BusinessIsDeleted
                        | WorkshopError::VersionConflict => StatusCode::CONFLICT,
                        // Seharusnya tidak pernah sampai di sini secara
                        // praktik — PgServiceOrderRepository sudah
                        // membungkusnya jadi RepositoryError::Unavailable
                        // sebelum sempat jadi WorkshopError di level ini.
                        // Dipetakan 500 karena berarti data di database
                        // rusak/di luar kendali aplikasi.
                        WorkshopError::UnknownStatus { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                    },
                    ServiceOrderError::ServiceOrderNotFound => StatusCode::NOT_FOUND,
                    ServiceOrderError::Repository(_) => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, Json(json!({ "error": message }))).into_response()
            }
        }
    }
}
