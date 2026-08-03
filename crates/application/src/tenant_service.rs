use domain::{Tenant, TenantId, TenantName, rules};

use crate::error::ApplicationError;
use crate::repository::{BusinessRepository, TenantRepository};

/// Orkestrasi use-case seputar Tenant: ambil data lewat Repository,
/// validasi lewat business rule domain, lalu simpan lewat Repository.
#[derive(Clone)]
pub struct TenantService<R: TenantRepository> {
    repository: R,
}

impl<R: TenantRepository> TenantService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn create_tenant(&self, name: TenantName) -> Result<Tenant, ApplicationError> {
        let tenant = Tenant::new(name);
        self.repository.save(&tenant).await?;
        Ok(tenant)
    }

    /// Mengambil satu Tenant berdasarkan id. Dibutuhkan pemanggil (mis. API)
    /// yang perlu objek `Tenant` utuh sebelum memanggil use-case lain, mis.
    /// `BusinessService::create_business` yang menerima `&Tenant`.
    pub async fn get_tenant(&self, id: TenantId) -> Result<Tenant, ApplicationError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::TenantNotFound)
    }

    pub async fn rename_tenant(
        &self,
        id: TenantId,
        new_name: TenantName,
        expected_version: u32,
    ) -> Result<Tenant, ApplicationError> {
        let mut tenant = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::TenantNotFound)?;

        rules::ensure_version_matches(expected_version, tenant.version())?;

        tenant.rename(new_name);
        self.repository.save(&tenant).await?;
        Ok(tenant)
    }

    /// Menghapus Tenant (soft delete).
    ///
    /// Butuh `BusinessRepository` sebagai parameter eksplisit — ini
    /// operasi lintas-aggregate (Tenant + Business), jadi kebutuhannya
    /// dinyatakan jelas di titik pemanggilan, bukan disembunyikan sebagai
    /// dependency tetap di struct `TenantService`.
    pub async fn delete_tenant(
        &self,
        id: TenantId,
        business_repository: &impl BusinessRepository,
    ) -> Result<(), ApplicationError> {
        let mut tenant = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::TenantNotFound)?;

        let active_business_count = business_repository.count_active_by_tenant(id).await?;
        rules::ensure_tenant_can_be_deleted(active_business_count)?;

        tenant.soft_delete();
        self.repository.save(&tenant).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::{InMemoryBusinessRepository, InMemoryTenantRepository};
    use domain::{Business, BusinessName, BusinessType, DomainError};

    #[tokio::test]
    async fn create_tenant_saves_and_returns_tenant() {
        let repo = InMemoryTenantRepository::new();
        let service = TenantService::new(repo);

        let tenant = service
            .create_tenant(TenantName::new("Tenant A").unwrap())
            .await
            .unwrap();

        assert_eq!(tenant.version(), 0);
    }

    #[tokio::test]
    async fn get_tenant_returns_saved_tenant() {
        let repo = InMemoryTenantRepository::new();
        let service = TenantService::new(repo);
        let created = service
            .create_tenant(TenantName::new("Tenant A").unwrap())
            .await
            .unwrap();

        let fetched = service.get_tenant(created.id()).await.unwrap();

        assert_eq!(fetched.id(), created.id());
    }

    #[tokio::test]
    async fn get_tenant_fails_when_not_found() {
        let repo = InMemoryTenantRepository::new();
        let service = TenantService::new(repo);

        let result = service.get_tenant(TenantId::new()).await;

        assert!(matches!(result, Err(ApplicationError::TenantNotFound)));
    }

    #[tokio::test]
    async fn rename_tenant_increments_version() {
        let repo = InMemoryTenantRepository::new();
        let service = TenantService::new(repo);
        let tenant = service
            .create_tenant(TenantName::new("Tenant A").unwrap())
            .await
            .unwrap();

        let renamed = service
            .rename_tenant(
                tenant.id(),
                TenantName::new("Tenant A Baru").unwrap(),
                tenant.version(),
            )
            .await
            .unwrap();

        assert_eq!(renamed.version(), 1);
        assert_eq!(renamed.name().as_str(), "Tenant A Baru");
    }

    #[tokio::test]
    async fn rename_tenant_rejects_stale_version() {
        let repo = InMemoryTenantRepository::new();
        let service = TenantService::new(repo);
        let tenant = service
            .create_tenant(TenantName::new("Tenant A").unwrap())
            .await
            .unwrap();

        let result = service
            .rename_tenant(
                tenant.id(),
                TenantName::new("Tenant A Baru").unwrap(),
                tenant.version() + 1,
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::VersionConflict))
        ));
    }

    #[tokio::test]
    async fn rename_tenant_fails_when_not_found() {
        let repo = InMemoryTenantRepository::new();
        let service = TenantService::new(repo);

        let result = service
            .rename_tenant(TenantId::new(), TenantName::new("X").unwrap(), 0)
            .await;

        assert!(matches!(result, Err(ApplicationError::TenantNotFound)));
    }

    #[tokio::test]
    async fn delete_tenant_fails_when_active_business_exists() {
        let tenant_repo = InMemoryTenantRepository::new();
        let business_repo = InMemoryBusinessRepository::new();
        let service = TenantService::new(tenant_repo);

        let tenant = service
            .create_tenant(TenantName::new("Tenant A").unwrap())
            .await
            .unwrap();
        let business = Business::new(
            tenant.id(),
            BusinessName::new("Toko Baju").unwrap(),
            BusinessType::new("retail").unwrap(),
        );
        business_repo.save(&business).await.unwrap();

        let result = service.delete_tenant(tenant.id(), &business_repo).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(
                DomainError::TenantHasActiveBusiness
            ))
        ));
    }

    #[tokio::test]
    async fn delete_tenant_succeeds_when_no_active_business() {
        let tenant_repo = InMemoryTenantRepository::new();
        let business_repo = InMemoryBusinessRepository::new();
        let service = TenantService::new(tenant_repo);

        let tenant = service
            .create_tenant(TenantName::new("Tenant A").unwrap())
            .await
            .unwrap();

        service
            .delete_tenant(tenant.id(), &business_repo)
            .await
            .unwrap();

        let stored = service
            .repository
            .find_by_id(tenant.id())
            .await
            .unwrap()
            .unwrap();
        assert!(stored.is_deleted());
    }
}
