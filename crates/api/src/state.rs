use std::sync::Arc;

use application::{
    BusinessRepository, BusinessService, CustomerRepository, CustomerService,
    InMemoryBusinessRepository, InMemoryCustomerRepository, InMemoryInteractionRepository,
    InMemoryRelationshipRepository, InMemoryTenantRepository, InMemoryTransactionRepository,
    InteractionRepository, InteractionService, RelationshipRepository, RelationshipService,
    TenantRepository, TenantService, TransactionRepository, TransactionService,
};
use capability_workshop::{
    InMemoryServiceOrderRepository, ServiceOrderRepository, ServiceOrderService,
};

/// State yang dibagi ke semua handler.
///
/// Generik atas tipe Repository (`TR`, `BR`, `CR`, `TxR`, `RR`, `IR`, `SR`)
/// — bukan cuma "boleh generik secara teori", ini alasan trait Repository
/// dibuat sejak awal: production (`main.rs`) memasangnya dengan
/// `PgTenantRepository`/`PgBusinessRepository`/dst, test
/// (`tests/tenant_flow.rs`) memasangnya dengan repository in-memory —
/// tanpa satu baris pun kode handler yang beda antara keduanya.
///
/// `SR` (`ServiceOrderRepository`) adalah repository Capability Workshop
/// — dari luar Core, tapi tetap dipasang lewat pola generik yang sama
/// supaya `api` tidak perlu tahu implementasi konkretnya (Postgres belum
/// ada untuk ServiceOrder di tahap ini, lihat `main.rs`).
pub struct AppState<
    TR: TenantRepository,
    BR: BusinessRepository,
    CR: CustomerRepository,
    TxR: TransactionRepository,
    RR: RelationshipRepository,
    IR: InteractionRepository,
    SR: ServiceOrderRepository,
> {
    pub tenant_service: TenantService<TR>,
    pub business_service: BusinessService<BR>,
    pub customer_service: CustomerService<CR>,
    pub transaction_service: TransactionService<TxR>,
    pub relationship_service: RelationshipService<RR>,
    pub interaction_service: InteractionService<IR>,
    pub service_order_service: ServiceOrderService<SR>,
    /// Dipakai langsung (bukan lewat service) khusus untuk
    /// `delete_tenant`, yang butuh `BusinessRepository` sebagai parameter
    /// eksplisit karena itu operasi lintas-aggregate.
    pub business_repository: BR,
}

/// Alias untuk `Arc<AppState<...>>` — dipakai di parameter `State<...>`
/// setiap handler supaya signature-nya tidak "very complex type" menurut
/// clippy. Murni penyederhanaan tulisan, bukan perubahan tipe.
pub type SharedState<TR, BR, CR, TxR, RR, IR, SR> = Arc<AppState<TR, BR, CR, TxR, RR, IR, SR>>;

impl<
    TR: TenantRepository,
    BR: BusinessRepository + Clone,
    CR: CustomerRepository,
    TxR: TransactionRepository,
    RR: RelationshipRepository,
    IR: InteractionRepository,
    SR: ServiceOrderRepository,
> AppState<TR, BR, CR, TxR, RR, IR, SR>
{
    pub fn new(
        tenant_repository: TR,
        business_repository: BR,
        customer_repository: CR,
        transaction_repository: TxR,
        relationship_repository: RR,
        interaction_repository: IR,
        service_order_repository: SR,
    ) -> Self {
        Self {
            tenant_service: TenantService::new(tenant_repository),
            business_service: BusinessService::new(business_repository.clone()),
            customer_service: CustomerService::new(customer_repository),
            transaction_service: TransactionService::new(transaction_repository),
            relationship_service: RelationshipService::new(relationship_repository),
            interaction_service: InteractionService::new(interaction_repository),
            service_order_service: ServiceOrderService::new(service_order_repository),
            business_repository,
        }
    }
}

/// Konstruktor khusus untuk test — repository in-memory, tidak butuh
/// Postgres sama sekali. Cuma tersedia untuk kombinasi tipe in-memory,
/// bukan untuk TR/BR/CR/TxR/RR/IR/SR generik apa pun (lihat `AppState::new`
/// untuk itu).
impl
    AppState<
        InMemoryTenantRepository,
        InMemoryBusinessRepository,
        InMemoryCustomerRepository,
        InMemoryTransactionRepository,
        InMemoryRelationshipRepository,
        InMemoryInteractionRepository,
        InMemoryServiceOrderRepository,
    >
{
    pub fn new_in_memory() -> Self {
        Self::new(
            InMemoryTenantRepository::new(),
            InMemoryBusinessRepository::new(),
            InMemoryCustomerRepository::new(),
            InMemoryTransactionRepository::new(),
            InMemoryRelationshipRepository::new(),
            InMemoryInteractionRepository::new(),
            InMemoryServiceOrderRepository::new(),
        )
    }
}
