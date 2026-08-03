use application::{
    BusinessService, InMemoryBusinessRepository, InMemoryTenantRepository, TenantService,
};

/// State yang dibagi ke semua handler.
///
/// Repository yang dipakai masih in-memory (lihat `application::in_memory`).
/// Implementasi Postgres menyusul di tahap terpisah — tidak mengubah satu
/// pun kode di sini kecuali cara `AppState` dibuat, karena handler hanya
/// bergantung pada trait `TenantRepository`/`BusinessRepository`, bukan
/// implementasi konkretnya.
#[derive(Clone)]
pub struct AppState {
    pub tenant_service: TenantService<InMemoryTenantRepository>,
    pub business_service: BusinessService<InMemoryBusinessRepository>,
    /// Dipakai langsung (bukan lewat service) khusus untuk
    /// `delete_tenant`, yang butuh `BusinessRepository` sebagai parameter
    /// eksplisit karena itu operasi lintas-aggregate.
    pub business_repository: InMemoryBusinessRepository,
}

impl AppState {
    pub fn new_in_memory() -> Self {
        let tenant_repository = InMemoryTenantRepository::new();
        let business_repository = InMemoryBusinessRepository::new();
        Self {
            tenant_service: TenantService::new(tenant_repository),
            business_service: BusinessService::new(business_repository.clone()),
            business_repository,
        }
    }
}
