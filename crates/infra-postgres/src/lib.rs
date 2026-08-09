pub mod business_repository;
pub mod customer_repository;
pub mod interaction_repository;
pub mod relationship_repository;
pub mod tenant_repository;
pub mod transaction_repository;

pub use business_repository::PgBusinessRepository;
pub use customer_repository::PgCustomerRepository;
pub use interaction_repository::PgInteractionRepository;
pub use relationship_repository::PgRelationshipRepository;
pub use tenant_repository::PgTenantRepository;
pub use transaction_repository::PgTransactionRepository;

/// Menjalankan migration di folder `migrations/` (di-embed saat compile
/// lewat `sqlx::migrate!`, bukan dibaca dari disk saat runtime — supaya
/// binary yang sudah dibuild tidak bergantung pada folder migrations ada
/// di sebelahnya).
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
