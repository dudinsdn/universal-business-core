//! DTO (request/response) untuk HTTP layer Capability Workshop.
//!
//! SENGAJA berada di crate ini (bukan di `api`) — konsekuensi dari
//! keputusan arsitektur: setiap Capability memiliki router HTTP-nya
//! sendiri, `api` crate tidak lagi "mengenal" ServiceOrder sama sekali.
//! Pola dan alasan tiap field sama persis seperti versi lamanya di
//! `api::dto` (lihat riwayat commit).

use serde::{Deserialize, Serialize};

use crate::service_order::ServiceOrder;

/// Body untuk `POST /businesses/{business_id}/service-orders`. `id`
/// opsional untuk idempotent create, `customer_id` wajib (ServiceOrder
/// selalu untuk satu Customer tertentu).
#[derive(Debug, Deserialize)]
pub struct CreateServiceOrderRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub customer_id: String,
    pub description: String,
}

/// Body untuk aksi status yang hanya butuh optimistic locking —
/// `PATCH /service-orders/{id}/start` dan `.../cancel`.
#[derive(Debug, Deserialize)]
pub struct ServiceOrderActionRequest {
    pub expected_version: u32,
}

/// Body untuk `PATCH /service-orders/{id}/complete`. `transaction_id`
/// opsional — link ke Transaction (Core) yang menagihnya.
#[derive(Debug, Deserialize)]
pub struct CompleteServiceOrderRequest {
    pub expected_version: u32,
    #[serde(default)]
    pub transaction_id: Option<String>,
}

/// Query param untuk endpoint incremental sync
/// (`GET /businesses/{business_id}/service-orders?updated_since=...`).
/// Duplikat kecil dari `api::dto::SyncQuery` — tidak bisa dipakai ulang
/// langsung karena `capability-workshop` tidak (dan tidak boleh) depend
/// ke crate `api` (arah dependency-nya terbalik: `api` yang depend ke
/// Capability, bukan sebaliknya).
#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    #[serde(default)]
    pub updated_since: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ServiceOrderResponse {
    pub id: String,
    pub business_id: String,
    pub customer_id: String,
    pub description: String,
    pub status: String,
    pub transaction_id: Option<String>,
    pub version: u32,
    pub is_deleted: bool,
}

impl From<&ServiceOrder> for ServiceOrderResponse {
    fn from(order: &ServiceOrder) -> Self {
        Self {
            id: order.id().to_string(),
            business_id: order.business_id().to_string(),
            customer_id: order.customer_id().to_string(),
            description: order.description().as_str().to_string(),
            status: order.status().as_str().to_string(),
            transaction_id: order.transaction_id().map(|t| t.to_string()),
            version: order.version(),
            is_deleted: order.is_deleted(),
        }
    }
}
