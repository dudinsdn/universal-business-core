use chrono::{DateTime, Utc};
use domain::{
    Business, BusinessId, CustomerId, Transaction, TransactionAmount, TransactionId,
    TransactionKind, rules,
};

use crate::error::ApplicationError;
use crate::repository::TransactionRepository;

/// Orkestrasi use-case seputar Transaction.
///
/// Hanya bergantung pada `TransactionRepository` — `business` yang sudah
/// diambil pemanggil dikirim sebagai parameter, sama seperti
/// `CustomerService` menerima `&Business`.
///
/// Tidak ada `rename_transaction`/`update_transaction` di sini — Transaction
/// memang didesain immutable di layer domain (lihat komentar di
/// `domain::transaction`): hanya `create` dan `delete` (soft delete) yang
/// tersedia.
///
/// Validasi "kalau `customer_id` diisi, harus milik Business yang sama"
/// SENGAJA belum diimplementasikan — belum ada keputusan eksplisit soal
/// itu. Ditambahkan nanti kalau memang dibutuhkan (butuh
/// `CustomerRepository` sebagai dependency tambahan, konsisten dengan pola
/// `TenantService::delete_tenant` yang menerima `BusinessRepository`
/// sebagai parameter eksplisit untuk operasi lintas-aggregate).
#[derive(Clone)]
pub struct TransactionService<R: TransactionRepository> {
    repository: R,
}

impl<R: TransactionRepository> TransactionService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Membuat Transaction baru — idempotent terhadap `id`, alasan dan
    /// kontrak sama seperti `CustomerService::create_customer`.
    pub async fn create_transaction(
        &self,
        business: &Business,
        id: TransactionId,
        customer_id: Option<CustomerId>,
        kind: TransactionKind,
        amount: TransactionAmount,
        occurred_at: DateTime<Utc>,
    ) -> Result<(Transaction, bool), ApplicationError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_business_is_active(business.is_deleted())?;

        let transaction =
            Transaction::with_id(id, business.id(), customer_id, kind, amount, occurred_at);
        self.repository.save(&transaction).await?;
        Ok((transaction, true))
    }

    /// Semua Transaction di bawah satu Business yang berubah sejak `since`
    /// — dipakai endpoint incremental sync nanti (belum dibuat di tahap
    /// ini).
    pub async fn list_updated_since(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Transaction>, ApplicationError> {
        Ok(self
            .repository
            .find_updated_since_by_business(business_id, since)
            .await?)
    }

    pub async fn delete_transaction(
        &self,
        id: TransactionId,
        expected_version: u32,
    ) -> Result<(), ApplicationError> {
        let mut transaction = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::TransactionNotFound)?;

        rules::ensure_version_matches(expected_version, transaction.version())?;

        transaction.soft_delete();
        self.repository.save(&transaction).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryTransactionRepository;
    use domain::{BusinessName, BusinessType, DomainError, Tenant, TenantName};

    fn active_business() -> Business {
        let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        Business::new(
            tenant.id(),
            BusinessName::new("Toko Baju").unwrap(),
            BusinessType::new("retail").unwrap(),
        )
    }

    fn sample_kind() -> TransactionKind {
        TransactionKind::new("sale").unwrap()
    }

    fn sample_amount() -> TransactionAmount {
        TransactionAmount::new(50_000).unwrap()
    }

    async fn create_test_transaction(
        service: &TransactionService<InMemoryTransactionRepository>,
        business: &Business,
    ) -> Transaction {
        service
            .create_transaction(
                business,
                TransactionId::new(),
                None,
                sample_kind(),
                sample_amount(),
                Utc::now(),
            )
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn create_transaction_succeeds_for_active_business() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();

        let transaction = create_test_transaction(&service, &business).await;

        assert_eq!(transaction.business_id(), business.id());
    }

    #[tokio::test]
    async fn create_transaction_rejects_inactive_business() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let mut business = active_business();
        business.soft_delete();

        let result = service
            .create_transaction(
                &business,
                TransactionId::new(),
                None,
                sample_kind(),
                sample_amount(),
                Utc::now(),
            )
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::BusinessIsDeleted))
        ));
    }

    #[tokio::test]
    async fn create_transaction_with_same_id_is_idempotent() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();
        let id = TransactionId::new();

        let (first, first_created) = service
            .create_transaction(
                &business,
                id,
                None,
                sample_kind(),
                sample_amount(),
                Utc::now(),
            )
            .await
            .unwrap();
        assert!(first_created);

        // Retry dengan Id sama TAPI amount beda — membuktikan idempotency
        // terjadi sebelum apa pun lainnya, sama seperti pola Business/
        // Customer.
        let (second, second_created) = service
            .create_transaction(
                &business,
                id,
                None,
                sample_kind(),
                TransactionAmount::new(999_999).unwrap(),
                Utc::now(),
            )
            .await
            .unwrap();

        assert!(!second_created);
        assert_eq!(second.id(), first.id());
        assert_eq!(second.amount(), first.amount());
    }

    #[tokio::test]
    async fn create_transaction_can_be_linked_to_a_customer() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();
        let customer_id = CustomerId::new();

        let (transaction, _) = service
            .create_transaction(
                &business,
                TransactionId::new(),
                Some(customer_id),
                sample_kind(),
                sample_amount(),
                Utc::now(),
            )
            .await
            .unwrap();

        assert_eq!(transaction.customer_id(), Some(customer_id));
    }

    #[tokio::test]
    async fn list_updated_since_only_returns_transaction_from_that_business() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business_a = active_business();
        let business_b = active_business();

        create_test_transaction(&service, &business_a).await;
        create_test_transaction(&service, &business_b).await;

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = service
            .list_updated_since(business_a.id(), epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].business_id(), business_a.id());
    }

    #[tokio::test]
    async fn delete_transaction_rejects_stale_version() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();
        let transaction = create_test_transaction(&service, &business).await;

        let result = service
            .delete_transaction(transaction.id(), transaction.version() + 1)
            .await;

        assert!(matches!(
            result,
            Err(ApplicationError::Domain(DomainError::VersionConflict))
        ));
    }

    #[tokio::test]
    async fn delete_transaction_marks_as_deleted() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();
        let transaction = create_test_transaction(&service, &business).await;

        service
            .delete_transaction(transaction.id(), transaction.version())
            .await
            .unwrap();

        let stored = service
            .repository
            .find_by_id(transaction.id())
            .await
            .unwrap()
            .unwrap();
        assert!(stored.is_deleted());
    }

    #[tokio::test]
    async fn delete_transaction_fails_when_not_found() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);

        let result = service.delete_transaction(TransactionId::new(), 0).await;

        assert!(matches!(result, Err(ApplicationError::TransactionNotFound)));
    }
}
