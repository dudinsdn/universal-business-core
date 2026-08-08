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

use chrono::{DateTime, Utc};
use domain::{
    Business, BusinessId, BusinessName, Customer, CustomerId, Interaction, InteractionId,
    Relationship, RelationshipId, Tenant, TenantId, Transaction, TransactionId,
};

use crate::error::RepositoryError;
use crate::repository::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    TenantRepository, TransactionRepository,
};

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

    async fn find_updated_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Tenant>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<Tenant> = data
            .values()
            .filter(|tenant| tenant.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|tenant| tenant.updated_at());
        Ok(result)
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

    async fn find_updated_since_by_tenant(
        &self,
        tenant_id: TenantId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Business>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<Business> = data
            .values()
            .filter(|b| b.tenant_id() == tenant_id && b.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|b| b.updated_at());
        Ok(result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryCustomerRepository {
    data: Arc<Mutex<HashMap<CustomerId, Customer>>>,
}

impl InMemoryCustomerRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CustomerRepository for InMemoryCustomerRepository {
    async fn find_by_id(&self, id: CustomerId) -> Result<Option<Customer>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn save(&self, customer: &Customer) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&customer.id()) {
            let expected_previous_version = customer.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(customer.id(), customer.clone());
        Ok(())
    }

    async fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Customer>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<Customer> = data
            .values()
            .filter(|c| c.business_id() == business_id && c.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|c| c.updated_at());
        Ok(result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTransactionRepository {
    data: Arc<Mutex<HashMap<TransactionId, Transaction>>>,
}

impl InMemoryTransactionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TransactionRepository for InMemoryTransactionRepository {
    async fn find_by_id(&self, id: TransactionId) -> Result<Option<Transaction>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn save(&self, transaction: &Transaction) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&transaction.id()) {
            let expected_previous_version = transaction.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(transaction.id(), transaction.clone());
        Ok(())
    }

    async fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Transaction>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<Transaction> = data
            .values()
            .filter(|t| t.business_id() == business_id && t.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|t| t.updated_at());
        Ok(result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRelationshipRepository {
    data: Arc<Mutex<HashMap<RelationshipId, Relationship>>>,
}

impl InMemoryRelationshipRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RelationshipRepository for InMemoryRelationshipRepository {
    async fn find_by_id(
        &self,
        id: RelationshipId,
    ) -> Result<Option<Relationship>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn save(&self, relationship: &Relationship) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&relationship.id()) {
            let expected_previous_version = relationship.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(relationship.id(), relationship.clone());
        Ok(())
    }

    async fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Relationship>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<Relationship> = data
            .values()
            .filter(|r| r.business_id() == business_id && r.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|r| r.updated_at());
        Ok(result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryInteractionRepository {
    data: Arc<Mutex<HashMap<InteractionId, Interaction>>>,
}

impl InMemoryInteractionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl InteractionRepository for InMemoryInteractionRepository {
    async fn find_by_id(&self, id: InteractionId) -> Result<Option<Interaction>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn save(&self, interaction: &Interaction) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&interaction.id()) {
            let expected_previous_version = interaction.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(interaction.id(), interaction.clone());
        Ok(())
    }

    async fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Interaction>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<Interaction> = data
            .values()
            .filter(|i| i.business_id() == business_id && i.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|i| i.updated_at());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        CustomerName, InteractionType, RelationshipType, TenantName, TransactionAmount,
        TransactionKind,
    };

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

    #[tokio::test]
    async fn find_updated_since_only_returns_tenants_changed_after_cursor() {
        let repo = InMemoryTenantRepository::new();
        let old_tenant = Tenant::new(TenantName::new("Tenant Lama").unwrap());
        repo.save(&old_tenant).await.unwrap();

        let cursor = Utc::now();

        let new_tenant = Tenant::new(TenantName::new("Tenant Baru").unwrap());
        repo.save(&new_tenant).await.unwrap();

        let changed = repo.find_updated_since(cursor).await.unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), new_tenant.id());
    }

    #[tokio::test]
    async fn find_updated_since_includes_soft_deleted_tenants() {
        let repo = InMemoryTenantRepository::new();
        let mut tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
        repo.save(&tenant).await.unwrap();

        let cursor = Utc::now();

        // Client offline harus tahu soal penghapusan juga, bukan cuma
        // perubahan pada entity yang masih aktif.
        tenant.soft_delete();
        repo.save(&tenant).await.unwrap();

        let changed = repo.find_updated_since(cursor).await.unwrap();

        assert_eq!(changed.len(), 1);
        assert!(changed[0].is_deleted());
    }

    #[tokio::test]
    async fn save_detects_concurrent_version_conflict_for_customer() {
        let repo = InMemoryCustomerRepository::new();
        let mut customer =
            Customer::new(BusinessId::new(), CustomerName::new("Budi").unwrap(), None);
        repo.save(&customer).await.unwrap();

        // Dua "pembaca" sama-sama mulai dari data versi 0.
        let mut stale_copy = customer.clone();

        // Salah satu menang duluan: rename, jadi versi 1, berhasil disimpan.
        customer.rename(CustomerName::new("Budi Santoso").unwrap());
        repo.save(&customer).await.unwrap();

        // Yang satu lagi telat: masih berdasarkan data versi 0 — harus
        // ditolak sebagai conflict.
        stale_copy.rename(CustomerName::new("Budi Telat").unwrap());
        let result = repo.save(&stale_copy).await;

        assert_eq!(result, Err(RepositoryError::VersionConflict));
    }

    #[tokio::test]
    async fn find_updated_since_by_business_only_returns_customers_changed_after_cursor() {
        let repo = InMemoryCustomerRepository::new();
        let business_id = BusinessId::new();

        let old_customer =
            Customer::new(business_id, CustomerName::new("Budi Lama").unwrap(), None);
        repo.save(&old_customer).await.unwrap();

        let cursor = Utc::now();

        let new_customer =
            Customer::new(business_id, CustomerName::new("Budi Baru").unwrap(), None);
        repo.save(&new_customer).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), new_customer.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_excludes_other_businesses() {
        let repo = InMemoryCustomerRepository::new();
        let business_a = BusinessId::new();
        let business_b = BusinessId::new();

        let customer_a = Customer::new(business_a, CustomerName::new("Budi A").unwrap(), None);
        let customer_b = Customer::new(business_b, CustomerName::new("Budi B").unwrap(), None);
        repo.save(&customer_a).await.unwrap();
        repo.save(&customer_b).await.unwrap();

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = repo
            .find_updated_since_by_business(business_a, epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), customer_a.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_includes_soft_deleted_customers() {
        let repo = InMemoryCustomerRepository::new();
        let business_id = BusinessId::new();

        let mut customer = Customer::new(business_id, CustomerName::new("Budi").unwrap(), None);
        repo.save(&customer).await.unwrap();

        let cursor = Utc::now();

        // Client offline harus tahu soal penghapusan juga.
        customer.soft_delete();
        repo.save(&customer).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert!(changed[0].is_deleted());
    }

    #[tokio::test]
    async fn save_detects_concurrent_version_conflict_for_transaction() {
        let repo = InMemoryTransactionRepository::new();
        let mut transaction = Transaction::new(
            BusinessId::new(),
            None,
            TransactionKind::new("sale").unwrap(),
            TransactionAmount::new(10_000).unwrap(),
            Utc::now(),
        );
        repo.save(&transaction).await.unwrap();

        // Dua "pembaca" sama-sama mulai dari data versi 0.
        let mut stale_copy = transaction.clone();

        // Salah satu menang duluan: soft delete, jadi versi 1, berhasil
        // disimpan.
        transaction.soft_delete();
        repo.save(&transaction).await.unwrap();

        // Yang telat masih berdasarkan versi 0 saat mulai mutasi — mencoba
        // soft delete juga (jadi versi 1 di sisinya sendiri), tapi versi 0 di
        // penyimpanan sudah tidak ada lagi — harus ditolak sebagai conflict.
        stale_copy.soft_delete();
        let result = repo.save(&stale_copy).await;

        assert_eq!(result, Err(RepositoryError::VersionConflict));
    }

    #[tokio::test]
    async fn find_updated_since_by_business_only_returns_transactions_changed_after_cursor() {
        let repo = InMemoryTransactionRepository::new();
        let business_id = BusinessId::new();

        let old_transaction = Transaction::new(
            business_id,
            None,
            TransactionKind::new("sale").unwrap(),
            TransactionAmount::new(10_000).unwrap(),
            Utc::now(),
        );
        repo.save(&old_transaction).await.unwrap();

        let cursor = Utc::now();

        let new_transaction = Transaction::new(
            business_id,
            None,
            TransactionKind::new("sale").unwrap(),
            TransactionAmount::new(20_000).unwrap(),
            Utc::now(),
        );
        repo.save(&new_transaction).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), new_transaction.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_excludes_other_businesses_for_transaction() {
        let repo = InMemoryTransactionRepository::new();
        let business_a = BusinessId::new();
        let business_b = BusinessId::new();

        let transaction_a = Transaction::new(
            business_a,
            None,
            TransactionKind::new("sale").unwrap(),
            TransactionAmount::new(10_000).unwrap(),
            Utc::now(),
        );
        let transaction_b = Transaction::new(
            business_b,
            None,
            TransactionKind::new("sale").unwrap(),
            TransactionAmount::new(10_000).unwrap(),
            Utc::now(),
        );
        repo.save(&transaction_a).await.unwrap();
        repo.save(&transaction_b).await.unwrap();

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = repo
            .find_updated_since_by_business(business_a, epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), transaction_a.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_includes_soft_deleted_transactions() {
        let repo = InMemoryTransactionRepository::new();
        let business_id = BusinessId::new();

        let mut transaction = Transaction::new(
            business_id,
            None,
            TransactionKind::new("sale").unwrap(),
            TransactionAmount::new(10_000).unwrap(),
            Utc::now(),
        );
        repo.save(&transaction).await.unwrap();

        let cursor = Utc::now();

        // Client offline harus tahu soal penghapusan juga.
        transaction.soft_delete();
        repo.save(&transaction).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert!(changed[0].is_deleted());
    }

    #[tokio::test]
    async fn save_detects_concurrent_version_conflict_for_relationship() {
        let repo = InMemoryRelationshipRepository::new();
        let mut relationship = Relationship::new(
            BusinessId::new(),
            CustomerId::new(),
            CustomerId::new(),
            RelationshipType::new("referral").unwrap(),
        )
        .unwrap();
        repo.save(&relationship).await.unwrap();

        // Dua "pembaca" sama-sama mulai dari data versi 0.
        let mut stale_copy = relationship.clone();

        // Salah satu menang duluan: soft delete, jadi versi 1, berhasil
        // disimpan.
        relationship.soft_delete();
        repo.save(&relationship).await.unwrap();

        // Yang telat masih berdasarkan versi 0 saat mulai mutasi — mencoba
        // soft delete juga, tapi versi 0 di penyimpanan sudah tidak ada
        // lagi — harus ditolak sebagai conflict.
        stale_copy.soft_delete();
        let result = repo.save(&stale_copy).await;

        assert_eq!(result, Err(RepositoryError::VersionConflict));
    }

    #[tokio::test]
    async fn find_updated_since_by_business_only_returns_relationships_changed_after_cursor() {
        let repo = InMemoryRelationshipRepository::new();
        let business_id = BusinessId::new();

        let old_relationship = Relationship::new(
            business_id,
            CustomerId::new(),
            CustomerId::new(),
            RelationshipType::new("referral").unwrap(),
        )
        .unwrap();
        repo.save(&old_relationship).await.unwrap();

        let cursor = Utc::now();

        let new_relationship = Relationship::new(
            business_id,
            CustomerId::new(),
            CustomerId::new(),
            RelationshipType::new("referral").unwrap(),
        )
        .unwrap();
        repo.save(&new_relationship).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), new_relationship.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_excludes_other_businesses_for_relationship() {
        let repo = InMemoryRelationshipRepository::new();
        let business_a = BusinessId::new();
        let business_b = BusinessId::new();

        let relationship_a = Relationship::new(
            business_a,
            CustomerId::new(),
            CustomerId::new(),
            RelationshipType::new("referral").unwrap(),
        )
        .unwrap();
        let relationship_b = Relationship::new(
            business_b,
            CustomerId::new(),
            CustomerId::new(),
            RelationshipType::new("referral").unwrap(),
        )
        .unwrap();
        repo.save(&relationship_a).await.unwrap();
        repo.save(&relationship_b).await.unwrap();

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = repo
            .find_updated_since_by_business(business_a, epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), relationship_a.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_includes_soft_deleted_relationships() {
        let repo = InMemoryRelationshipRepository::new();
        let business_id = BusinessId::new();

        let mut relationship = Relationship::new(
            business_id,
            CustomerId::new(),
            CustomerId::new(),
            RelationshipType::new("referral").unwrap(),
        )
        .unwrap();
        repo.save(&relationship).await.unwrap();

        let cursor = Utc::now();

        // Client offline harus tahu soal penghapusan juga.
        relationship.soft_delete();
        repo.save(&relationship).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert!(changed[0].is_deleted());
    }

    #[tokio::test]
    async fn save_detects_concurrent_version_conflict_for_interaction() {
        let repo = InMemoryInteractionRepository::new();
        let mut interaction = Interaction::new(
            BusinessId::new(),
            CustomerId::new(),
            InteractionType::new("call").unwrap(),
            None,
            Utc::now(),
        );
        repo.save(&interaction).await.unwrap();

        // Dua "pembaca" sama-sama mulai dari data versi 0.
        let mut stale_copy = interaction.clone();

        // Salah satu menang duluan: soft delete, jadi versi 1, berhasil
        // disimpan.
        interaction.soft_delete();
        repo.save(&interaction).await.unwrap();

        // Yang telat masih berdasarkan versi 0 saat mulai mutasi — mencoba
        // soft delete juga, tapi versi 0 di penyimpanan sudah tidak ada
        // lagi — harus ditolak sebagai conflict.
        stale_copy.soft_delete();
        let result = repo.save(&stale_copy).await;

        assert_eq!(result, Err(RepositoryError::VersionConflict));
    }

    #[tokio::test]
    async fn find_updated_since_by_business_only_returns_interactions_changed_after_cursor() {
        let repo = InMemoryInteractionRepository::new();
        let business_id = BusinessId::new();

        let old_interaction = Interaction::new(
            business_id,
            CustomerId::new(),
            InteractionType::new("call").unwrap(),
            None,
            Utc::now(),
        );
        repo.save(&old_interaction).await.unwrap();

        let cursor = Utc::now();

        let new_interaction = Interaction::new(
            business_id,
            CustomerId::new(),
            InteractionType::new("call").unwrap(),
            None,
            Utc::now(),
        );
        repo.save(&new_interaction).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), new_interaction.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_excludes_other_businesses_for_interaction() {
        let repo = InMemoryInteractionRepository::new();
        let business_a = BusinessId::new();
        let business_b = BusinessId::new();

        let interaction_a = Interaction::new(
            business_a,
            CustomerId::new(),
            InteractionType::new("call").unwrap(),
            None,
            Utc::now(),
        );
        let interaction_b = Interaction::new(
            business_b,
            CustomerId::new(),
            InteractionType::new("call").unwrap(),
            None,
            Utc::now(),
        );
        repo.save(&interaction_a).await.unwrap();
        repo.save(&interaction_b).await.unwrap();

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = repo
            .find_updated_since_by_business(business_a, epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id(), interaction_a.id());
    }

    #[tokio::test]
    async fn find_updated_since_by_business_includes_soft_deleted_interactions() {
        let repo = InMemoryInteractionRepository::new();
        let business_id = BusinessId::new();

        let mut interaction = Interaction::new(
            business_id,
            CustomerId::new(),
            InteractionType::new("call").unwrap(),
            None,
            Utc::now(),
        );
        repo.save(&interaction).await.unwrap();

        let cursor = Utc::now();

        // Client offline harus tahu soal penghapusan juga.
        interaction.soft_delete();
        repo.save(&interaction).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert!(changed[0].is_deleted());
    }
}
