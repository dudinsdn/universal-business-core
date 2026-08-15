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
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_of(err: ApplicationError) -> (StatusCode, String) {
        let response = ApiError::from(err).into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let message = body["error"].as_str().unwrap().to_string();
        (status, message)
    }

    // --- DomainError -> 400 Bad Request (pelanggaran validasi Value Object) ---

    #[tokio::test]
    async fn domain_empty_name_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::EmptyName)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("kosong"));
    }

    #[tokio::test]
    async fn domain_name_too_long_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::NameTooLong {
            max: 255,
        }))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("255"));
    }

    #[tokio::test]
    async fn domain_empty_business_type_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::EmptyBusinessType)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis usaha"));
    }

    #[tokio::test]
    async fn domain_invalid_business_type_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::InvalidBusinessType)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis usaha"));
    }

    #[tokio::test]
    async fn domain_invalid_id_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::InvalidId)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("id"));
    }

    #[tokio::test]
    async fn domain_invalid_timestamp_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::InvalidTimestamp)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("RFC 3339"));
    }

    #[tokio::test]
    async fn domain_empty_phone_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::EmptyPhone)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("telepon"));
    }

    #[tokio::test]
    async fn domain_phone_too_long_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::PhoneTooLong {
            max: 32,
        }))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("32"));
    }

    #[tokio::test]
    async fn domain_invalid_phone_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::InvalidPhone)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("telepon"));
    }

    #[tokio::test]
    async fn domain_empty_transaction_kind_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::EmptyTransactionKind)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis transaksi"));
    }

    #[tokio::test]
    async fn domain_transaction_kind_too_long_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::TransactionKindTooLong { max: 64 },
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("64"));
    }

    #[tokio::test]
    async fn domain_invalid_transaction_kind_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::InvalidTransactionKind,
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis transaksi"));
    }

    #[tokio::test]
    async fn domain_invalid_amount_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(DomainError::InvalidAmount)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("lebih besar dari nol"));
    }

    #[tokio::test]
    async fn domain_empty_relationship_type_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::EmptyRelationshipType)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis hubungan"));
    }

    #[tokio::test]
    async fn domain_relationship_type_too_long_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::RelationshipTypeTooLong { max: 64 },
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("64"));
    }

    #[tokio::test]
    async fn domain_invalid_relationship_type_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::InvalidRelationshipType,
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis hubungan"));
    }

    #[tokio::test]
    async fn domain_empty_interaction_type_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::EmptyInteractionType)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis interaksi"));
    }

    #[tokio::test]
    async fn domain_interaction_type_too_long_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::InteractionTypeTooLong { max: 64 },
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("64"));
    }

    #[tokio::test]
    async fn domain_invalid_interaction_type_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::InvalidInteractionType,
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("jenis interaksi"));
    }

    #[tokio::test]
    async fn domain_empty_interaction_note_maps_to_400() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::EmptyInteractionNote)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("catatan"));
    }

    #[tokio::test]
    async fn domain_interaction_note_too_long_maps_to_400() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::InteractionNoteTooLong { max: 500 },
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("500"));
    }

    // --- DomainError -> 409 Conflict (pelanggaran business rule / state) ---

    #[tokio::test]
    async fn domain_duplicate_business_name_maps_to_409() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::DuplicateBusinessName)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("nama business"));
    }

    #[tokio::test]
    async fn domain_tenant_has_active_business_maps_to_409() {
        let (status, msg) = response_of(ApplicationError::Domain(
            DomainError::TenantHasActiveBusiness,
        ))
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("business aktif"));
    }

    #[tokio::test]
    async fn domain_tenant_is_deleted_maps_to_409() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::TenantIsDeleted)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("tenant sudah dihapus"));
    }

    #[tokio::test]
    async fn domain_business_is_deleted_maps_to_409() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::BusinessIsDeleted)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("business sudah dihapus"));
    }

    #[tokio::test]
    async fn domain_self_relationship_maps_to_409() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::SelfRelationship)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("dirinya sendiri"));
    }

    #[tokio::test]
    async fn domain_version_conflict_maps_to_409() {
        let (status, msg) =
            response_of(ApplicationError::Domain(DomainError::VersionConflict)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("versi"));
    }

    // --- ApplicationError::*NotFound -> 404 Not Found ---

    #[tokio::test]
    async fn tenant_not_found_maps_to_404() {
        let (status, msg) = response_of(ApplicationError::TenantNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("tenant"));
    }

    #[tokio::test]
    async fn business_not_found_maps_to_404() {
        let (status, msg) = response_of(ApplicationError::BusinessNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("business"));
    }

    #[tokio::test]
    async fn customer_not_found_maps_to_404() {
        let (status, msg) = response_of(ApplicationError::CustomerNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("customer"));
    }

    #[tokio::test]
    async fn transaction_not_found_maps_to_404() {
        let (status, msg) = response_of(ApplicationError::TransactionNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("transaction"));
    }

    #[tokio::test]
    async fn relationship_not_found_maps_to_404() {
        let (status, msg) = response_of(ApplicationError::RelationshipNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("relationship"));
    }

    #[tokio::test]
    async fn interaction_not_found_maps_to_404() {
        let (status, msg) = response_of(ApplicationError::InteractionNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(msg.contains("interaction"));
    }

    // --- Repository (infrastruktur) -> 500 Internal Server Error ---

    #[tokio::test]
    async fn repository_unavailable_maps_to_500() {
        let (status, msg) = response_of(ApplicationError::Repository(
            application::RepositoryError::Unavailable("koneksi database putus".to_string()),
        ))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(msg.contains("koneksi database putus"));
    }
}
