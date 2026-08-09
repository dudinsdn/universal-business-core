//! Entity ServiceOrder: satu pekerjaan servis/perbaikan milik satu
//! Business, untuk satu Customer.
//!
//! Ini entity PERTAMA Capability Workshop — sengaja dibuat sesempit
//! mungkin (keputusan Din): tanpa `Vehicle`, tanpa `Technician`, tanpa
//! rincian biaya per item. Kendaraan/keluhan dicatat sebagai teks bebas
//! (`ServiceOrderDescription`), biaya sepenuhnya didelegasikan ke
//! `domain::Transaction` (Core) lewat referensi opsional `transaction_id`.
//!
//! Beda mendasar dari entity Core (`Business`, `Customer`, dst.):
//! ServiceOrder punya SIKLUS HIDUP (status yang berubah seiring waktu),
//! bukan cuma "ada lalu di-soft-delete". `status` dan `deleted_at` adalah
//! dua sumbu yang terpisah: `status: Cancelled` berarti "batal secara
//! bisnis" (mis. pelanggan berubah pikiran), sedangkan `soft_delete()`
//! berarti "salah input, dihapus dari catatan aktif" — pola yang sama
//! seperti Core Domain lainnya.

use std::fmt;

use chrono::{DateTime, Utc};
use domain::{BusinessId, CustomerId, TransactionId};
use uuid::Uuid;

use crate::error::WorkshopError;

const MAX_DESCRIPTION_LENGTH: usize = 1000;

/// Identitas unik ServiceOrder. Selalu berupa UUID v7 — pola sama persis
/// seperti Id di Core Domain (`BusinessId`, `TransactionId`, dst).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceOrderId(Uuid);

impl ServiceOrderId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ServiceOrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ServiceOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ServiceOrderId {
    type Err = WorkshopError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| WorkshopError::InvalidId)
    }
}

/// Deskripsi pekerjaan/keluhan/kendaraan yang diservis. Teks bebas —
/// TIDAK ada entity `Vehicle` terpisah di tahap ini (keputusan Din).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceOrderDescription(String);

impl ServiceOrderDescription {
    pub fn new(raw: impl Into<String>) -> Result<Self, WorkshopError> {
        let trimmed = raw.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(WorkshopError::EmptyDescription);
        }
        if trimmed.chars().count() > MAX_DESCRIPTION_LENGTH {
            return Err(WorkshopError::DescriptionTooLong {
                max: MAX_DESCRIPTION_LENGTH,
            });
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Status siklus hidup ServiceOrder. SENGAJA berupa enum tertutup — beda
/// dari `BusinessType`/`TransactionKind` (string terbuka) — karena
/// transisi antar-statusnya dijaga oleh business rule di sini, bukan
/// bebas ditentukan Capability lain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceOrderStatus {
    Received,
    InProgress,
    Completed,
    Cancelled,
}

impl ServiceOrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceOrderStatus::Received => "received",
            ServiceOrderStatus::InProgress => "in_progress",
            ServiceOrderStatus::Completed => "completed",
            ServiceOrderStatus::Cancelled => "cancelled",
        }
    }
}

/// Data mentah untuk merekonstruksi ServiceOrder dari penyimpanan. Sama
/// alasannya dengan `PersistedBusiness`/`PersistedTransaction` di Core:
/// menghindari constructor dengan terlalu banyak parameter.
pub struct PersistedServiceOrder {
    pub id: ServiceOrderId,
    pub business_id: BusinessId,
    pub customer_id: CustomerId,
    pub description: ServiceOrderDescription,
    pub status: ServiceOrderStatus,
    pub transaction_id: Option<TransactionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Entity ServiceOrder: satu pekerjaan servis milik satu Business, untuk
/// satu Customer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceOrder {
    id: ServiceOrderId,
    business_id: BusinessId,
    customer_id: CustomerId,
    description: ServiceOrderDescription,
    status: ServiceOrderStatus,
    transaction_id: Option<TransactionId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl ServiceOrder {
    /// Membuat ServiceOrder baru, dengan Id yang di-generate otomatis oleh
    /// sistem. Status awal SELALU `Received`, `transaction_id` SELALU
    /// `None` — servis baru dibuat belum mungkin sudah selesai/ditagih.
    ///
    /// PENTING: pengecekan "apakah Customer ini benar-benar milik
    /// Business yang sama" TIDAK dilakukan di sini — itu business rule
    /// lintas-aggregate, tanggung jawab Application Service (belum
    /// diimplementasikan di tahap domain ini, sesuai keputusan Din untuk
    /// didiskusikan lagi nanti).
    pub fn new(
        business_id: BusinessId,
        customer_id: CustomerId,
        description: ServiceOrderDescription,
    ) -> Self {
        Self::with_id(ServiceOrderId::new(), business_id, customer_id, description)
    }

    /// Membuat ServiceOrder baru dengan Id yang SUDAH ditentukan
    /// (idempotent create) — pola sama seperti entity Core lainnya.
    pub fn with_id(
        id: ServiceOrderId,
        business_id: BusinessId,
        customer_id: CustomerId,
        description: ServiceOrderDescription,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            business_id,
            customer_id,
            description,
            status: ServiceOrderStatus::Received,
            transaction_id: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        }
    }

    pub fn id(&self) -> ServiceOrderId {
        self.id
    }

    /// Merekonstruksi ServiceOrder dari data yang SUDAH tersimpan. Dipakai
    /// HANYA oleh implementasi Repository konkret nanti.
    pub fn from_persisted(data: PersistedServiceOrder) -> Self {
        Self {
            id: data.id,
            business_id: data.business_id,
            customer_id: data.customer_id,
            description: data.description,
            status: data.status,
            transaction_id: data.transaction_id,
            created_at: data.created_at,
            updated_at: data.updated_at,
            deleted_at: data.deleted_at,
            version: data.version,
        }
    }

    pub fn business_id(&self) -> BusinessId {
        self.business_id
    }

    pub fn customer_id(&self) -> CustomerId {
        self.customer_id
    }

    pub fn description(&self) -> &ServiceOrderDescription {
        &self.description
    }

    pub fn status(&self) -> ServiceOrderStatus {
        self.status
    }

    pub fn transaction_id(&self) -> Option<TransactionId> {
        self.transaction_id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Received -> InProgress. Pekerjaan mulai dikerjakan.
    pub fn start(&mut self) -> Result<(), WorkshopError> {
        match self.status {
            ServiceOrderStatus::Received => {
                self.status = ServiceOrderStatus::InProgress;
                self.touch();
                Ok(())
            }
            other => Err(WorkshopError::InvalidTransition {
                from: other.as_str(),
                to: ServiceOrderStatus::InProgress.as_str(),
            }),
        }
    }

    /// InProgress -> Completed. HANYA boleh dari `InProgress` — bukan
    /// langsung dari `Received` (keputusan Din: setiap servis dianggap
    /// selalu melalui fase "sedang dikerjakan").
    ///
    /// `transaction_id` opsional: diisi kalau penagihan (Transaction Core)
    /// sudah dibuat bersamaan, atau dibiarkan `None` dan ditautkan
    /// belakangan lewat mekanisme lain (belum diimplementasikan di tahap
    /// domain ini).
    pub fn complete(&mut self, transaction_id: Option<TransactionId>) -> Result<(), WorkshopError> {
        match self.status {
            ServiceOrderStatus::InProgress => {
                self.status = ServiceOrderStatus::Completed;
                self.transaction_id = transaction_id;
                self.touch();
                Ok(())
            }
            other => Err(WorkshopError::InvalidTransition {
                from: other.as_str(),
                to: ServiceOrderStatus::Completed.as_str(),
            }),
        }
    }

    /// Received/InProgress -> Cancelled. Tidak bisa dibatalkan kalau
    /// sudah `Completed`.
    pub fn cancel(&mut self) -> Result<(), WorkshopError> {
        match self.status {
            ServiceOrderStatus::Received | ServiceOrderStatus::InProgress => {
                self.status = ServiceOrderStatus::Cancelled;
                self.touch();
                Ok(())
            }
            other => Err(WorkshopError::InvalidTransition {
                from: other.as_str(),
                to: ServiceOrderStatus::Cancelled.as_str(),
            }),
        }
    }

    /// Soft delete — untuk "salah input", BUKAN pengganti `cancel()`.
    /// Sumbu terpisah dari `status`, pola sama seperti Core Domain.
    pub fn soft_delete(&mut self) {
        self.deleted_at = Some(Utc::now());
        self.touch();
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now();
        self.version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_business_id() -> BusinessId {
        BusinessId::new()
    }

    fn sample_customer_id() -> CustomerId {
        CustomerId::new()
    }

    fn sample_description() -> ServiceOrderDescription {
        ServiceOrderDescription::new("Ganti oli dan servis rem").unwrap()
    }

    #[test]
    fn service_order_id_roundtrips_through_uuid() {
        let id = ServiceOrderId::new();
        let rebuilt = ServiceOrderId::from_uuid(id.as_uuid());
        assert_eq!(id, rebuilt);
    }

    #[test]
    fn service_order_id_roundtrips_through_string() {
        let id = ServiceOrderId::new();
        let parsed: ServiceOrderId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn service_order_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<ServiceOrderId>(),
            Err(WorkshopError::InvalidId)
        );
    }

    #[test]
    fn description_rejects_empty_string() {
        assert_eq!(
            ServiceOrderDescription::new(""),
            Err(WorkshopError::EmptyDescription)
        );
        assert_eq!(
            ServiceOrderDescription::new("   "),
            Err(WorkshopError::EmptyDescription)
        );
    }

    #[test]
    fn description_trims_whitespace() {
        let description = ServiceOrderDescription::new("  Ganti oli  ").unwrap();
        assert_eq!(description.as_str(), "Ganti oli");
    }

    #[test]
    fn description_rejects_too_long() {
        let long_description = "a".repeat(MAX_DESCRIPTION_LENGTH + 1);
        assert_eq!(
            ServiceOrderDescription::new(long_description),
            Err(WorkshopError::DescriptionTooLong {
                max: MAX_DESCRIPTION_LENGTH
            })
        );
    }

    #[test]
    fn new_service_order_starts_as_received_with_no_transaction() {
        let business_id = sample_business_id();
        let customer_id = sample_customer_id();
        let order = ServiceOrder::new(business_id, customer_id, sample_description());

        assert_eq!(order.business_id(), business_id);
        assert_eq!(order.customer_id(), customer_id);
        assert_eq!(order.status(), ServiceOrderStatus::Received);
        assert!(order.transaction_id().is_none());
        assert_eq!(order.version(), 0);
        assert!(!order.is_deleted());
    }

    #[test]
    fn with_id_uses_the_given_id() {
        let id = ServiceOrderId::new();
        let order = ServiceOrder::with_id(
            id,
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );
        assert_eq!(order.id(), id);
        assert_eq!(order.version(), 0);
    }

    #[test]
    fn happy_path_transitions_received_to_in_progress_to_completed() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );

        order.start().unwrap();
        assert_eq!(order.status(), ServiceOrderStatus::InProgress);
        assert_eq!(order.version(), 1);

        let transaction_id = TransactionId::new();
        order.complete(Some(transaction_id)).unwrap();
        assert_eq!(order.status(), ServiceOrderStatus::Completed);
        assert_eq!(order.transaction_id(), Some(transaction_id));
        assert_eq!(order.version(), 2);
    }

    #[test]
    fn complete_rejects_directly_from_received() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );

        let result = order.complete(None);

        assert_eq!(
            result,
            Err(WorkshopError::InvalidTransition {
                from: "received",
                to: "completed",
            })
        );
        assert_eq!(order.status(), ServiceOrderStatus::Received);
    }

    #[test]
    fn cancel_allowed_from_received() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );

        order.cancel().unwrap();
        assert_eq!(order.status(), ServiceOrderStatus::Cancelled);
    }

    #[test]
    fn cancel_allowed_from_in_progress() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );

        order.start().unwrap();
        order.cancel().unwrap();
        assert_eq!(order.status(), ServiceOrderStatus::Cancelled);
    }

    #[test]
    fn cancel_rejects_when_already_completed() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );
        order.start().unwrap();
        order.complete(None).unwrap();

        let result = order.cancel();

        assert_eq!(
            result,
            Err(WorkshopError::InvalidTransition {
                from: "completed",
                to: "cancelled",
            })
        );
    }

    #[test]
    fn start_rejects_when_not_received() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );
        order.start().unwrap();

        let result = order.start();

        assert_eq!(
            result,
            Err(WorkshopError::InvalidTransition {
                from: "in_progress",
                to: "in_progress",
            })
        );
    }

    #[test]
    fn soft_delete_marks_deleted_and_increments_version_independently_of_status() {
        let mut order = ServiceOrder::new(
            sample_business_id(),
            sample_customer_id(),
            sample_description(),
        );
        order.soft_delete();

        assert!(order.is_deleted());
        assert_eq!(order.status(), ServiceOrderStatus::Received);
        assert_eq!(order.version(), 1);
    }

    #[test]
    fn from_persisted_reconstructs_exact_state() {
        let id = ServiceOrderId::new();
        let business_id = sample_business_id();
        let customer_id = sample_customer_id();
        let description = sample_description();
        let transaction_id = TransactionId::new();
        let created_at = Utc::now();

        let order = ServiceOrder::from_persisted(PersistedServiceOrder {
            id,
            business_id,
            customer_id,
            description: description.clone(),
            status: ServiceOrderStatus::Completed,
            transaction_id: Some(transaction_id),
            created_at,
            updated_at: created_at,
            deleted_at: None,
            version: 2,
        });

        assert_eq!(order.id(), id);
        assert_eq!(order.business_id(), business_id);
        assert_eq!(order.customer_id(), customer_id);
        assert_eq!(order.description(), &description);
        assert_eq!(order.status(), ServiceOrderStatus::Completed);
        assert_eq!(order.transaction_id(), Some(transaction_id));
        assert_eq!(order.version(), 2);
    }
}
