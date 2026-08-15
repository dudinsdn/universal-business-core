//! Pembungkus error HTTP untuk route Capability Workshop.
//!
//! SENGAJA berada di crate ini, bukan lagi di `api` — konsekuensi
//! langsung dari desain "Capability = router mandiri". Efeknya: mapping
//! `ApplicationError` (Core) -> HTTP status DIDUPLIKASI dari
//! `api::error::ApiError`, karena arah dependency crate tidak
//! mengizinkan `capability-workshop` memakai tipe dari `api` (justru
//! `api` yang depend ke Capability, bukan sebaliknya).
//!
//! PENTING — konsekuensi duplikasi ini WAJIB diingat: setiap kali ada
//! varian baru di `domain::DomainError` atau `application::ApplicationError`,
//! DUA match harus diperbarui bersamaan: `api::error::ApiError` (untuk
//! route Core) DAN match `DomainError`/`ApplicationError` di bawah ini
//! (untuk route Capability manapun yang memanggil Core, termasuk
//! Workshop). Alternatif "satu crate error bersama" sengaja tidak
//! dipilih di tahap ini — baru sepadan kalau capability ketiga/keempat
//! menunjukkan pola yang sama persis berulang (lihat prinsip refactor:
//! hanya kalau ada duplikasi nyata DAN manfaat jelas).
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use application::ApplicationError;
use domain::DomainError;

use crate::error::{ServiceOrderError, WorkshopError};

/// Route Workshop sering perlu mengambil `Business` dari Core dulu (bisa
/// gagal dengan `ApplicationError`/`DomainError`) SEBELUM memanggil
/// `ServiceOrderService` (bisa gagal dengan `ServiceOrderError`) — jadi
/// satu tipe error di sini perlu bisa menampung keduanya.
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
/// dipetakan sebagai kesalahan validasi Core (400).
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

/// Pemetaan `ApplicationError` (Core) -> HTTP status. DUPLIKAT dari
/// `api::error::ApiError` — lihat catatan panjang di atas modul ini
/// untuk alasannya.
fn core_error_response(err: &ApplicationError) -> (StatusCode, String) {
    let message = err.to_string();
    let status = match err {
        ApplicationError::Domain(domain_err) => match domain_err {
            DomainError::EmptyName
            | DomainError::NameTooLong { .. }
            | DomainError::EmptyBusinessType
            | DomainError::InvalidBusinessType
            | DomainError::InvalidId
            | DomainError::InvalidTimestamp
            | DomainError::EmptyPhone
            | DomainError::PhoneTooLong { .. }
            | DomainError::InvalidPhone
            | DomainError::EmptyTransactionKind
            | DomainError::TransactionKindTooLong { .. }
            | DomainError::InvalidTransactionKind
            | DomainError::InvalidAmount
            | DomainError::EmptyRelationshipType
            | DomainError::RelationshipTypeTooLong { .. }
            | DomainError::InvalidRelationshipType
            | DomainError::EmptyInteractionType
            | DomainError::InteractionTypeTooLong { .. }
            | DomainError::InvalidInteractionType
            | DomainError::EmptyInteractionNote
            | DomainError::InteractionNoteTooLong { .. } => StatusCode::BAD_REQUEST,
            DomainError::DuplicateBusinessName
            | DomainError::TenantHasActiveBusiness
            | DomainError::TenantIsDeleted
            | DomainError::BusinessIsDeleted
            | DomainError::SelfRelationship
            | DomainError::VersionConflict => StatusCode::CONFLICT,
        },
        ApplicationError::TenantNotFound
        | ApplicationError::BusinessNotFound
        | ApplicationError::CustomerNotFound
        | ApplicationError::TransactionNotFound
        | ApplicationError::RelationshipNotFound
        | ApplicationError::InteractionNotFound => StatusCode::NOT_FOUND,
        ApplicationError::Repository(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, message)
}

impl IntoResponse for WorkshopApiError {
    fn into_response(self) -> Response {
        match self {
            WorkshopApiError::Core(err) => {
                let (status, message) = core_error_response(&err);
                (status, Json(json!({ "error": message }))).into_response()
            }
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
                        WorkshopError::UnknownStatus { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                        // Sama seperti ApplicationError::CustomerNotFound
                        // di Core: 404, bukan 409 — lihat komentar di
                        // WorkshopError::CustomerNotFound.
                        WorkshopError::CustomerNotFound => StatusCode::NOT_FOUND,
                        // Pola identik dengan CustomerNotFound di atas —
                        // lihat komentar di WorkshopError::TransactionNotFound.
                        WorkshopError::TransactionNotFound => StatusCode::NOT_FOUND,
                    },
                    ServiceOrderError::ServiceOrderNotFound => StatusCode::NOT_FOUND,
                    ServiceOrderError::Repository(_) => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, Json(json!({ "error": message }))).into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_of(err: WorkshopApiError) -> (StatusCode, String) {
        let response = err.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let message = body["error"].as_str().unwrap().to_string();
        (status, message)
    }

    // --- Core (ApplicationError) — representatif per kategori status.
    // Cakupan penuh 27 varian sudah ada di api::error::ApiError; di sini
    // cukup membuktikan match Core di WorkshopApiError tidak melenceng
    // (lihat catatan drift-risk di kepala modul ini).

    #[tokio::test]
    async fn core_domain_validation_error_maps_to_400() {
        let (status, msg) = response_of(WorkshopApiError::Core(ApplicationError::Domain(
            DomainError::EmptyName,
        )))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("kosong"));
    }

    #[tokio::test]
    async fn core_domain_conflict_error_maps_to_409() {
        let (status, msg) = response_of(WorkshopApiError::Core(ApplicationError::Domain(
            DomainError::VersionConflict,
        )))
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("versi"));
    }

    #[tokio::test]
    async fn core_business_not_found_maps_to_404() {
        let (status, msg) =
            response_of(WorkshopApiError::Core(ApplicationError::BusinessNotFound)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("business"));
    }

    #[tokio::test]
    async fn core_repository_error_maps_to_500() {
        let (status, msg) = response_of(WorkshopApiError::Core(ApplicationError::Repository(
            application::RepositoryError::Unavailable("koneksi database putus".to_string()),
        )))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(msg.contains("koneksi database putus"));
    }

    #[tokio::test]
    async fn core_domain_error_from_str_parsing_maps_to_400() {
        // Jalur khusus: BusinessId::from_str dll mengembalikan DomainError
        // langsung (bukan ApplicationError) -- lihat impl From<DomainError>.
        let (status, msg) = response_of(WorkshopApiError::from(DomainError::InvalidId)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("id"));
    }

    // --- Workshop (WorkshopError via ServiceOrderError) — cakupan penuh,
    // ini bagian yang benar-benar baru di WorkshopApiError.

    #[tokio::test]
    async fn workshop_empty_description_maps_to_400() {
        let (status, msg) =
            response_of(WorkshopApiError::from(WorkshopError::EmptyDescription)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("deskripsi"));
    }

    #[tokio::test]
    async fn workshop_description_too_long_maps_to_400() {
        let (status, msg) =
            response_of(WorkshopApiError::from(WorkshopError::DescriptionTooLong {
                max: 1000,
            }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("1000"));
    }

    #[tokio::test]
    async fn workshop_invalid_id_maps_to_400() {
        let (status, msg) = response_of(WorkshopApiError::from(WorkshopError::InvalidId)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("id"));
    }

    #[tokio::test]
    async fn workshop_invalid_transition_maps_to_409() {
        let (status, msg) = response_of(WorkshopApiError::from(WorkshopError::InvalidTransition {
            from: "received",
            to: "completed",
        }))
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("received"));
        assert!(msg.contains("completed"));
    }

    #[tokio::test]
    async fn workshop_business_is_deleted_maps_to_409() {
        let (status, msg) =
            response_of(WorkshopApiError::from(WorkshopError::BusinessIsDeleted)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("business sudah dihapus"));
    }

    #[tokio::test]
    async fn workshop_version_conflict_maps_to_409() {
        let (status, msg) =
            response_of(WorkshopApiError::from(WorkshopError::VersionConflict)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("versi"));
    }

    #[tokio::test]
    async fn workshop_unknown_status_maps_to_500() {
        // Seharusnya tidak pernah terjadi lewat jalur normal -- lihat
        // komentar di WorkshopError::UnknownStatus -- tapi mapping-nya
        // tetap harus benar (500, bukan diam-diam 200).
        let (status, msg) = response_of(WorkshopApiError::from(WorkshopError::UnknownStatus {
            value: "bukan-status-valid".to_string(),
        }))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(msg.contains("bukan-status-valid"));
    }

    #[tokio::test]
    async fn workshop_customer_not_found_maps_to_404_not_409() {
        // Info-hiding: dipetakan sebagai "tidak ditemukan" (404), BUKAN
        // "konflik" (409) -- lihat komentar di WorkshopError::CustomerNotFound.
        let (status, msg) =
            response_of(WorkshopApiError::from(WorkshopError::CustomerNotFound)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("customer"));
    }

    #[tokio::test]
    async fn workshop_transaction_not_found_maps_to_404_not_409() {
        // Pola identik dengan CustomerNotFound -- lihat komentar di
        // WorkshopError::TransactionNotFound.
        let (status, msg) =
            response_of(WorkshopApiError::from(WorkshopError::TransactionNotFound)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("transaction"));
    }

    #[tokio::test]
    async fn service_order_not_found_maps_to_404() {
        let (status, msg) = response_of(WorkshopApiError::Workshop(
            ServiceOrderError::ServiceOrderNotFound,
        ))
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("service order"));
    }

    #[tokio::test]
    async fn service_order_repository_error_maps_to_500() {
        let (status, msg) = response_of(WorkshopApiError::Workshop(ServiceOrderError::Repository(
            crate::error::RepositoryError::Unavailable("koneksi database putus".to_string()),
        )))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(msg.contains("koneksi database putus"));
    }
}
