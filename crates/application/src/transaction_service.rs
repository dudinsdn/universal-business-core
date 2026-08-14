use chrono::{DateTime, Utc};
use domain::{
    Business, BusinessId, Customer, Transaction, TransactionAmount, TransactionId, TransactionKind,
    rules,
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
/// `customer` diterima sebagai `Option<&Customer>` (BUKAN `Option<CustomerId>`
/// mentah) — pemanggil (route) WAJIB mengambilnya lebih dulu lewat
/// `CustomerService::get_customer`, supaya validasi
/// `rules::customer_belongs_to_business` bisa dilakukan di sini SEBELUM
/// Transaction dibuat. Ini menutup gap: sebelumnya client bisa mengirim
/// `customer_id` milik Business/Tenant lain dan tetap diterima begitu
/// saja.
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
    ///
    /// PENTING soal urutan: pengecekan idempotency ("apakah `id` sudah
    /// ada") tetap paling pertama — sama seperti `BusinessService::
    /// create_business` — supaya retry dengan payload identik tidak
    /// salah ditolak. Validasi kepemilikan Customer dicek SETELAH
    /// idempotency, SEBELUM entity dibuat.
    pub async fn create_transaction(
        &self,
        business: &Business,
        id: TransactionId,
        customer: Option<&Customer>,
        kind: TransactionKind,
        amount: TransactionAmount,
        occurred_at: DateTime<Utc>,
    ) -> Result<(Transaction, bool), ApplicationError> {
        if let Some(existing) = self.repository.find_by_id(id).await? {
            return Ok((existing, false));
        }

        rules::ensure_business_is_active(business.is_deleted())?;

        if let Some(customer) = customer {
            // Info-hiding sengaja: kalau Customer ada tapi milik
            // Business/Tenant lain, dipetakan ke error yang SAMA seperti
            // "tidak ditemukan" (bukan error khusus "bukan milik Anda")
            // — supaya client tidak bisa membedakan "customer_id salah"
            // dari "customer_id itu milik tenant lain" (lihat diskusi
            // desain gap #3).
            if !rules::customer_belongs_to_business(customer.business_id(), business.id()) {
                return Err(ApplicationError::CustomerNotFound);
            }
        }

        let customer_id = customer.map(|c| c.id());
        let transaction =
            Transaction::with_id(id, business.id(), customer_id, kind, amount, occurred_at);
        self.repository.save(&transaction).await?;
        Ok((transaction, true))
    }

    /// Mengambil satu Transaction berdasarkan id. Dibutuhkan pemanggil (mis.
    /// Capability Workshop) yang perlu objek `Transaction` utuh sebelum
    /// menautkannya ke entity lain, supaya bisa divalidasi benar-benar
    /// milik `Business` yang sama sebelum ditautkan. Pola sama seperti
    /// `BusinessService::get_business`/`CustomerService::get_customer`.
    pub async fn get_transaction(
        &self,
        id: TransactionId,
    ) -> Result<Transaction, ApplicationError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(ApplicationError::TransactionNotFound)
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
    use domain::{BusinessName, BusinessType, CustomerName, DomainError, Tenant, TenantName};

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
        let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);

        let (transaction, _) = service
            .create_transaction(
                &business,
                TransactionId::new(),
                Some(&customer),
                sample_kind(),
                sample_amount(),
                Utc::now(),
            )
            .await
            .unwrap();

        assert_eq!(transaction.customer_id(), Some(customer.id()));
    }

    #[tokio::test]
    async fn create_transaction_rejects_customer_from_another_business() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();
        let other_business = active_business();
        // Customer ini sengaja dibuat di bawah Business LAIN — mensimulasikan
        // client mengirim customer_id yang bukan miliknya.
        let foreign_customer = Customer::new(
            other_business.id(),
            CustomerName::new("Budi").unwrap(),
            None,
        );

        let result = service
            .create_transaction(
                &business,
                TransactionId::new(),
                Some(&foreign_customer),
                sample_kind(),
                sample_amount(),
                Utc::now(),
            )
            .await;

        assert!(matches!(result, Err(ApplicationError::CustomerNotFound)));
    }

    #[tokio::test]
    async fn get_transaction_returns_saved_transaction() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);
        let business = active_business();
        let created = create_test_transaction(&service, &business).await;

        let fetched = service.get_transaction(created.id()).await.unwrap();

        assert_eq!(fetched.id(), created.id());
    }

    #[tokio::test]
    async fn get_transaction_fails_when_not_found() {
        let repo = InMemoryTransactionRepository::new();
        let service = TransactionService::new(repo);

        let result = service.get_transaction(TransactionId::new()).await;

        assert!(matches!(result, Err(ApplicationError::TransactionNotFound)));
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
