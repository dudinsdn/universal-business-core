pub mod business_repository;
pub mod tenant_repository;

pub use business_repository::PgBusinessRepository;
pub use tenant_repository::PgTenantRepository;

/// Menjalankan migration di folder `migrations/` (di-embed saat compile
/// lewat `sqlx::migrate!`, bukan dibaca dari disk saat runtime — supaya
/// binary yang sudah dibuild tidak bergantung pada folder migrations ada
/// di sebelahnya).
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
