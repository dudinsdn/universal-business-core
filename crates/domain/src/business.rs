use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::DomainError;
use crate::tenant::TenantId;

const MAX_NAME_LENGTH: usize = 255;

/// Identitas unik Business. Dibuat sistem (UUID v7), bukan auto-increment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BusinessId(Uuid);

impl BusinessId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
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
    /// Membuat Business baru di bawah sebuah Tenant.
    ///
    /// PENTING: keunikan nama per Tenant TIDAK dicek di sini karena entity
    /// tidak boleh mengakses data Business lain (butuh Repository).
    /// Panggil `rules::ensure_business_name_unique` di Application Service
    /// sebelum memanggil constructor ini.
    pub fn new(tenant_id: TenantId, name: BusinessName, business_type: BusinessType) -> Self {
        let now = Utc::now();
        Self {
            id: BusinessId::new(),
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
    pub fn from_persisted(
        id: BusinessId,
        tenant_id: TenantId,
        name: BusinessName,
        business_type: BusinessType,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        version: u32,
    ) -> Self {
        Self {
            id,
            tenant_id,
            name,
            business_type,
            created_at,
            updated_at,
            deleted_at,
            version,
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
    fn from_persisted_reconstructs_exact_state() {
        let id = BusinessId::new();
        let tenant_id = sample_tenant_id();
        let name = BusinessName::new("Toko Baju").unwrap();
        let business_type = BusinessType::new("retail").unwrap();
        let created_at = Utc::now();

        let business = Business::from_persisted(
            id,
            tenant_id,
            name.clone(),
            business_type.clone(),
            created_at,
            created_at,
            None,
            5,
        );

        assert_eq!(business.id(), id);
        assert_eq!(business.tenant_id(), tenant_id);
        assert_eq!(business.name(), &name);
        assert_eq!(business.business_type(), &business_type);
        assert_eq!(business.version(), 5);
    }
}
