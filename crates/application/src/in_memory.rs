//! Implementasi in-memory dari trait Repository.
//!
//! Dipakai untuk dua kebutuhan nyata:
//! - Unit test Application Service (cepat, tanpa infrastruktur).
//! - Bootstrap awal API sebelum implementasi Postgres tersedia
//!   (lihat Development Rules: "Implementasikan API" (poin 5) mendahului
//!   "Tambahkan database" (poin 6)).
//!
//! Data disimpan dalam `Arc<Mutex<..>>`, bukan `RefCell`, karena struct ini
//! harus bisa di-clone dan dibagi antar-thread (HTTP server multi-thread).
//! `Clone` di sini murah: hanya menyalin `Arc` (pointer + refcount), bukan
//! menyalin seluruh data — semua clone tetap menunjuk ke penyimpanan yang
//! sama.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use domain::{Business, BusinessId, BusinessName, Tenant, TenantId};

use crate::error::RepositoryError;
use crate::repository::{BusinessRepository, TenantRepository};

#[derive(Debug, Clone, Default)]
pub struct InMemoryTenantRepository {
    data: Arc<Mutex<HashMap<TenantId, Tenant>>>,
}

impl InMemoryTenantRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TenantRepository for InMemoryTenantRepository {
    async fn find_by_id(&self, id: TenantId) -> Result<Option<Tenant>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn save(&self, tenant: &Tenant) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&tenant.id()) {
            let expected_previous_version = tenant.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(tenant.id(), tenant.clone());
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryBusinessRepository {
    data: Arc<Mutex<HashMap<BusinessId, Business>>>,
}

impl InMemoryBusinessRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BusinessRepository for InMemoryBusinessRepository {
    async fn find_by_id(&self, id: BusinessId) -> Result<Option<Business>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn find_active_names_by_tenant(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<BusinessName>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data
            .values()
            .filter(|b| b.tenant_id() == tenant_id && !b.is_deleted())
            .map(|b| b.name().clone())
            .collect())
    }

    async fn count_active_by_tenant(&self, tenant_id: TenantId) -> Result<usize, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data
            .values()
            .filter(|b| b.tenant_id() == tenant_id && !b.is_deleted())
            .count())
    }

    async fn save(&self, business: &Business) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&business.id()) {
            let expected_previous_version = business.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(business.id(), business.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::TenantName;

    #[tokio::test]
    async fn save_detects_concurrent_version_conflict() {
        let repo = InMemoryTenantRepository::new();
        let mut tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        repo.save(&tenant).await.unwrap();

        // Dua "pembaca" sama-sama mulai dari data versi 0.
        let mut stale_copy = tenant.clone();

        // Salah satu menang duluan: rename, jadi versi 1, berhasil disimpan.
        tenant.rename(TenantName::new("Tenant A Baru").unwrap());
        repo.save(&tenant).await.unwrap();

        // Yang satu lagi telat: masih berdasarkan data versi 0, ikut jadi
        // versi 1 di sisinya sendiri, tapi versi 0 di penyimpanan sudah
        // tidak ada lagi — harus ditolak sebagai conflict.
        stale_copy.rename(TenantName::new("Tenant A Telat").unwrap());
        let result = repo.save(&stale_copy).await;

        assert_eq!(result, Err(RepositoryError::VersionConflict));
    }
}
