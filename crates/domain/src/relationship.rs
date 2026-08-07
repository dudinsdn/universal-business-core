//! Entity Relationship: hubungan peer-to-peer antar dua Customer dalam
//! Business yang sama.
//!
//! Cakupan SENGAJA dibuat sempit di tahap ini (keputusan Din): hanya
//! Customer <-> Customer, dalam satu Business yang sama, searah
//! (`from_customer_id` -> `to_customer_id`). TIDAK mencakup Business <->
//! Business, lintas-Business, atau tipe entity lain — itu bisa
//! diperluas nanti kalau memang ada kebutuhan capability yang konkret
//! (menghindari generalisasi dini sebelum ada bukti nyata perlu).
//!
//! Beda dari hierarki kepemilikan yang sudah ada (Tenant -> Business,
//! Business -> Customer/Transaction, yang dimodelkan lewat foreign key
//! langsung), Relationship khusus untuk hubungan yang punya makna
//! tersendiri di ANTARA dua entity setara — mis. "A adalah referral dari
//! B", "A dan B bersaudara".
//!
//! Sama seperti Transaction, Relationship TIDAK bisa diubah pihaknya
//! setelah dibuat (tidak ada `rename`/`update`) — hanya `soft_delete`.

use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::business::BusinessId;
use crate::customer::CustomerId;
use crate::error::DomainError;

const MAX_TYPE_LENGTH: usize = 64;

/// Identitas unik Relationship. Selalu berupa UUID v7 — pola sama seperti
/// `TransactionId`/`CustomerId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelationshipId(Uuid);

impl RelationshipId {
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

impl Default for RelationshipId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RelationshipId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RelationshipId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidId)
    }
}

/// Jenis hubungan. Sengaja berupa string terbuka (pola sama seperti
/// `TransactionKind`/`BusinessType`), BUKAN enum tertutup — supaya
/// capability bisa mendefinisikan jenisnya sendiri (mis. "sibling",
/// "referral", "guardian") tanpa perlu mengubah Core Domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelationshipType(String);

impl RelationshipType {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let normalized = raw.into().trim().to_lowercase();
        if normalized.is_empty() {
            return Err(DomainError::EmptyRelationshipType);
        }
        if normalized.chars().count() > MAX_TYPE_LENGTH {
            return Err(DomainError::RelationshipTypeTooLong {
                max: MAX_TYPE_LENGTH,
            });
        }
        let is_valid = normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !is_valid {
            return Err(DomainError::InvalidRelationshipType);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Data mentah untuk merekonstruksi Relationship dari penyimpanan. Sama
/// alasannya dengan `PersistedTransaction`/`PersistedCustomer`.
pub struct PersistedRelationship {
    pub id: RelationshipId,
    pub business_id: BusinessId,
    pub from_customer_id: CustomerId,
    pub to_customer_id: CustomerId,
    pub relationship_type: RelationshipType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Entity Relationship: hubungan searah antara dua Customer dalam satu
/// Business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    id: RelationshipId,
    business_id: BusinessId,
    from_customer_id: CustomerId,
    to_customer_id: CustomerId,
    relationship_type: RelationshipType,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl Relationship {
    /// Membuat Relationship baru, dengan Id yang di-generate otomatis oleh
    /// sistem.
    ///
    /// Mengembalikan `Result` (beda dari `Transaction::new` yang langsung
    /// `Self`) karena ada satu invarian yang HANYA bisa dicek dengan
    /// membandingkan dua field entity ini sendiri (bukan lewat Value
    /// Object terpisah): `from_customer_id` tidak boleh sama dengan
    /// `to_customer_id` — Customer tidak bisa berelasi dengan dirinya
    /// sendiri.
    ///
    /// PENTING: pengecekan lain (apakah kedua Customer benar-benar milik
    /// Business yang sama, apakah Business masih aktif, apakah relationship
    /// dengan pasangan+jenis yang sama sudah ada) TIDAK dilakukan di sini —
    /// itu business rule lintas-aggregate, jadi tanggung jawab Application
    /// Service (belum diimplementasikan di tahap domain ini).
    pub fn new(
        business_id: BusinessId,
        from_customer_id: CustomerId,
        to_customer_id: CustomerId,
        relationship_type: RelationshipType,
    ) -> Result<Self, DomainError> {
        Self::with_id(
            RelationshipId::new(),
            business_id,
            from_customer_id,
            to_customer_id,
            relationship_type,
        )
    }

    /// Membuat Relationship baru dengan Id yang SUDAH ditentukan
    /// (idempotent create) — pola sama seperti `Transaction::with_id`.
    pub fn with_id(
        id: RelationshipId,
        business_id: BusinessId,
        from_customer_id: CustomerId,
        to_customer_id: CustomerId,
        relationship_type: RelationshipType,
    ) -> Result<Self, DomainError> {
        if from_customer_id == to_customer_id {
            return Err(DomainError::SelfRelationship);
        }

        let now = Utc::now();
        Ok(Self {
            id,
            business_id,
            from_customer_id,
            to_customer_id,
            relationship_type,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        })
    }

    pub fn id(&self) -> RelationshipId {
        self.id
    }

    /// Merekonstruksi Relationship dari data yang SUDAH tersimpan. Dipakai
    /// HANYA oleh implementasi Repository konkret — TIDAK mengulang
    /// validasi `SelfRelationship` (data yang sudah tersimpan dianggap
    /// pernah valid saat pertama kali dibuat).
    pub fn from_persisted(data: PersistedRelationship) -> Self {
        Self {
            id: data.id,
            business_id: data.business_id,
            from_customer_id: data.from_customer_id,
            to_customer_id: data.to_customer_id,
            relationship_type: data.relationship_type,
            created_at: data.created_at,
            updated_at: data.updated_at,
            deleted_at: data.deleted_at,
            version: data.version,
        }
    }

    pub fn business_id(&self) -> BusinessId {
        self.business_id
    }

    pub fn from_customer_id(&self) -> CustomerId {
        self.from_customer_id
    }

    pub fn to_customer_id(&self) -> CustomerId {
        self.to_customer_id
    }

    pub fn relationship_type(&self) -> &RelationshipType {
        &self.relationship_type
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

    fn sample_type() -> RelationshipType {
        RelationshipType::new("referral").unwrap()
    }

    #[test]
    fn relationship_id_roundtrips_through_uuid() {
        let id = RelationshipId::new();
        let rebuilt = RelationshipId::from_uuid(id.as_uuid());
        assert_eq!(id, rebuilt);
    }

    #[test]
    fn relationship_id_roundtrips_through_string() {
        let id = RelationshipId::new();
        let parsed: RelationshipId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn relationship_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<RelationshipId>(),
            Err(DomainError::InvalidId)
        );
    }

    #[test]
    fn relationship_type_normalizes_to_lowercase_and_trims() {
        let rt = RelationshipType::new("  Referral  ").unwrap();
        assert_eq!(rt.as_str(), "referral");
    }

    #[test]
    fn relationship_type_rejects_empty() {
        assert_eq!(
            RelationshipType::new("   "),
            Err(DomainError::EmptyRelationshipType)
        );
    }

    #[test]
    fn relationship_type_rejects_invalid_characters() {
        assert_eq!(
            RelationshipType::new("referral!"),
            Err(DomainError::InvalidRelationshipType)
        );
        assert_eq!(
            RelationshipType::new("family member"),
            Err(DomainError::InvalidRelationshipType)
        );
    }

    #[test]
    fn relationship_type_allows_underscore_and_hyphen() {
        assert!(RelationshipType::new("referred_by").is_ok());
        assert!(RelationshipType::new("referred-by").is_ok());
    }

    #[test]
    fn new_relationship_is_linked_to_given_business_and_customers() {
        let business_id = sample_business_id();
        let from = CustomerId::new();
        let to = CustomerId::new();

        let relationship = Relationship::new(business_id, from, to, sample_type()).unwrap();

        assert_eq!(relationship.business_id(), business_id);
        assert_eq!(relationship.from_customer_id(), from);
        assert_eq!(relationship.to_customer_id(), to);
        assert_eq!(relationship.version(), 0);
        assert!(!relationship.is_deleted());
    }

    #[test]
    fn new_relationship_rejects_self_relationship() {
        let customer_id = CustomerId::new();

        let result = Relationship::new(
            sample_business_id(),
            customer_id,
            customer_id,
            sample_type(),
        );

        assert_eq!(result, Err(DomainError::SelfRelationship));
    }

    #[test]
    fn with_id_uses_the_given_id() {
        let id = RelationshipId::new();
        let relationship = Relationship::with_id(
            id,
            sample_business_id(),
            CustomerId::new(),
            CustomerId::new(),
            sample_type(),
        )
        .unwrap();
        assert_eq!(relationship.id(), id);
        assert_eq!(relationship.version(), 0);
    }

    #[test]
    fn with_id_also_rejects_self_relationship() {
        let customer_id = CustomerId::new();
        let result = Relationship::with_id(
            RelationshipId::new(),
            sample_business_id(),
            customer_id,
            customer_id,
            sample_type(),
        );
        assert_eq!(result, Err(DomainError::SelfRelationship));
    }

    #[test]
    fn soft_delete_marks_deleted_and_increments_version() {
        let mut relationship = Relationship::new(
            sample_business_id(),
            CustomerId::new(),
            CustomerId::new(),
            sample_type(),
        )
        .unwrap();
        relationship.soft_delete();
        assert!(relationship.is_deleted());
        assert_eq!(relationship.version(), 1);
    }

    #[test]
    fn from_persisted_reconstructs_exact_state() {
        let id = RelationshipId::new();
        let business_id = sample_business_id();
        let from = CustomerId::new();
        let to = CustomerId::new();
        let relationship_type = sample_type();
        let created_at = Utc::now();

        let relationship = Relationship::from_persisted(PersistedRelationship {
            id,
            business_id,
            from_customer_id: from,
            to_customer_id: to,
            relationship_type: relationship_type.clone(),
            created_at,
            updated_at: created_at,
            deleted_at: None,
            version: 3,
        });

        assert_eq!(relationship.id(), id);
        assert_eq!(relationship.business_id(), business_id);
        assert_eq!(relationship.from_customer_id(), from);
        assert_eq!(relationship.to_customer_id(), to);
        assert_eq!(relationship.relationship_type(), &relationship_type);
        assert_eq!(relationship.version(), 3);
    }
}
