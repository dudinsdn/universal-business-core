use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::DomainError;

const MAX_NAME_LENGTH: usize = 255;

/// Identitas unik Tenant. Selalu dibuat oleh sistem (UUID v7: timestamp +
/// random, urut berdasarkan waktu dibuat), tidak pernah diisi manual oleh
/// user — sesuai Development Rules: jangan pakai auto-increment integer
/// sebagai identitas utama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TenantId(Uuid);

impl TenantId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TenantId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidId)
    }
}

/// Nama Tenant. Value Object — begitu berhasil dibuat, isinya pasti valid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantName(String);

impl TenantName {
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

/// Entity Tenant: batas isolasi data di platform multi-tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    id: TenantId,
    name: TenantName,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl Tenant {
    /// Satu-satunya cara membuat Tenant baru dari luar modul ini,
    /// sehingga Tenant tidak pernah berada pada state yang tidak valid.
    pub fn new(name: TenantName) -> Self {
        let now = Utc::now();
        Self {
            id: TenantId::new(),
            name,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        }
    }

    pub fn id(&self) -> TenantId {
        self.id
    }

    pub fn name(&self) -> &TenantName {
        &self.name
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    pub fn rename(&mut self, name: TenantName) {
        self.name = name;
        self.touch();
    }

    /// Menandai Tenant sebagai terhapus (soft delete).
    ///
    /// PENTING: method ini tidak mengecek apakah masih ada Business aktif —
    /// itu bukan tanggung jawab entity ini karena Tenant tidak menyimpan
    /// daftar Business (aggregate terpisah). Pemanggil (Application Service)
    /// WAJIB memanggil `rules::ensure_tenant_can_be_deleted` terlebih dahulu.
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

    #[test]
    fn tenant_id_roundtrips_through_string() {
        let id = TenantId::new();
        let parsed: TenantId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn tenant_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<TenantId>(),
            Err(DomainError::InvalidId)
        );
    }

    #[test]
    fn tenant_name_rejects_empty_string() {
        assert_eq!(TenantName::new(""), Err(DomainError::EmptyName));
        assert_eq!(TenantName::new("   "), Err(DomainError::EmptyName));
    }

    #[test]
    fn tenant_name_trims_whitespace() {
        let name = TenantName::new("  Toko Baju  ").unwrap();
        assert_eq!(name.as_str(), "Toko Baju");
    }

    #[test]
    fn tenant_name_rejects_too_long() {
        let long_name = "a".repeat(MAX_NAME_LENGTH + 1);
        assert_eq!(
            TenantName::new(long_name),
            Err(DomainError::NameTooLong {
                max: MAX_NAME_LENGTH
            })
        );
    }

    #[test]
    fn new_tenant_starts_at_version_zero_and_not_deleted() {
        let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        assert_eq!(tenant.version(), 0);
        assert!(!tenant.is_deleted());
    }

    #[test]
    fn rename_increments_version() {
        let mut tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        tenant.rename(TenantName::new("Tenant A Baru").unwrap());
        assert_eq!(tenant.version(), 1);
        assert_eq!(tenant.name().as_str(), "Tenant A Baru");
    }

    #[test]
    fn soft_delete_marks_deleted_and_increments_version() {
        let mut tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        tenant.soft_delete();
        assert!(tenant.is_deleted());
        assert_eq!(tenant.version(), 1);
    }
}
