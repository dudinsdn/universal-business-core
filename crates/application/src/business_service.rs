use domain::{Business, BusinessId, BusinessName, BusinessType, Tenant, rules};

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

    pub async fn create_business(
        &self,
        tenant: &Tenant,
        name: BusinessName,
        business_type: BusinessType,
    ) -> Result<Business, ApplicationError> {
        rules::ensure_tenant_is_active(tenant.is_deleted())?;

        let existing_names = self
            .repository
            .find_active_names_by_tenant(tenant.id())
            .await?;
        rules::ensure_business_name_unique(&existing_names, &name)?;

        let business = Business::new(tenant.id(), name, business_type);
        self.repository.save(&business).await?;
        Ok(business)
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

    #[tokio::test]
    async fn create_business_succeeds_for_active_tenant() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();

        let business = service
            .create_business(
                &tenant,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();

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

        service
            .create_business(
                &tenant,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();

        let result = service
            .create_business(
                &tenant,
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

        service
            .create_business(
                &tenant_a,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();

        let result = service
            .create_business(
                &tenant_b,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn rename_business_rejects_stale_version() {
        let repo = InMemoryBusinessRepository::new();
        let service = BusinessService::new(repo);
        let tenant = active_tenant();
        let business = service
            .create_business(
                &tenant,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();

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
        let business = service
            .create_business(
                &tenant,
                BusinessName::new("Toko Baju").unwrap(),
                BusinessType::new("retail").unwrap(),
            )
            .await
            .unwrap();

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
