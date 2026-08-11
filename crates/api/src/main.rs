use api::{AppState, build_router};
use infra_postgres::{
    PgBusinessRepository, PgCustomerRepository, PgInteractionRepository, PgRelationshipRepository,
    PgServiceOrderRepository, PgTenantRepository, PgTransactionRepository, run_migrations,
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
    let transaction_repository = PgTransactionRepository::new(pool.clone());
    let relationship_repository = PgRelationshipRepository::new(pool.clone());
    let interaction_repository = PgInteractionRepository::new(pool.clone());
    let service_order_repository = PgServiceOrderRepository::new(pool);

    let state = AppState::new(
        tenant_repository,
        business_repository,
        customer_repository,
        transaction_repository,
        relationship_repository,
        interaction_repository,
        service_order_repository,
    );
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("gagal bind ke port 3000");

    println!("🚀 Server Axum berjalan di:");
    println!("   -> Local:   http://localhost:3000");

    // Otomatis deteksi semua IP lokal aktif (Wi-Fi, USB Tethering, Ethernet)
    if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
        for iface in interfaces {
            // Filter hanya IPv4 dan abaikan loopback (127.0.0.1)
            if !iface.is_loopback() {
                if let std::net::IpAddr::V4(ip) = iface.ip() {
                    println!("   -> Network ({}) : http://{}:3000", iface.name, ip);
                }
            }
        }
    }

    axum::serve(listener, app)
        .await
        .expect("server berhenti tidak terduga");
}
