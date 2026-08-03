//! Repository palsu (in-memory) khusus untuk unit test Application Service.
//! Diletakkan di satu tempat supaya tidak duplikat antara test
//! `tenant_service` dan `business_service`.

use std::cell::RefCell;
use std::collections::HashMap;

use domain::{Business, BusinessId, BusinessName, Tenant, TenantId};

use crate::error::RepositoryError;
use crate::repository::{BusinessRepository, TenantRepository};

#[derive(Default)]
pub struct InMemoryTenantRepository {
    data: RefCell<HashMap<TenantId, Tenant>>,
}

impl InMemoryTenantRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TenantRepository for InMemoryTenantRepository {
    fn find_by_id(&self, id: TenantId) -> Result<Option<Tenant>, RepositoryError> {
        Ok(self.data.borrow().get(&id).cloned())
    }

    fn save(&self, tenant: &Tenant) -> Result<(), RepositoryError> {
        self.data.borrow_mut().insert(tenant.id(), tenant.clone());
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryBusinessRepository {
    data: RefCell<HashMap<BusinessId, Business>>,
}

impl InMemoryBusinessRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BusinessRepository for InMemoryBusinessRepository {
    fn find_by_id(&self, id: BusinessId) -> Result<Option<Business>, RepositoryError> {
        Ok(self.data.borrow().get(&id).cloned())
    }

    fn find_active_names_by_tenant(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<BusinessName>, RepositoryError> {
        Ok(self
            .data
            .borrow()
            .values()
            .filter(|b| b.tenant_id() == tenant_id && !b.is_deleted())
            .map(|b| b.name().clone())
            .collect())
    }

    fn count_active_by_tenant(&self, tenant_id: TenantId) -> Result<usize, RepositoryError> {
        Ok(self
            .data
            .borrow()
            .values()
            .filter(|b| b.tenant_id() == tenant_id && !b.is_deleted())
            .count())
    }

    fn save(&self, business: &Business) -> Result<(), RepositoryError> {
        self.data
            .borrow_mut()
            .insert(business.id(), business.clone());
        Ok(())
    }
}
