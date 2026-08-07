use application::{
    BusinessRepository, BusinessService, CustomerRepository, CustomerService,
    InMemoryBusinessRepository, InMemoryCustomerRepository, InMemoryTenantRepository,
    TenantRepository, TenantService,
};

/// State yang dibagi ke semua handler.
///
/// Generik atas tipe Repository (`TR`, `BR`, `CR`) — bukan cuma "boleh
/// generik secara teori", ini alasan trait Repository dibuat sejak awal:
/// production (`main.rs`) memasangnya dengan `PgTenantRepository`/
/// `PgBusinessRepository`/dst, test (`tests/tenant_flow.rs`) memasangnya
/// dengan repository in-memory — tanpa satu baris pun kode handler yang
/// beda antara keduanya.
pub struct AppState<TR: TenantRepository, BR: BusinessRepository, CR: CustomerRepository> {
    pub tenant_service: TenantService<TR>,
    pub business_service: BusinessService<BR>,
    pub customer_service: CustomerService<CR>,
    /// Dipakai langsung (bukan lewat service) khusus untuk
    /// `delete_tenant`, yang butuh `BusinessRepository` sebagai parameter
    /// eksplisit karena itu operasi lintas-aggregate.
    pub business_repository: BR,
}

impl<TR: TenantRepository, BR: BusinessRepository + Clone, CR: CustomerRepository>
    AppState<TR, BR, CR>
{
    pub fn new(tenant_repository: TR, business_repository: BR, customer_repository: CR) -> Self {
        Self {
            tenant_service: TenantService::new(tenant_repository),
            business_service: BusinessService::new(business_repository.clone()),
            customer_service: CustomerService::new(customer_repository),
            business_repository,
        }
    }
}

/// Konstruktor khusus untuk test — repository in-memory, tidak butuh
/// Postgres sama sekali. Cuma tersedia untuk kombinasi tipe in-memory,
/// bukan untuk TR/BR/CR generik apa pun (lihat `AppState::new` untuk itu).
impl AppState<InMemoryTenantRepository, InMemoryBusinessRepository, InMemoryCustomerRepository> {
    pub fn new_in_memory() -> Self {
        Self::new(
            InMemoryTenantRepository::new(),
            InMemoryBusinessRepository::new(),
            InMemoryCustomerRepository::new(),
        )
    }
}
