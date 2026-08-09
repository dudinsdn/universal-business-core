//! Implementasi in-memory dari `ServiceOrderRepository`.
//!
//! Dipakai untuk dua kebutuhan nyata (pola sama seperti
//! `application::in_memory` di Core):
//! - Unit test `ServiceOrderService` (cepat, tanpa infrastruktur).
//! - Bootstrap awal API sebelum implementasi Postgres tersedia.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use domain::BusinessId;

use crate::error::RepositoryError;
use crate::repository::ServiceOrderRepository;
use crate::service_order::{ServiceOrder, ServiceOrderId};

#[derive(Debug, Clone, Default)]
pub struct InMemoryServiceOrderRepository {
    data: Arc<Mutex<HashMap<ServiceOrderId, ServiceOrder>>>,
}

impl InMemoryServiceOrderRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ServiceOrderRepository for InMemoryServiceOrderRepository {
    async fn find_by_id(
        &self,
        id: ServiceOrderId,
    ) -> Result<Option<ServiceOrder>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        Ok(data.get(&id).cloned())
    }

    async fn save(&self, order: &ServiceOrder) -> Result<(), RepositoryError> {
        let mut data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        if let Some(existing) = data.get(&order.id()) {
            let expected_previous_version = order.version().saturating_sub(1);
            if existing.version() != expected_previous_version {
                return Err(RepositoryError::VersionConflict);
            }
        }
        data.insert(order.id(), order.clone());
        Ok(())
    }

    async fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<ServiceOrder>, RepositoryError> {
        let data = self
            .data
            .lock()
            .expect("in-memory repository lock poisoned");
        let mut result: Vec<ServiceOrder> = data
            .values()
            .filter(|o| o.business_id() == business_id && o.updated_at() > since)
            .cloned()
            .collect();
        result.sort_by_key(|o| o.updated_at());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_order::ServiceOrderDescription;
    use domain::CustomerId;

    fn sample_order(business_id: BusinessId) -> ServiceOrder {
        ServiceOrder::new(
            business_id,
            CustomerId::new(),
            ServiceOrderDescription::new("Ganti oli").unwrap(),
        )
    }

    #[tokio::test]
    async fn save_detects_concurrent_version_conflict() {
        let repo = InMemoryServiceOrderRepository::new();
        let mut order = sample_order(BusinessId::new());
        repo.save(&order).await.unwrap();

        // Dua "pembaca" sama-sama mulai dari data versi 0.
        let mut stale_copy = order.clone();

        // Salah satu menang duluan: start(), jadi versi 1, berhasil disimpan.
        order.start().unwrap();
        repo.save(&order).await.unwrap();

        // Yang satu lagi telat: masih berdasarkan data versi 0 — harus
        // ditolak sebagai conflict.
        stale_copy.start().unwrap();
        let result = repo.save(&stale_copy).await;

        assert_eq!(result, Err(RepositoryError::VersionConflict));
    }

    #[tokio::test]
    async fn find_updated_since_by_business_excludes_other_businesses() {
        let repo = InMemoryServiceOrderRepository::new();
        let business_a = BusinessId::new();
        let business_b = BusinessId::new();

        repo.save(&sample_order(business_a)).await.unwrap();
        repo.save(&sample_order(business_b)).await.unwrap();

        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let changed = repo
            .find_updated_since_by_business(business_a, epoch)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].business_id(), business_a);
    }

    #[tokio::test]
    async fn find_updated_since_by_business_includes_soft_deleted() {
        let repo = InMemoryServiceOrderRepository::new();
        let business_id = BusinessId::new();

        let mut order = sample_order(business_id);
        repo.save(&order).await.unwrap();

        let cursor = Utc::now();

        // Client offline harus tahu soal penghapusan juga — pola sama
        // seperti Core.
        order.soft_delete();
        repo.save(&order).await.unwrap();

        let changed = repo
            .find_updated_since_by_business(business_id, cursor)
            .await
            .unwrap();

        assert_eq!(changed.len(), 1);
        assert!(changed[0].is_deleted());
    }
}
