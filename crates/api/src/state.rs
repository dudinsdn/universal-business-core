use application::{
    BusinessRepository, BusinessService, InMemoryBusinessRepository, InMemoryTenantRepository,
    TenantRepository, TenantService,
};

/// State yang dibagi ke semua handler.
///
/// Generik atas tipe Repository (`TR`, `BR`) — bukan cuma "boleh generik
/// secara teori", ini alasan trait Repository dibuat sejak awal: production
/// (`main.rs`) memasangnya dengan `PgTenantRepository`/`PgBusinessRepository`,
/// test (`tests/tenant_flow.rs`) memasangnya dengan repository in-memory —
/// tanpa satu baris pun kode handler yang beda antara keduanya.
pub struct AppState<TR: TenantRepository, BR: BusinessRepository> {
    pub tenant_service: TenantService<TR>,
    pub business_service: BusinessService<BR>,
    /// Dipakai langsung (bukan lewat service) khusus untuk
    /// `delete_tenant`, yang butuh `BusinessRepository` sebagai parameter
    /// eksplisit karena itu operasi lintas-aggregate.
    pub business_repository: BR,
}

impl<TR: TenantRepository, BR: BusinessRepository + Clone> AppState<TR, BR> {
    pub fn new(tenant_repository: TR, business_repository: BR) -> Self {
        Self {
            tenant_service: TenantService::new(tenant_repository),
            business_service: BusinessService::new(business_repository.clone()),
            business_repository,
        }
    }
}

/// Konstruktor khusus untuk test — repository in-memory, tidak butuh
/// Postgres sama sekali. Cuma tersedia untuk kombinasi tipe in-memory,
/// bukan untuk TR/BR generik apa pun (lihat `AppState::new` untuk itu).
impl AppState<InMemoryTenantRepository, InMemoryBusinessRepository> {
    pub fn new_in_memory() -> Self {
        Self::new(
            InMemoryTenantRepository::new(),
            InMemoryBusinessRepository::new(),
        )
    }
}
