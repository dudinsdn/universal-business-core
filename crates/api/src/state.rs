use std::sync::Arc;

use application::{
    BusinessRepository, BusinessService, CustomerRepository, CustomerService,
    InMemoryBusinessRepository, InMemoryCustomerRepository, InMemoryInteractionRepository,
    InMemoryRelationshipRepository, InMemoryTenantRepository, InMemoryTransactionRepository,
    InteractionRepository, InteractionService, RelationshipRepository, RelationshipService,
    TenantRepository, TenantService, TransactionRepository, TransactionService,
};

/// State yang dibagi ke semua handler Core.
///
/// Generik atas tipe Repository (`TR`, `BR`, `CR`, `TxR`, `RR`, `IR`) —
/// production (`main.rs`) memasangnya dengan
/// `PgTenantRepository`/`PgBusinessRepository`/dst, test memasangnya
/// dengan repository in-memory — tanpa satu baris pun kode handler yang
/// beda antara keduanya.
///
/// SENGAJA TIDAK generik atas Capability apa pun (mis. tidak ada lagi
/// `SR`/`ServiceOrderRepository` di sini). Setiap Capability (Workshop,
/// nanti Laundry/Klinik) punya state & router HTTP-nya sendiri (lihat
/// `capability_workshop::WorkshopState`/`build_workshop_router`),
/// digabung ke Router Core lewat `.merge()` di titik komposisi
/// (`main.rs`/test) — bukan lewat parameter generik gabungan di sini.
/// Ini yang membuat jumlah generik di `AppState` TIDAK bertambah setiap
/// kali Capability baru ditambahkan.
pub struct AppState<
    TR: TenantRepository,
    BR: BusinessRepository,
    CR: CustomerRepository,
    TxR: TransactionRepository,
    RR: RelationshipRepository,
    IR: InteractionRepository,
> {
    pub tenant_service: TenantService<TR>,
    pub business_service: BusinessService<BR>,
    pub customer_service: CustomerService<CR>,
    pub transaction_service: TransactionService<TxR>,
    pub relationship_service: RelationshipService<RR>,
    pub interaction_service: InteractionService<IR>,
    /// Dipakai langsung (bukan lewat service) khusus untuk
    /// `delete_tenant`, yang butuh `BusinessRepository` sebagai parameter
    /// eksplisit karena itu operasi lintas-aggregate.
    pub business_repository: BR,
}

/// Alias untuk `Arc<AppState<...>>` — dipakai di parameter `State<...>`
/// setiap handler supaya signature-nya tidak "very complex type" menurut
/// clippy. Murni penyederhanaan tulisan, bukan perubahan tipe.
pub type SharedState<TR, BR, CR, TxR, RR, IR> = Arc<AppState<TR, BR, CR, TxR, RR, IR>>;

impl<
    TR: TenantRepository,
    BR: BusinessRepository + Clone,
    CR: CustomerRepository,
    TxR: TransactionRepository,
    RR: RelationshipRepository,
    IR: InteractionRepository,
> AppState<TR, BR, CR, TxR, RR, IR>
{
    pub fn new(
        tenant_repository: TR,
        business_repository: BR,
        customer_repository: CR,
        transaction_repository: TxR,
        relationship_repository: RR,
        interaction_repository: IR,
    ) -> Self {
        Self {
            tenant_service: TenantService::new(tenant_repository),
            business_service: BusinessService::new(business_repository.clone()),
            customer_service: CustomerService::new(customer_repository),
            transaction_service: TransactionService::new(transaction_repository),
            relationship_service: RelationshipService::new(relationship_repository),
            interaction_service: InteractionService::new(interaction_repository),
            business_repository,
        }
    }
}

/// Konstruktor khusus untuk test — repository in-memory, tidak butuh
/// Postgres sama sekali.
impl
    AppState<
        InMemoryTenantRepository,
        InMemoryBusinessRepository,
        InMemoryCustomerRepository,
        InMemoryTransactionRepository,
        InMemoryRelationshipRepository,
        InMemoryInteractionRepository,
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
        )
    }
}
