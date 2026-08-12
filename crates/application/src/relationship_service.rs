use chrono::{DateTime, Utc};
use domain::{
    Business, BusinessId, Customer, Relationship, RelationshipId, RelationshipType, rules,
};

use crate::error::ApplicationError;
use crate::repository::RelationshipRepository;

/// Orkestrasi use-case seputar Relationship.
///
/// Hanya bergantung pada `RelationshipRepository` — `business` yang sudah
/// diambil pemanggil dikirim sebagai parameter, sama seperti
/// `TransactionService` menerima `&Business`.
///
/// Tidak ada `rename_relationship`/`update_relationship` di sini —
/// Relationship memang didesain immutable di layer domain (lihat komentar
/// di `domain::relationship`): hanya `create` dan `delete` (soft delete)
/// yang tersedia.
///
/// `from_customer`/`to_customer` diterima sebagai `&Customer` (BUKAN
/// `CustomerId` mentah) — pemanggil (route) WAJIB mengambil keduanya
/// lebih dulu lewat `CustomerService::get_customer`, supaya validasi
/// `rules::customer_belongs_to_business` bisa dilakukan di sini untuk
/// KEDUA sisi relasi sebelum Relationship dibuat.
///
/// SATU validasi lain masih SENGAJA belum diimplementasikan, konsisten
/// dengan pola yang sama di seluruh Core: pencegahan relationship
/// duplikat — pasangan Customer + jenis yang sama tercatat lebih dari
/// sekali. Bisa ditambahkan nanti kalau memang dibutuhkan.
#[derive(Clone)]
pub struct RelationshipService<R: RelationshipRepository> {
    repository: R,
}

impl<R: RelationshipRepository> RelationshipService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Membuat Relationship baru — idempotent terhadap `id`, alasan dan
    /// kontrak sama seperti `TransactionService::create_transaction`.
    ///
    /// Beda dari `create_transaction`: mengembalikan `ApplicationError`
    /// (bukan cuma infra/business rule biasa) karena `Relationship::with_id`
    /// sendiri bisa gagal (`SelfRelationship`) — propagasi lewat `?` otomatis
    /// lewat `impl From<DomainError> for ApplicationError`.
    pub async fn create_relationship(
        &self,
        business: &Business,
        id: RelationshipId,
        from_customer: &Customer,
        to_customer: &Customer,
        relationship_type: RelationshipType,
    ) -> Result<(Relationship, bool), ApplicationError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_business_is_active(business.is_deleted())?;

        // Info-hiding sengaja: lihat komentar di
        // `TransactionService::create_transaction` — kedua sisi relasi
        // divalidasi, apa pun yang gagal duluan dipetakan ke
        // `CustomerNotFound` yang sama.
        if !rules::customer_belongs_to_business(from_customer.business_id(), business.id())
            || !rules::customer_belongs_to_business(to_customer.business_id(), business.id())
        {
            return Err(ApplicationError::CustomerNotFound);
        }

        let relationship = Relationship::with_id(
            id,
            business.id(),
            from_customer.id(),
            to_customer.id(),
            relationship_type,
        )?;
        self.repository.save(&relationship).await?;
        Ok((relationship, true))
    }

    /// Semua Relationship di bawah satu Business yang berubah sejak
    /// `since` — dipakai endpoint incremental sync nanti (belum dibuat di
    /// tahap ini).
    pub async fn list_updated_since(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Relationship>, ApplicationError> {
        Ok(self
            .repository
            .find_updated_since_by_business(business_id, since)
            .await?)
    }

    pub async fn delete_relationship(
        &self,
        id: RelationshipId,
        expected_version: u32,
    ) -> Result<(), ApplicationError> {
        let mut relationship = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::RelationshipNotFound)?;

        rules::ensure_version_matches(expected_version, relationship.version())?;

        relationship.soft_delete();
        self.repository.save(&relationship).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryRelationshipRepository;
    use domain::{BusinessName, BusinessType, CustomerName, DomainError, Tenant, TenantName};

    fn active_business() -> Business {
        let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        Business::new(
            tenant.id(),
            BusinessName::new("Toko Baju").unwrap(),
            BusinessType::new("retail").unwrap(),
        )
    }

    fn sample_type() -> RelationshipType {
        RelationshipType::new("referral").unwrap()
    }

    fn sample_customer(business: &Business, name: &str) -> Customer {
        Customer::new(business.id(), CustomerName::new(name).unwrap(), None)
    }

    async fn create_test_relationship(
        service: &RelationshipService<InMemoryRelationshipRepository>,
        business: &Business,
        from: &Customer,
        to: &Customer,
    ) -> Relationship {
        service
            .create_relationship(business, RelationshipId::new(), from, to, sample_type())
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_relationship_succeeds_for_active_business() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business = active_business();
        let from = sample_customer(&business, "Budi");
        let to = sample_customer(&business, "Ani");

        let relationship = create_test_relationship(&service, &business, &from, &to).await;

        assert_eq!(relationship.business_id(), business.id());
    }

    #[tokio::test]
    async fn create_relationship_rejects_inactive_business() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let mut business = active_business();
        let from = sample_customer(&business, "Budi");
        let to = sample_customer(&business, "Ani");
        business.soft_delete();

        let result = service
            .create_relationship(&business, RelationshipId::new(), &from, &to, sample_type())
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::BusinessIsDeleted))
        ));
    }

    #[tokio::test]
    async fn create_relationship_rejects_self_relationship() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business = active_business();
        let customer = sample_customer(&business, "Budi");

        let result = service
            .create_relationship(
                &business,
                RelationshipId::new(),
                &customer,
                &customer,
                sample_type(),
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::SelfRelationship))
        ));
    }

    #[tokio::test]
    async fn create_relationship_rejects_customer_from_another_business() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business = active_business();
        let other_business = active_business();
        let from = sample_customer(&business, "Budi");
        // to milik Business LAIN — mensimulasikan client mengirim
        // customer_id yang bukan miliknya.
        let foreign_to = sample_customer(&other_business, "Ani");

        let result = service
            .create_relationship(
                &business,
                RelationshipId::new(),
                &from,
                &foreign_to,
                sample_type(),
            )
            .await;

        assert!(matches!(result, Err(ApplicationError::CustomerNotFound)));
    }

    #[tokio::test]
    async fn create_relationship_with_same_id_is_idempotent() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business = active_business();
        let from = sample_customer(&business, "Budi");
        let to = sample_customer(&business, "Ani");
        let id = RelationshipId::new();

        let (first, first_created) = service
            .create_relationship(&business, id, &from, &to, sample_type())
            .await
            .unwrap();
        assert!(first_created);

        // Retry dengan Id sama TAPI jenis beda — membuktikan idempotency
        // terjadi sebelum apa pun lainnya.
        let (second, second_created) = service
            .create_relationship(
                &business,
                id,
                &from,
                &to,
                RelationshipType::new("sibling").unwrap(),
            )
            .await
            .unwrap();

        assert!(!second_created);
        assert_eq!(second.id(), first.id());
        assert_eq!(second.relationship_type(), first.relationship_type());
    }

    #[tokio::test]
    async fn list_updated_since_only_returns_relationship_from_that_business() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business_a = active_business();
        let business_b = active_business();
        let from_a = sample_customer(&business_a, "Budi");
        let to_a = sample_customer(&business_a, "Ani");
        let from_b = sample_customer(&business_b, "Siti");
        let to_b = sample_customer(&business_b, "Joko");

        create_test_relationship(&service, &business_a, &from_a, &to_a).await;
        create_test_relationship(&service, &business_b, &from_b, &to_b).await;

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = service
            .list_updated_since(business_a.id(), epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].business_id(), business_a.id());
    }

    #[tokio::test]
    async fn delete_relationship_rejects_stale_version() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business = active_business();
        let from = sample_customer(&business, "Budi");
        let to = sample_customer(&business, "Ani");
        let relationship = create_test_relationship(&service, &business, &from, &to).await;

        let result = service
            .delete_relationship(relationship.id(), relationship.version() + 1)
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::VersionConflict))
        ));
    }

    #[tokio::test]
    async fn delete_relationship_marks_as_deleted() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);
        let business = active_business();
        let from = sample_customer(&business, "Budi");
        let to = sample_customer(&business, "Ani");
        let relationship = create_test_relationship(&service, &business, &from, &to).await;

        service
            .delete_relationship(relationship.id(), relationship.version())
            .await
            .unwrap();

        let stored = service
            .repository
            .find_by_id(relationship.id())
            .await
            .unwrap()
            .unwrap();
        assert!(stored.is_deleted());
    }

    #[tokio::test]
    async fn delete_relationship_fails_when_not_found() {
        let repo = InMemoryRelationshipRepository::new();
        let service = RelationshipService::new(repo);

        let result = service.delete_relationship(RelationshipId::new(), 0).await;

        assert!(matches!(
            result,
            Err(ApplicationError::RelationshipNotFound)
        ));
    }
}
