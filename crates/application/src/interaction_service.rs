use chrono::{DateTime, Utc};
use domain::{
    Business, BusinessId, Customer, Interaction, InteractionId, InteractionNote, InteractionType,
    rules,
};

use crate::error::ApplicationError;
use crate::repository::InteractionRepository;

/// Orkestrasi use-case seputar Interaction.
///
/// Hanya bergantung pada `InteractionRepository` — `business` yang sudah
/// diambil pemanggil dikirim sebagai parameter, sama seperti
/// `TransactionService`/`RelationshipService` menerima `&Business`.
///
/// Tidak ada `rename_interaction`/`update_interaction` di sini —
/// Interaction memang didesain immutable di layer domain: hanya `create`
/// dan `delete` (soft delete) yang tersedia.
///
/// `customer` diterima sebagai `&Customer` (BUKAN `CustomerId` mentah) —
/// pemanggil (route) WAJIB mengambilnya lebih dulu lewat
/// `CustomerService::get_customer`, supaya validasi
/// `rules::customer_belongs_to_business` bisa dilakukan di sini SEBELUM
/// Interaction dibuat. Pola sama seperti
/// `TransactionService::create_transaction`/`RelationshipService::create_relationship`.
#[derive(Clone)]
pub struct InteractionService<R: InteractionRepository> {
    repository: R,
}

impl<R: InteractionRepository> InteractionService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Membuat Interaction baru — idempotent terhadap `id`, alasan dan
    /// kontrak sama seperti `TransactionService::create_transaction`.
    pub async fn create_interaction(
        &self,
        business: &Business,
        id: InteractionId,
        customer: &Customer,
        interaction_type: InteractionType,
        note: Option<InteractionNote>,
        occurred_at: DateTime<Utc>,
    ) -> Result<(Interaction, bool), ApplicationError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_business_is_active(business.is_deleted())?;

        // Info-hiding sengaja — lihat komentar di
        // `TransactionService::create_transaction`.
        if !rules::customer_belongs_to_business(customer.business_id(), business.id()) {
            return Err(ApplicationError::CustomerNotFound);
        }

        let interaction = Interaction::with_id(
            id,
            business.id(),
            customer.id(),
            interaction_type,
            note,
            occurred_at,
        );
        self.repository.save(&interaction).await?;
        Ok((interaction, true))
    }

    /// Semua Interaction di bawah satu Business yang berubah sejak
    /// `since` — dipakai endpoint incremental sync.
    pub async fn list_updated_since(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Interaction>, ApplicationError> {
        Ok(self
            .repository
            .find_updated_since_by_business(business_id, since)
            .await?)
    }

    pub async fn delete_interaction(
        &self,
        id: InteractionId,
        expected_version: u32,
    ) -> Result<(), ApplicationError> {
        let mut interaction = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::InteractionNotFound)?;

        rules::ensure_version_matches(expected_version, interaction.version())?;

        interaction.soft_delete();
        self.repository.save(&interaction).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryInteractionRepository;
    use domain::{BusinessName, BusinessType, CustomerName, DomainError, Tenant, TenantName};

    fn active_business() -> Business {
        let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        Business::new(
            tenant.id(),
            BusinessName::new("Klinik A").unwrap(),
            BusinessType::new("clinic").unwrap(),
        )
    }

    fn sample_type() -> InteractionType {
        InteractionType::new("call").unwrap()
    }

    fn sample_customer(business: &Business) -> Customer {
        Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None)
    }

    async fn create_test_interaction(
        service: &InteractionService<InMemoryInteractionRepository>,
        business: &Business,
        customer: &Customer,
    ) -> Interaction {
        service
            .create_interaction(
                business,
                InteractionId::new(),
                customer,
                sample_type(),
                None,
                Utc::now(),
            )
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_interaction_succeeds_for_active_business() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);

        let interaction = create_test_interaction(&service, &business, &customer).await;

        assert_eq!(interaction.business_id(), business.id());
    }

    #[tokio::test]
    async fn create_interaction_rejects_inactive_business() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let mut business = active_business();
        let customer = sample_customer(&business);
        business.soft_delete();

        let result = service
            .create_interaction(
                &business,
                InteractionId::new(),
                &customer,
                sample_type(),
                None,
                Utc::now(),
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::BusinessIsDeleted))
        ));
    }

    #[tokio::test]
    async fn create_interaction_rejects_customer_from_another_business() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business = active_business();
        let other_business = active_business();
        let foreign_customer = sample_customer(&other_business);

        let result = service
            .create_interaction(
                &business,
                InteractionId::new(),
                &foreign_customer,
                sample_type(),
                None,
                Utc::now(),
            )
            .await;

        assert!(matches!(result, Err(ApplicationError::CustomerNotFound)));
    }

    #[tokio::test]
    async fn create_interaction_can_have_a_note() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let note = InteractionNote::new("Follow up minggu depan").unwrap();

        let (interaction, _) = service
            .create_interaction(
                &business,
                InteractionId::new(),
                &customer,
                sample_type(),
                Some(note.clone()),
                Utc::now(),
            )
            .await
            .unwrap();

        assert_eq!(interaction.note(), Some(&note));
    }

    #[tokio::test]
    async fn create_interaction_with_same_id_is_idempotent() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let id = InteractionId::new();

        let (first, first_created) = service
            .create_interaction(&business, id, &customer, sample_type(), None, Utc::now())
            .await
            .unwrap();
        assert!(first_created);

        // Retry dengan Id sama TAPI jenis beda — membuktikan idempotency
        // terjadi sebelum apa pun lainnya.
        let (second, second_created) = service
            .create_interaction(
                &business,
                id,
                &customer,
                InteractionType::new("visit").unwrap(),
                None,
                Utc::now(),
            )
            .await
            .unwrap();

        assert!(!second_created);
        assert_eq!(second.id(), first.id());
        assert_eq!(second.interaction_type(), first.interaction_type());
    }

    #[tokio::test]
    async fn list_updated_since_only_returns_interaction_from_that_business() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business_a = active_business();
        let business_b = active_business();
        let customer_a = sample_customer(&business_a);
        let customer_b = sample_customer(&business_b);

        create_test_interaction(&service, &business_a, &customer_a).await;
        create_test_interaction(&service, &business_b, &customer_b).await;

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = service
            .list_updated_since(business_a.id(), epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].business_id(), business_a.id());
    }

    #[tokio::test]
    async fn delete_interaction_rejects_stale_version() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let interaction = create_test_interaction(&service, &business, &customer).await;

        let result = service
            .delete_interaction(interaction.id(), interaction.version() + 1)
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::VersionConflict))
        ));
    }

    #[tokio::test]
    async fn delete_interaction_marks_as_deleted() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business);
        let interaction = create_test_interaction(&service, &business, &customer).await;

        service
            .delete_interaction(interaction.id(), interaction.version())
            .await
            .unwrap();

        let stored = service
            .repository
            .find_by_id(interaction.id())
            .await
            .unwrap()
            .unwrap();
        assert!(stored.is_deleted());
    }

    #[tokio::test]
    async fn delete_interaction_fails_when_not_found() {
        let repo = InMemoryInteractionRepository::new();
        let service = InteractionService::new(repo);

        let result = service.delete_interaction(InteractionId::new(), 0).await;

        assert!(matches!(result, Err(ApplicationError::InteractionNotFound)));
    }
}
