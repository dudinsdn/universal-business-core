//! Entity Transaction: satu kejadian bernilai uang yang terjadi pada
//! sebuah Business.
//!
//! Sengaja TIDAK menyimpan line-item (mis. "beli baju 2pcs") — itu
//! representasi spesifik capability (Retail dll), bukan Core Domain.
//! Core hanya butuh: nilai total, jenis transaksi, kapan terjadi, dan
//! (opsional) siapa customer-nya.
//!
//! Transaction TIDAK bisa diubah nilainya setelah dibuat (tidak ada
//! `rename`/`update_amount` seperti pada Business/Customer) — sekali
//! tercatat, sebuah transaksi finansial adalah fakta historis. Koreksi
//! dilakukan lewat transaksi baru (mis. `transaction_type = "refund"`),
//! bukan mengubah transaksi yang sudah ada. Satu-satunya mutasi yang
//! diizinkan adalah `soft_delete` (mis. transaksi salah input dan perlu
//! dibatalkan dari catatan aktif).

use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::business::BusinessId;
use crate::customer::CustomerId;
use crate::error::DomainError;

const MAX_KIND_LENGTH: usize = 64;

/// Identitas unik Transaction. Selalu berupa UUID v7 — pola sama seperti
/// `BusinessId`/`CustomerId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionId(Uuid);

impl TransactionId {
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

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TransactionId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidId)
    }
}

/// Jenis transaksi. Sengaja berupa string terbuka (pola sama seperti
/// `BusinessType`), BUKAN enum tertutup — supaya capability bisa
/// mendefinisikan jenisnya sendiri (mis. "sale", "refund", "payment",
/// "adjustment") tanpa perlu mengubah Core Domain.
///
/// Refund/pembatalan dimodelkan sebagai kind tersendiri dengan amount
/// tetap positif (bukan amount negatif pada kind yang sama) — membuat
/// agregasi dan pelaporan lebih konsisten (lihat keputusan Din).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransactionKind(String);

impl TransactionKind {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let normalized = raw.into().trim().to_lowercase();
        if normalized.is_empty() {
            return Err(DomainError::EmptyTransactionKind);
        }
        if normalized.chars().count() > MAX_KIND_LENGTH {
            return Err(DomainError::TransactionKindTooLong {
                max: MAX_KIND_LENGTH,
            });
        }
        let is_valid = normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !is_valid {
            return Err(DomainError::InvalidTransactionKind);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Nilai transaksi dalam satuan terkecil mata uang (mis. sen, atau rupiah
/// kalau tidak ada pecahan) — BUKAN `Money` dengan currency. `Money`
/// sengaja belum dibuat karena belum ada kebutuhan nyata multi-currency;
/// bisa diperkenalkan nanti kalau memang diperlukan (keputusan Din).
///
/// SELALU positif (> 0) — transaksi dengan nilai nol atau negatif tidak
/// masuk akal sebagai kejadian finansial. Ini bukan tempatnya validasi
/// "refund harus lebih kecil dari transaksi asal" dsb. — itu business rule
/// lintas-transaksi, akan dipertimbangkan saat memang dibutuhkan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionAmount(i64);

impl TransactionAmount {
    pub fn new(raw: i64) -> Result<Self, DomainError> {
        if raw <= 0 {
            return Err(DomainError::InvalidAmount);
        }
        Ok(Self(raw))
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

/// Data mentah untuk merekonstruksi Transaction dari penyimpanan. Sama
/// alasannya dengan `PersistedBusiness`/`PersistedCustomer`.
pub struct PersistedTransaction {
    pub id: TransactionId,
    pub business_id: BusinessId,
    pub customer_id: Option<CustomerId>,
    pub kind: TransactionKind,
    pub amount: TransactionAmount,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Entity Transaction: satu kejadian bernilai uang milik satu Business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    id: TransactionId,
    business_id: BusinessId,
    customer_id: Option<CustomerId>,
    kind: TransactionKind,
    amount: TransactionAmount,
    occurred_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl Transaction {
    /// Membuat Transaction baru di bawah satu Business, dengan Id yang
    /// di-generate otomatis oleh sistem.
    ///
    /// PENTING: pengecekan "apakah Business masih aktif" TIDAK dilakukan
    /// di sini — sama seperti `Customer::new`. Panggil
    /// `rules::ensure_business_is_active` di Application Service sebelum
    /// memanggil constructor ini (rule ini dipakai ulang dari Customer,
    /// karena aggregate boundary-nya sama: Business -> Transaction).
    ///
    /// `occurred_at` diterima sebagai parameter (bukan selalu `Utc::now()`)
    /// karena transaksi offline bisa dicatat belakangan tapi terjadi di
    /// waktu lampau — beda dari `created_at` yang selalu waktu pencatatan.
    pub fn new(
        business_id: BusinessId,
        customer_id: Option<CustomerId>,
        kind: TransactionKind,
        amount: TransactionAmount,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self::with_id(
            TransactionId::new(),
            business_id,
            customer_id,
            kind,
            amount,
            occurred_at,
        )
    }

    /// Membuat Transaction baru dengan Id yang SUDAH ditentukan (idempotent
    /// create) — pola sama seperti `Business::with_id`/`Customer::with_id`.
    pub fn with_id(
        id: TransactionId,
        business_id: BusinessId,
        customer_id: Option<CustomerId>,
        kind: TransactionKind,
        amount: TransactionAmount,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            business_id,
            customer_id,
            kind,
            amount,
            occurred_at,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        }
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }

    /// Merekonstruksi Transaction dari data yang SUDAH tersimpan. Dipakai
    /// HANYA oleh implementasi Repository konkret.
    pub fn from_persisted(data: PersistedTransaction) -> Self {
        Self {
            id: data.id,
            business_id: data.business_id,
            customer_id: data.customer_id,
            kind: data.kind,
            amount: data.amount,
            occurred_at: data.occurred_at,
            created_at: data.created_at,
            updated_at: data.updated_at,
            deleted_at: data.deleted_at,
            version: data.version,
        }
    }

    pub fn business_id(&self) -> BusinessId {
        self.business_id
    }

    pub fn customer_id(&self) -> Option<CustomerId> {
        self.customer_id
    }

    pub fn kind(&self) -> &TransactionKind {
        &self.kind
    }

    pub fn amount(&self) -> TransactionAmount {
        self.amount
    }

    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
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

    fn sample_kind() -> TransactionKind {
        TransactionKind::new("sale").unwrap()
    }

    fn sample_amount() -> TransactionAmount {
        TransactionAmount::new(50_000).unwrap()
    }

    #[test]
    fn transaction_id_roundtrips_through_uuid() {
        let id = TransactionId::new();
        let rebuilt = TransactionId::from_uuid(id.as_uuid());
        assert_eq!(id, rebuilt);
    }

    #[test]
    fn transaction_id_roundtrips_through_string() {
        let id = TransactionId::new();
        let parsed: TransactionId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn transaction_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<TransactionId>(),
            Err(DomainError::InvalidId)
        );
    }

    #[test]
    fn transaction_kind_normalizes_to_lowercase_and_trims() {
        let kind = TransactionKind::new("  Sale  ").unwrap();
        assert_eq!(kind.as_str(), "sale");
    }

    #[test]
    fn transaction_kind_rejects_empty() {
        assert_eq!(
            TransactionKind::new("   "),
            Err(DomainError::EmptyTransactionKind)
        );
    }

    #[test]
    fn transaction_kind_rejects_invalid_characters() {
        assert_eq!(
            TransactionKind::new("sale!"),
            Err(DomainError::InvalidTransactionKind)
        );
        assert_eq!(
            TransactionKind::new("sale online"),
            Err(DomainError::InvalidTransactionKind)
        );
    }

    #[test]
    fn transaction_kind_allows_underscore_and_hyphen() {
        assert!(TransactionKind::new("down_payment").is_ok());
        assert!(TransactionKind::new("down-payment").is_ok());
    }

    #[test]
    fn transaction_amount_rejects_zero_and_negative() {
        assert_eq!(TransactionAmount::new(0), Err(DomainError::InvalidAmount));
        assert_eq!(
            TransactionAmount::new(-1_000),
            Err(DomainError::InvalidAmount)
        );
    }

    #[test]
    fn transaction_amount_accepts_positive_value() {
        let amount = TransactionAmount::new(50_000).unwrap();
        assert_eq!(amount.as_i64(), 50_000);
    }

    #[test]
    fn new_transaction_is_linked_to_given_business_and_has_no_customer_by_default() {
        let business_id = sample_business_id();
        let transaction = Transaction::new(
            business_id,
            None,
            sample_kind(),
            sample_amount(),
            Utc::now(),
        );

        assert_eq!(transaction.business_id(), business_id);
        assert!(transaction.customer_id().is_none());
        assert_eq!(transaction.version(), 0);
        assert!(!transaction.is_deleted());
    }

    #[test]
    fn new_transaction_can_be_linked_to_a_customer() {
        let customer_id = CustomerId::new();
        let transaction = Transaction::new(
            sample_business_id(),
            Some(customer_id),
            sample_kind(),
            sample_amount(),
            Utc::now(),
        );

        assert_eq!(transaction.customer_id(), Some(customer_id));
    }

    #[test]
    fn with_id_uses_the_given_id() {
        let id = TransactionId::new();
        let transaction = Transaction::with_id(
            id,
            sample_business_id(),
            None,
            sample_kind(),
            sample_amount(),
            Utc::now(),
        );
        assert_eq!(transaction.id(), id);
        assert_eq!(transaction.version(), 0);
    }

    #[test]
    fn soft_delete_marks_deleted_and_increments_version() {
        let mut transaction = Transaction::new(
            sample_business_id(),
            None,
            sample_kind(),
            sample_amount(),
            Utc::now(),
        );
        transaction.soft_delete();
        assert!(transaction.is_deleted());
        assert_eq!(transaction.version(), 1);
    }

    #[test]
    fn from_persisted_reconstructs_exact_state() {
        let id = TransactionId::new();
        let business_id = sample_business_id();
        let kind = sample_kind();
        let amount = sample_amount();
        let occurred_at = Utc::now();

        let transaction = Transaction::from_persisted(PersistedTransaction {
            id,
            business_id,
            customer_id: None,
            kind: kind.clone(),
            amount,
            occurred_at,
            created_at: occurred_at,
            updated_at: occurred_at,
            deleted_at: None,
            version: 5,
        });

        assert_eq!(transaction.id(), id);
        assert_eq!(transaction.business_id(), business_id);
        assert_eq!(transaction.kind(), &kind);
        assert_eq!(transaction.amount(), amount);
        assert_eq!(transaction.version(), 5);
    }
}
