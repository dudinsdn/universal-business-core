use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::DomainError;
use crate::tenant::TenantId;

const MAX_NAME_LENGTH: usize = 255;

/// Identitas unik Business. Selalu berupa UUID v7. Bisa di-generate oleh
/// sistem (`BusinessId::new`) atau ditentukan oleh pemanggil (dipakai untuk
/// idempotent create, lihat `Business::with_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BusinessId(Uuid);

impl BusinessId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Uuid mentah di baliknya. Dipakai implementasi Repository konkret
    /// untuk binding parameter query.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Kebalikan dari `as_uuid`: membangun BusinessId dari Uuid yang sudah
    /// ada (mis. hasil baca kolom database).
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for BusinessId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BusinessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BusinessId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidId)
    }
}

/// Nama tampilan Business. Keunikan nama per Tenant TIDAK dicek di sini —
/// lihat `rules::ensure_business_name_unique`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BusinessName(String);

impl BusinessName {
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

/// Kode jenis usaha. Sengaja berupa string terbuka (bukan enum tertutup)
/// supaya Core Domain tidak perlu diubah setiap kali capability baru
/// (Retail, Laundry, Klinik, dll) ditambahkan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BusinessType(String);

impl BusinessType {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let normalized = raw.into().trim().to_lowercase();
        if normalized.is_empty() {
            return Err(DomainError::EmptyBusinessType);
        }
        let is_valid = normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !is_valid {
            return Err(DomainError::InvalidBusinessType);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Data mentah untuk merekonstruksi Business dari penyimpanan (mis. satu
/// baris hasil query database). Dikelompokkan jadi satu struct supaya
/// `from_persisted` tidak melanggar `clippy::too_many_arguments` (>7
/// parameter) — bukan konsep bisnis, murni pembawa data untuk Repository.
pub struct PersistedBusiness {
    pub id: BusinessId,
    pub tenant_id: TenantId,
    pub name: BusinessName,
    pub business_type: BusinessType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Entity Business: satu bisnis/cabang nyata yang hidup di dalam satu Tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Business {
    id: BusinessId,
    tenant_id: TenantId,
    name: BusinessName,
    business_type: BusinessType,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl Business {
    /// Membuat Business baru di bawah sebuah Tenant, dengan Id yang
    /// di-generate otomatis oleh sistem.
    ///
    /// PENTING: keunikan nama per Tenant TIDAK dicek di sini karena entity
    /// tidak boleh mengakses data Business lain (butuh Repository).
    /// Panggil `rules::ensure_business_name_unique` di Application Service
    /// sebelum memanggil constructor ini.
    pub fn new(tenant_id: TenantId, name: BusinessName, business_type: BusinessType) -> Self {
        Self::with_id(BusinessId::new(), tenant_id, name, business_type)
    }

    /// Membuat Business baru dengan Id yang SUDAH ditentukan (mis. dikirim
    /// oleh client saat request create).
    ///
    /// Alasan sama seperti `Tenant::with_id`: dipakai untuk idempotent
    /// create — retry request create yang sama (Id sama) dari client tidak
    /// membuat Business duplikat, dan jadi fondasi alami untuk Offline First.
    pub fn with_id(
        id: BusinessId,
        tenant_id: TenantId,
        name: BusinessName,
        business_type: BusinessType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            tenant_id,
            name,
            business_type,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        }
    }

    pub fn id(&self) -> BusinessId {
        self.id
    }

    /// Merekonstruksi Business dari data yang SUDAH tersimpan. Lihat
    /// `Tenant::from_persisted` untuk alasan yang sama.
    pub fn from_persisted(data: PersistedBusiness) -> Self {
        Self {
            id: data.id,
            tenant_id: data.tenant_id,
            name: data.name,
            business_type: data.business_type,
            created_at: data.created_at,
            updated_at: data.updated_at,
            deleted_at: data.deleted_at,
            version: data.version,
        }
    }

    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    pub fn name(&self) -> &BusinessName {
        &self.name
    }

    pub fn business_type(&self) -> &BusinessType {
        &self.business_type
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

    /// Timestamp soft-delete mentah (`None` kalau belum dihapus). Dipakai
    /// Repository konkret untuk menyimpan nilai aslinya, bukan cuma boolean.
    pub fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    pub fn rename(&mut self, name: BusinessName) {
        self.name = name;
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

    fn sample_tenant_id() -> TenantId {
        TenantId::new()
    }

    #[test]
    fn business_id_roundtrips_through_uuid() {
        let id = BusinessId::new();
        let rebuilt = BusinessId::from_uuid(id.as_uuid());
        assert_eq!(id, rebuilt);
    }

    #[test]
    fn business_id_roundtrips_through_string() {
        let id = BusinessId::new();
        let parsed: BusinessId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn business_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<BusinessId>(),
            Err(DomainError::InvalidId)
        );
    }

    #[test]
    fn business_name_rejects_empty_string() {
        assert_eq!(BusinessName::new(""), Err(DomainError::EmptyName));
    }

    #[test]
    fn business_type_normalizes_to_lowercase_and_trims() {
        let bt = BusinessType::new("  Laundry  ").unwrap();
        assert_eq!(bt.as_str(), "laundry");
    }

    #[test]
    fn business_type_rejects_empty() {
        assert_eq!(
            BusinessType::new("   "),
            Err(DomainError::EmptyBusinessType)
        );
    }

    #[test]
    fn business_type_rejects_invalid_characters() {
        assert_eq!(
            BusinessType::new("laundry!"),
            Err(DomainError::InvalidBusinessType)
        );
        assert_eq!(
            BusinessType::new("laundry express"),
            Err(DomainError::InvalidBusinessType)
        );
    }

    #[test]
    fn business_type_allows_underscore_and_hyphen() {
        assert!(BusinessType::new("auto_workshop").is_ok());
        assert!(BusinessType::new("auto-workshop").is_ok());
    }

    #[test]
    fn new_business_is_linked_to_given_tenant() {
        let tenant_id = sample_tenant_id();
        let business = Business::new(
            tenant_id,
            BusinessName::new("Toko Baju").unwrap(),
            BusinessType::new("retail").unwrap(),
        );
        assert_eq!(business.tenant_id(), tenant_id);
        assert_eq!(business.version(), 0);
        assert!(!business.is_deleted());
    }

    #[test]
    fn with_id_uses_the_given_id() {
        let id = BusinessId::new();
        let tenant_id = sample_tenant_id();
        let business = Business::with_id(
            id,
            tenant_id,
            BusinessName::new("Toko Baju").unwrap(),
            BusinessType::new("retail").unwrap(),
        );
        assert_eq!(business.id(), id);
        assert_eq!(business.version(), 0);
    }

    #[test]
    fn from_persisted_reconstructs_exact_state() {
        let id = BusinessId::new();
        let tenant_id = sample_tenant_id();
        let name = BusinessName::new("Toko Baju").unwrap();
        let business_type = BusinessType::new("retail").unwrap();
        let created_at = Utc::now();

        let business = Business::from_persisted(PersistedBusiness {
            id,
            tenant_id,
            name: name.clone(),
            business_type: business_type.clone(),
            created_at,
            updated_at: created_at,
            deleted_at: None,
            version: 5,
        });

        assert_eq!(business.id(), id);
        assert_eq!(business.tenant_id(), tenant_id);
        assert_eq!(business.name(), &name);
        assert_eq!(business.business_type(), &business_type);
        assert_eq!(business.version(), 5);
    }
}
