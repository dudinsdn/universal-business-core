use chrono::{DateTime, Utc};
use domain::{Business, BusinessId, Customer, CustomerId, CustomerName, CustomerPhone, rules};

use crate::error::ApplicationError;
use crate::repository::CustomerRepository;

/// Orkestrasi use-case seputar Customer.
///
/// Hanya bergantung pada `CustomerRepository` — `business` yang sudah
/// diambil pemanggil dikirim sebagai parameter, sama seperti
/// `BusinessService` menerima `&Tenant`. Tidak ada pengecekan keunikan
/// nama di sini (beda dari `BusinessService::create_business`) karena
/// nama Customer memang sengaja tidak unik.
#[derive(Clone)]
pub struct CustomerService<R: CustomerRepository> {
    repository: R,
}

impl<R: CustomerRepository> CustomerService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Membuat Customer baru — idempotent terhadap `id`, alasan dan
    /// kontrak sama seperti `BusinessService::create_business`.
    pub async fn create_customer(
        &self,
        business: &Business,
        id: CustomerId,
        name: CustomerName,
        phone: Option<CustomerPhone>,
    ) -> Result<(Customer, bool), ApplicationError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_business_is_active(business.is_deleted())?;

        let customer = Customer::with_id(id, business.id(), name, phone);
        self.repository.save(&customer).await?;
        Ok((customer, true))
    }

    /// Semua Customer di bawah satu Business yang berubah sejak `since` —
    /// dipakai endpoint incremental sync nanti (belum dibuat di tahap ini).
    pub async fn list_updated_since(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Customer>, ApplicationError> {
        Ok(self
            .repository
            .find_updated_since_by_business(business_id, since)
            .await?)
    }

    pub async fn rename_customer(
        &self,
        id: CustomerId,
        new_name: CustomerName,
        expected_version: u32,
    ) -> Result<Customer, ApplicationError> {
        let mut customer = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::CustomerNotFound)?;

        rules::ensure_version_matches(expected_version, customer.version())?;

        customer.rename(new_name);
        self.repository.save(&customer).await?;
        Ok(customer)
    }

    /// Mengganti nomor telepon Customer. Kirim `phone: None` untuk
    /// menghapus nomor telepon yang tersimpan.
    pub async fn update_customer_phone(
        &self,
        id: CustomerId,
        phone: Option<CustomerPhone>,
        expected_version: u32,
    ) -> Result<Customer, ApplicationError> {
        let mut customer = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::CustomerNotFound)?;

        rules::ensure_version_matches(expected_version, customer.version())?;

        customer.update_phone(phone);
        self.repository.save(&customer).await?;
        Ok(customer)
    }

    pub async fn delete_customer(
        &self,
        id: CustomerId,
        expected_version: u32,
    ) -> Result<(), ApplicationError> {
        let mut customer = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::CustomerNotFound)?;

        rules::ensure_version_matches(expected_version, customer.version())?;

        customer.soft_delete();
        self.repository.save(&customer).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryCustomerRepository;
    use domain::{BusinessName, BusinessType, DomainError, Tenant, TenantName};

    fn active_business() -> Business {
        let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        Business::new(
            tenant.id(),
            BusinessName::new("Toko Baju").unwrap(),
            BusinessType::new("retail").unwrap(),
        )
    }

    async fn create_test_customer(
        service: &CustomerService<InMemoryCustomerRepository>,
        business: &Business,
        name: &str,
    ) -> Customer {
        service
            .create_customer(
                business,
                CustomerId::new(),
                CustomerName::new(name).unwrap(),
                None,
            )
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_customer_succeeds_for_active_business() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business = active_business();

        let customer = create_test_customer(&service, &business, "Budi").await;

        assert_eq!(customer.business_id(), business.id());
    }

    #[tokio::test]
    async fn create_customer_rejects_inactive_business() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let mut business = active_business();
        business.soft_delete();

        let result = service
            .create_customer(
                &business,
                CustomerId::new(),
                CustomerName::new("Budi").unwrap(),
                None,
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::BusinessIsDeleted))
        ));
    }

    #[tokio::test]
    async fn create_customer_allows_duplicate_name_in_same_business() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business = active_business();

        create_test_customer(&service, &business, "Budi").await;
        let result = service
            .create_customer(
                &business,
                CustomerId::new(),
                CustomerName::new("Budi").unwrap(),
                None,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn create_customer_with_same_id_is_idempotent() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business = active_business();
        let id = CustomerId::new();

        let (first, first_created) = service
            .create_customer(&business, id, CustomerName::new("Budi").unwrap(), None)
            .await
            .unwrap();
        assert!(first_created);

        let (second, second_created) = service
            .create_customer(&business, id, CustomerName::new("Budi Lain").unwrap(), None)
            .await
            .unwrap();

        assert!(!second_created);
        assert_eq!(second.id(), first.id());
        assert_eq!(second.name(), first.name());
    }

    #[tokio::test]
    async fn list_updated_since_only_returns_customer_from_that_business() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business_a = active_business();
        let business_b = active_business();

        create_test_customer(&service, &business_a, "Budi").await;
        create_test_customer(&service, &business_b, "Siti").await;

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = service
            .list_updated_since(business_a.id(), epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].business_id(), business_a.id());
    }

    #[tokio::test]
    async fn rename_customer_rejects_stale_version() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business = active_business();
        let customer = create_test_customer(&service, &business, "Budi").await;

        let result = service
            .rename_customer(
                customer.id(),
                CustomerName::new("Budi Baru").unwrap(),
                customer.version() + 1,
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::VersionConflict))
        ));
    }

    #[tokio::test]
    async fn update_customer_phone_sets_and_clears_phone() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business = active_business();
        let customer = create_test_customer(&service, &business, "Budi").await;

        let updated = service
            .update_customer_phone(
                customer.id(),
                Some(CustomerPhone::new("081234567890").unwrap()),
                customer.version(),
            )
            .await
            .unwrap();
        assert!(updated.phone().is_some());

        let cleared = service
            .update_customer_phone(customer.id(), None, updated.version())
            .await
            .unwrap();
        assert!(cleared.phone().is_none());
    }

    #[tokio::test]
    async fn delete_customer_marks_as_deleted() {
        let repo = InMemoryCustomerRepository::new();
        let service = CustomerService::new(repo);
        let business = active_business();
        let customer = create_test_customer(&service, &business, "Budi").await;

        service
            .delete_customer(customer.id(), customer.version())
            .await
            .unwrap();

        let stored = service
            .repository
            .find_by_id(customer.id())
            .await
            .unwrap()
            .unwrap();
        assert!(stored.is_deleted());
    }
}
