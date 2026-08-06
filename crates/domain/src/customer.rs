//! Entity Customer: pelanggan yang dimiliki oleh satu Business.
//!
//! Beda penting dari Business: nama Customer TIDAK wajib unik (dua orang
//! boleh bernama sama), jadi tidak ada rule
//! `ensure_customer_name_unique` di sini. Kontak (telepon) opsional,
//! karena Customer bisa dibuat dulu tanpa kontak lalu dilengkapi belakangan.

use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::business::BusinessId;
use crate::error::DomainError;

const MAX_NAME_LENGTH: usize = 255;
const MAX_PHONE_LENGTH: usize = 32;

/// Identitas unik Customer. Selalu berupa UUID v7. Bisa di-generate oleh
/// sistem (`CustomerId::new`) atau ditentukan oleh pemanggil (idempotent
/// create, lihat `Customer::with_id`) — pola sama seperti `BusinessId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CustomerId(Uuid);

impl CustomerId {
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

impl Default for CustomerId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CustomerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for CustomerId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidId)
    }
}

/// Nama tampilan Customer. Keunikan TIDAK dicek — beda dari
/// `BusinessName`, karena banyak customer boleh bernama sama.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomerName(String);

impl CustomerName {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyName);
        }
        if trimmed.chars().count() > MAX_NAME_LENGTH {
            return Err(DomainError::NameTooLong {
                max: MAX_NAME_LENGTH,
            });
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Nomor telepon Customer. SELALU dibungkus `Option<CustomerPhone>` di
/// level `Customer` — kalau customer belum punya nomor telepon, field-nya
/// `None`, BUKAN `CustomerPhone` berisi string kosong. Validasi di sini
/// sengaja longgar (bukan format E.164 dsb.) karena belum ada kebutuhan
/// nyata untuk itu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomerPhone(String);

impl CustomerPhone {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyPhone);
        }
        if trimmed.chars().count() > MAX_PHONE_LENGTH {
            return Err(DomainError::PhoneTooLong {
                max: MAX_PHONE_LENGTH,
            });
        }
        let is_valid = trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | ' ' | '(' | ')'));
        if !is_valid {
            return Err(DomainError::InvalidPhone);
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Data mentah untuk merekonstruksi Customer dari penyimpanan. Sama
/// alasannya dengan `PersistedBusiness`: menghindari constructor dengan
/// terlalu banyak parameter.
pub struct PersistedCustomer {
    pub id: CustomerId,
    pub business_id: BusinessId,
    pub name: CustomerName,
    pub phone: Option<CustomerPhone>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Entity Customer: satu pelanggan nyata milik satu Business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    id: CustomerId,
    business_id: BusinessId,
    name: CustomerName,
    phone: Option<CustomerPhone>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl Customer {
    /// Membuat Customer baru di bawah satu Business, dengan Id yang
    /// di-generate otomatis oleh sistem.
    ///
    /// PENTING: pengecekan "apakah Business masih aktif" TIDAK dilakukan
    /// di sini — entity tidak boleh tahu status Business lain. Panggil
    /// `rules::ensure_business_is_active` di Application Service sebelum
    /// memanggil constructor ini.
    pub fn new(business_id: BusinessId, name: CustomerName, phone: Option<CustomerPhone>) -> Self {
        Self::with_id(CustomerId::new(), business_id, name, phone)
    }

    /// Membuat Customer baru dengan Id yang SUDAH ditentukan (idempotent
    /// create) — pola sama seperti `Business::with_id`.
    pub fn with_id(
        id: CustomerId,
        business_id: BusinessId,
        name: CustomerName,
        phone: Option<CustomerPhone>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            business_id,
            name,
            phone,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        }
    }

    pub fn id(&self) -> CustomerId {
        self.id
    }

    /// Merekonstruksi Customer dari data yang SUDAH tersimpan. Dipakai
    /// HANYA oleh implementasi Repository konkret.
    pub fn from_persisted(data: PersistedCustomer) -> Self {
        Self {
            id: data.id,
            business_id: data.business_id,
            name: data.name,
            phone: data.phone,
            created_at: data.created_at,
            updated_at: data.updated_at,
            deleted_at: data.deleted_at,
            version: data.version,
        }
    }

    pub fn business_id(&self) -> BusinessId {
        self.business_id
    }

    pub fn name(&self) -> &CustomerName {
        &self.name
    }

    pub fn phone(&self) -> Option<&CustomerPhone> {
        self.phone.as_ref()
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

    pub fn rename(&mut self, name: CustomerName) {
        self.name = name;
        self.touch();
    }

    /// Mengganti nomor telepon. Kirim `None` untuk menghapus nomor telepon
    /// yang tersimpan (mis. customer tidak mau dihubungi lewat telepon lagi).
    pub fn update_phone(&mut self, phone: Option<CustomerPhone>) {
        self.phone = phone;
        self.touch();
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

    #[test]
    fn customer_id_roundtrips_through_uuid() {
        let id = CustomerId::new();
        let rebuilt = CustomerId::from_uuid(id.as_uuid());
        assert_eq!(id, rebuilt);
    }

    #[test]
    fn customer_id_roundtrips_through_string() {
        let id = CustomerId::new();
        let parsed: CustomerId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn customer_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<CustomerId>(),
            Err(DomainError::InvalidId)
        );
    }

    #[test]
    fn customer_name_rejects_empty_string() {
        assert_eq!(CustomerName::new(""), Err(DomainError::EmptyName));
        assert_eq!(CustomerName::new("   "), Err(DomainError::EmptyName));
    }

    #[test]
    fn customer_name_trims_whitespace() {
        let name = CustomerName::new("  Budi Santoso  ").unwrap();
        assert_eq!(name.as_str(), "Budi Santoso");
    }

    #[test]
    fn customer_phone_rejects_empty_string() {
        assert_eq!(CustomerPhone::new(""), Err(DomainError::EmptyPhone));
    }

    #[test]
    fn customer_phone_accepts_common_formats() {
        assert!(CustomerPhone::new("081234567890").is_ok());
        assert!(CustomerPhone::new("+62 812-3456-7890").is_ok());
        assert!(CustomerPhone::new("(021) 555-0100").is_ok());
    }

    #[test]
    fn customer_phone_rejects_letters() {
        assert_eq!(
            CustomerPhone::new("call-me-maybe"),
            Err(DomainError::InvalidPhone)
        );
    }

    #[test]
    fn new_customer_is_linked_to_given_business_and_has_no_phone_by_default() {
        let business_id = sample_business_id();
        let customer = Customer::new(business_id, CustomerName::new("Budi").unwrap(), None);

        assert_eq!(customer.business_id(), business_id);
        assert_eq!(customer.version(), 0);
        assert!(!customer.is_deleted());
        assert!(customer.phone().is_none());
    }

    #[test]
    fn with_id_uses_the_given_id() {
        let id = CustomerId::new();
        let business_id = sample_business_id();
        let customer = Customer::with_id(id, business_id, CustomerName::new("Budi").unwrap(), None);
        assert_eq!(customer.id(), id);
        assert_eq!(customer.version(), 0);
    }

    #[test]
    fn rename_increments_version() {
        let mut customer = Customer::new(
            sample_business_id(),
            CustomerName::new("Budi").unwrap(),
            None,
        );
        customer.rename(CustomerName::new("Budi Santoso").unwrap());
        assert_eq!(customer.version(), 1);
        assert_eq!(customer.name().as_str(), "Budi Santoso");
    }

    #[test]
    fn update_phone_sets_and_clears_phone() {
        let mut customer = Customer::new(
            sample_business_id(),
            CustomerName::new("Budi").unwrap(),
            None,
        );

        customer.update_phone(Some(CustomerPhone::new("081234567890").unwrap()));
        assert_eq!(customer.version(), 1);
        assert!(customer.phone().is_some());

        customer.update_phone(None);
        assert_eq!(customer.version(), 2);
        assert!(customer.phone().is_none());
    }

    #[test]
    fn soft_delete_marks_deleted_and_increments_version() {
        let mut customer = Customer::new(
            sample_business_id(),
            CustomerName::new("Budi").unwrap(),
            None,
        );
        customer.soft_delete();
        assert!(customer.is_deleted());
        assert_eq!(customer.version(), 1);
    }

    #[test]
    fn from_persisted_reconstructs_exact_state() {
        let id = CustomerId::new();
        let business_id = sample_business_id();
        let name = CustomerName::new("Budi").unwrap();
        let created_at = Utc::now();

        let customer = Customer::from_persisted(PersistedCustomer {
            id,
            business_id,
            name: name.clone(),
            phone: None,
            created_at,
            updated_at: created_at,
            deleted_at: None,
            version: 5,
        });

        assert_eq!(customer.id(), id);
        assert_eq!(customer.business_id(), business_id);
        assert_eq!(customer.name(), &name);
        assert_eq!(customer.version(), 5);
    }
}
