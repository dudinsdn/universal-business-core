use chrono::{DateTime, Utc};
use domain::{Business, BusinessId, Customer, TransactionId, rules as domain_rules};

use crate::error::{ServiceOrderError, WorkshopError};
use crate::repository::ServiceOrderRepository;
use crate::rules;
use crate::service_order::{ServiceOrder, ServiceOrderDescription, ServiceOrderId};

/// Orkestrasi use-case seputar ServiceOrder.
///
/// Hanya bergantung pada `ServiceOrderRepository` — `business` yang sudah
/// diambil pemanggil (lewat `application::BusinessService::get_business`
/// di Core) dikirim sebagai parameter, pola sama persis seperti
/// `CustomerService`/`TransactionService` menerima `&Business`.
///
/// `customer` diterima sebagai `&Customer` (BUKAN `CustomerId` mentah) —
/// pemanggil (route) WAJIB mengambilnya lebih dulu lewat
/// `application::CustomerService::get_customer`, supaya validasi
/// `domain::rules::customer_belongs_to_business` bisa dilakukan di sini
/// SEBELUM ServiceOrder dibuat. Pola sama persis seperti
/// `TransactionService`/`RelationshipService`/`InteractionService` di
/// Core (gap #3: validasi customer_id lintas-aggregate).
#[derive(Clone)]
pub struct ServiceOrderService<R: ServiceOrderRepository> {
    repository: R,
}

impl<R: ServiceOrderRepository> ServiceOrderService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Membuat ServiceOrder baru — idempotent terhadap `id`, alasan dan
    /// kontrak sama seperti `TransactionService::create_transaction` di
    /// Core.
    pub async fn create_service_order(
        &self,
        business: &Business,
        id: ServiceOrderId,
        customer: &Customer,
        description: ServiceOrderDescription,
    ) -> Result<(ServiceOrder, bool), ServiceOrderError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_business_is_active(business.is_deleted())?;

        // Info-hiding sengaja — lihat komentar di
        // `WorkshopError::CustomerNotFound` dan pola yang sama di Core
        // (`TransactionService::create_transaction`).
        if !domain_rules::customer_belongs_to_business(customer.business_id(), business.id()) {
            return Err(WorkshopError::CustomerNotFound.into());
        }

        let order = ServiceOrder::with_id(id, business.id(), customer.id(), description);
        self.repository.save(&order).await?;
        Ok((order, true))
    }

    pub async fn get_service_order(
        &self,
        id: ServiceOrderId,
    ) -> Result<ServiceOrder, ServiceOrderError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(ServiceOrderError::ServiceOrderNotFound)
    }

    /// Semua ServiceOrder di bawah satu Business yang berubah sejak
    /// `since` — dipakai endpoint incremental sync nanti.
    pub async fn list_updated_since(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<ServiceOrder>, ServiceOrderError> {
        Ok(self
            .repository
            .find_updated_since_by_business(business_id, since)
            .await?)
    }

    /// Received -> InProgress.
    pub async fn start_service_order(
        &self,
        id: ServiceOrderId,
        expected_version: u32,
    ) -> Result<ServiceOrder, ServiceOrderError> {
        let mut order = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ServiceOrderError::ServiceOrderNotFound)?;

        rules::ensure_version_matches(expected_version, order.version())?;
        order.start()?;
        self.repository.save(&order).await?;
        Ok(order)
    }

    /// InProgress -> Completed, dengan referensi opsional ke Transaction
    /// (Core) yang menagihnya.
    pub async fn complete_service_order(
        &self,
        id: ServiceOrderId,
        expected_version: u32,
        transaction_id: Option<TransactionId>,
    ) -> Result<ServiceOrder, ServiceOrderError> {
        let mut order = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ServiceOrderError::ServiceOrderNotFound)?;

        rules::ensure_version_matches(expected_version, order.version())?;
        order.complete(transaction_id)?;
        self.repository.save(&order).await?;
        Ok(order)
    }

    /// Received/InProgress -> Cancelled.
    pub async fn cancel_service_order(
        &self,
        id: ServiceOrderId,
        expected_version: u32,
    ) -> Result<ServiceOrder, ServiceOrderError> {
        let mut order = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ServiceOrderError::ServiceOrderNotFound)?;

        rules::ensure_version_matches(expected_version, order.version())?;
        order.cancel()?;
        self.repository.save(&order).await?;
        Ok(order)
    }

    /// Soft delete — untuk "salah input", bukan pengganti `cancel()`.
    pub async fn delete_service_order(
        &self,
        id: ServiceOrderId,
        expected_version: u32,
    ) -> Result<(), ServiceOrderError> {
        let mut order = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ServiceOrderError::ServiceOrderNotFound)?;

        rules::ensure_version_matches(expected_version, order.version())?;
        order.soft_delete();
        self.repository.save(&order).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryServiceOrderRepository;
    use domain::{BusinessName, BusinessType, CustomerName, Tenant, TenantName};

    fn active_business() -> Business {
        let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        Business::new(
            tenant.id(),
            BusinessName::new("Bengkel Jaya").unwrap(),
            BusinessType::new("workshop").unwrap(),
        )
    }

    fn sample_description() -> ServiceOrderDescription {
        ServiceOrderDescription::new("Ganti oli dan servis rem").unwrap()
    }

    fn sample_customer(business: &Business) -> Customer {
        Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None)
    }

    async fn create_test_order(
        service: &ServiceOrderService<InMemoryServiceOrderRepository>,
        business: &Business,
        customer: &Customer,
    ) -> ServiceOrder {
        service
            .create_service_order(
                business,
                ServiceOrderId::new(),
                customer,
                sample_description(),
            )
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_service_order_succeeds_for_active_business() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);

        let order = create_test_order(&service, &business, &customer).await;

        assert_eq!(order.business_id(), business.id());
    }

    #[tokio::test]
    async fn create_service_order_rejects_inactive_business() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let mut business = active_business();
        let customer = sample_customer(&business);
        business.soft_delete();

        let result = service
            .create_service_order(
                &business,
                ServiceOrderId::new(),
                &customer,
                sample_description(),
            )
            .await;

        assert_eq!(
            result,
            Err(ServiceOrderError::Workshop(
                WorkshopError::BusinessIsDeleted
            ))
        );
    }

    #[tokio::test]
    async fn create_service_order_rejects_customer_from_another_business() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let other_business = active_business();
        let foreign_customer = sample_customer(&other_business);

        let result = service
            .create_service_order(
                &business,
                ServiceOrderId::new(),
                &foreign_customer,
                sample_description(),
            )
            .await;

        assert_eq!(
            result,
            Err(ServiceOrderError::Workshop(WorkshopError::CustomerNotFound))
        );
    }

    #[tokio::test]
    async fn create_service_order_with_same_id_is_idempotent() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let id = ServiceOrderId::new();

        let (first, first_created) = service
            .create_service_order(&business, id, &customer, sample_description())
            .await
            .unwrap();
        assert!(first_created);

        // Retry dengan Id sama TAPI deskripsi beda — membuktikan
        // idempotency terjadi sebelum apa pun lainnya, pola sama seperti
        // Core.
        let (second, second_created) = service
            .create_service_order(
                &business,
                id,
                &customer,
                ServiceOrderDescription::new("Deskripsi lain").unwrap(),
            )
            .await
            .unwrap();

        assert!(!second_created);
        assert_eq!(second.id(), first.id());
        assert_eq!(second.description(), first.description());
    }

    #[tokio::test]
    async fn get_service_order_fails_when_not_found() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);

        let result = service.get_service_order(ServiceOrderId::new()).await;

        assert_eq!(result, Err(ServiceOrderError::ServiceOrderNotFound));
    }

    #[tokio::test]
    async fn start_then_complete_happy_path() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let order = create_test_order(&service, &business, &customer).await;

        let started = service
            .start_service_order(order.id(), order.version())
            .await
            .unwrap();
        assert_eq!(started.status(), crate::ServiceOrderStatus::InProgress);
    }

    #[tokio::test]
    async fn start_service_order_rejects_stale_version() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let order = create_test_order(&service, &business, &customer).await;

        let result = service
            .start_service_order(order.id(), order.version() + 1)
            .await;

        assert_eq!(
            result,
            Err(ServiceOrderError::Workshop(WorkshopError::VersionConflict))
        );
    }

    #[tokio::test]
    async fn complete_service_order_stores_transaction_link() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let order = create_test_order(&service, &business, &customer).await;

        let started = service
            .start_service_order(order.id(), order.version())
            .await
            .unwrap();

        let transaction_id = TransactionId::new();
        let completed = service
            .complete_service_order(order.id(), started.version(), Some(transaction_id))
            .await
            .unwrap();

        assert_eq!(completed.transaction_id(), Some(transaction_id));
    }

    #[tokio::test]
    async fn complete_service_order_rejects_directly_from_received() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let order = create_test_order(&service, &business, &customer).await;

        let result = service
            .complete_service_order(order.id(), order.version(), None)
            .await;

        assert!(matches!(
            result,
            Err(ServiceOrderError::Workshop(
                WorkshopError::InvalidTransition { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn cancel_service_order_marks_cancelled() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let order = create_test_order(&service, &business, &customer).await;

        let cancelled = service
            .cancel_service_order(order.id(), order.version())
            .await
            .unwrap();

        assert_eq!(cancelled.status(), crate::ServiceOrderStatus::Cancelled);
    }

    #[tokio::test]
    async fn delete_service_order_marks_as_deleted() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let order = create_test_order(&service, &business, &customer).await;

        service
            .delete_service_order(order.id(), order.version())
            .await
            .unwrap();

        let stored = service.get_service_order(order.id()).await.unwrap();
        assert!(stored.is_deleted());
    }

    #[tokio::test]
    async fn list_updated_since_only_returns_orders_from_that_business() {
        let repo = InMemoryServiceOrderRepository::new();
        let service = ServiceOrderService::new(repo);
        let business_a = active_business();
        let business_b = active_business();
        let customer_a = sample_customer(&business_a);
        let customer_b = sample_customer(&business_b);

        create_test_order(&service, &business_a, &customer_a).await;
        create_test_order(&service, &business_b, &customer_b).await;

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = service
            .list_updated_since(business_a.id(), epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].business_id(), business_a.id());
    }
}
