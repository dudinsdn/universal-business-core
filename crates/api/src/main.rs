use api::{AppState, build_router};
use application::InMemoryRelationshipRepository;
use infra_postgres::{
    PgBusinessRepository, PgCustomerRepository, PgTenantRepository, PgTransactionRepository,
    run_migrations,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL harus di-set, contoh: postgres://user:pass@localhost/dbname");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("gagal konek ke Postgres — cek DATABASE_URL & apakah Postgres jalan");

    run_migrations(&pool)
        .await
        .expect("gagal menjalankan migration");

    let tenant_repository = PgTenantRepository::new(pool.clone());
    let business_repository = PgBusinessRepository::new(pool.clone());
    let customer_repository = PgCustomerRepository::new(pool.clone());
    let transaction_repository = PgTransactionRepository::new(pool);
    // SEMENTARA: belum ada PgRelationshipRepository — sesuai Development
    // Rules, urutan wajibnya "Implementasikan API" (5) mendahului
    // "Tambahkan database" (6). Data Relationship TIDAK akan persisten
    // sampai PgRelationshipRepository dibuat di langkah database
    // selanjutnya.
    let relationship_repository = InMemoryRelationshipRepository::new();

    let state = AppState::new(
        tenant_repository,
        business_repository,
        customer_repository,
        transaction_repository,
        relationship_repository,
    );
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("gagal bind ke port 3000");

    println!(
        "API berjalan di http://0.0.0.0:3000 (repository: Postgres, kecuali Relationship: in-memory sementara)"
    );

    axum::serve(listener, app)
        .await
        .expect("server berhenti tidak terduga");
}
