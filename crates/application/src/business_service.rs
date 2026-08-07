use chrono::{DateTime, Utc};
use domain::{Business, BusinessId, BusinessName, BusinessType, Tenant, TenantId, rules};

use crate::error::ApplicationError;
use crate::repository::BusinessRepository;

/// Orkestrasi use-case seputar Business.
///
/// Sengaja hanya bergantung pada `BusinessRepository`, bukan juga
/// `TenantRepository` — `tenant` yang sudah diambil pemanggil dikirim
/// sebagai parameter. Ini menjaga service tetap fokus pada satu aggregate
/// dan mudah diuji, konsisten dengan pola `delete_tenant` di `TenantService`.
#[derive(Clone)]
pub struct BusinessService<R: BusinessRepository> {
    repository: R,
}

impl<R: BusinessRepository> BusinessService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Membuat Business baru — idempotent terhadap `id`, dengan alasan dan
    /// kontrak return value yang sama seperti `TenantService::create_tenant`.
    ///
    /// PENTING soal urutan: pengecekan "apakah `id` sudah ada" dilakukan
    /// SEBELUM `ensure_tenant_is_active` dan `ensure_business_name_unique`.
    /// Kalau urutan dibalik, retry dengan payload yang identik ke request
    /// pertama akan salah ditolak sebagai "nama duplikat" — padahal nama
    /// itu adalah nama Business itu sendiri dari request pertama.
    pub async fn create_business(
        &self,
        tenant: &Tenant,
        id: BusinessId,
        name: BusinessName,
        business_type: BusinessType,
    ) -> Result<(Business, bool), ApplicationError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_tenant_is_active(tenant.is_deleted())?;

        let existing_names = self
            .repository
            .find_active_names_by_tenant(tenant.id())
            .await?;
        rules::ensure_business_name_unique(&existing_names, &name)?;

        let business = Business::with_id(id, tenant.id(), name, business_type);
        self.repository.save(&business).await?;
        Ok((business, true))
    }

    /// Mengambil satu Business berdasarkan id. Dibutuhkan pemanggil (mis.
    /// API) yang perlu objek `Business` utuh sebelum memanggil use-case
    /// lain, mis. `CustomerService::create_customer` yang menerima
    /// `&Business` — pola sama seperti `TenantService::get_tenant`.
    pub async fn get_business(&self, id: BusinessId) -> Result<Business, ApplicationError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::BusinessNotFound)
    }

    /// Semua Business di bawah satu Tenant yang berubah sejak `since` —
    /// dipakai endpoint incremental sync
    /// (`GET /tenants/{tenant_id}/businesses?updated_since=...`). Lihat
    /// `BusinessRepository::find_updated_since_by_tenant` untuk detail
    /// (termasuk Business yang sudah di-soft-delete).
    pub async fn list_updated_since(
        &self,
        tenant_id: TenantId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Business>, ApplicationError> {
        Ok(self
            .repository
            .find_updated_since_by_tenant(tenant_id, since)
            .await?)
    }

    pub async fn rename_business(
        &self,
        id: BusinessId,
        new_name: BusinessName,
        expected_version: u32,
    ) -> Result<Business, ApplicationError> {
        let mut business = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::BusinessNotFound)?;

        rules::ensure_version_matches(expected_version, business.version())?;

        let existing_names: Vec<BusinessName> = self
            .repository
            .find_active_names_by_tenant(business.tenant_id())
            .await?
            .into_iter()
            .filter(|existing| existing != business.name())
            .collect();
        rules::ensure_business_name_unique(&existing_names, &new_name)?;

        business.rename(new_name);
        self.repository.save(&business).await?;
        Ok(business)
    }

    pub async fn delete_business(
        &self,
        id: BusinessId,
        expected_version: u32,
    ) -> Result<(), ApplicationError> {
        let mut business = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::BusinessNotFound)?;

        rules::ensure_version_matches(expected_version, business.version())?;

        business.soft_delete();
        self.repository.save(&business).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryBusinessRepository;
    use domain::{DomainError, TenantName};

    fn active_tenant() -> Tenant {
        Tenant::new(TenantName::new("Tenant A").unwrap())
    }

    /// Helper test: membuat Business dengan Id baru yang di-generate acak
    /// (skenario umum, bukan skenario idempotency).
    async fn create_test_business(
        service: &BusinessService<InMemoryBusinessRepository>,
        tenant: &Tenant,
        name: &str,
    ) -> Business {
        service
            .create_business(
                tenant,
                BusinessId::new(),
                BusinessName::new(name).unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_business_succeeds_for_active_tenant() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();

        let business = create_test_business(&service, &tenant, "Toko Baju").await;

        assert_eq!(business.tenant_id(), tenant.id());
    }

    #[tokio::test]
    async fn create_business_rejects_deleted_tenant() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let mut tenant = active_tenant();
        tenant.soft_delete();

        let result = service
            .create_business(
                &tenant,
                BusinessId::new(),
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::TenantIsDeleted))
        ));
    }

    #[tokio::test]
    async fn create_business_rejects_duplicate_name_in_same_tenant() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();

        create_test_business(&service, &tenant, "Toko Baju").await;

        let result = service
            .create_business(
                &tenant,
                BusinessId::new(),
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::DuplicateBusinessName))
        ));
    }

    #[tokio::test]
    async fn create_business_allows_same_name_in_different_tenant() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant_a = active_tenant();
        let tenant_b = active_tenant();

        create_test_business(&service, &tenant_a, "Toko Baju").await;

        let result = service
            .create_business(
                &tenant_b,
                BusinessId::new(),
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn create_business_with_same_id_is_idempotent() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();
        let id = BusinessId::new();

        let (first, first_created) = service
            .create_business(
                &tenant,
                id,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();
        assert!(first_created);

        // Retry dengan Id sama TAPI nama beda — ini membuktikan pengecekan
        // idempotency terjadi SEBELUM pengecekan nama duplikat. Kalau
        // urutannya terbalik, retry ini justru akan salah ditolak sebagai
        // "nama duplikat" (bentrok dengan business hasil request pertama).
        let (second, second_created) = service
            .create_business(
                &tenant,
                id,
                BusinessName::new("Toko Baju Lain").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();

        assert!(!second_created);
        assert_eq!(second.id(), first.id());
        assert_eq!(second.name(), first.name());
    }

    #[tokio::test]
    async fn get_business_returns_saved_business() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();
        let created = create_test_business(&service, &tenant, "Toko Baju").await;

        let fetched = service.get_business(created.id()).await.unwrap();

        assert_eq!(fetched.id(), created.id());
    }

    #[tokio::test]
    async fn get_business_fails_when_not_found() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);

        let result = service.get_business(BusinessId::new()).await;

        assert!(matches!(result, Err(ApplicationError::BusinessNotFound)));
    }

    #[tokio::test]
    async fn list_updated_since_only_returns_business_from_that_tenant() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant_a = active_tenant();
        let tenant_b = active_tenant();

        create_test_business(&service, &tenant_a, "Toko A").await;
        create_test_business(&service, &tenant_b, "Toko B").await;

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = service
            .list_updated_since(tenant_a.id(), epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].tenant_id(), tenant_a.id());
    }

    #[tokio::test]
    async fn list_updated_since_excludes_business_changed_before_cursor() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();

        create_test_business(&service, &tenant, "Toko Lama").await;
        let cursor = Utc::now();
        let baru = create_test_business(&service, &tenant, "Toko Baru").await;

        let changed = service
            .list_updated_since(tenant.id(), cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), baru.id());
    }

    #[tokio::test]
    async fn rename_business_rejects_stale_version() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();
        let business = create_test_business(&service, &tenant, "Toko Baju").await;

        let result = service
            .rename_business(
                business.id(),
                BusinessName::new("Toko Baju Baru").unwrap(),
                business.version() + 1,
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::VersionConflict))
        ));
    }

    #[tokio::test]
    async fn delete_business_marks_as_deleted() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();
        let business = create_test_business(&service, &tenant, "Toko Baju").await;

        service
            .delete_business(business.id(), business.version())
            .await
            .unwrap();

        let stored = service
            .repository
            .find_by_id(business.id())
            .await
            .unwrap()
            .unwrap();
        assert!(stored.is_deleted());
    }
}
