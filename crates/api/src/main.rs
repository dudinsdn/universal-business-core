use api::{AppState, build_router};
use capability_workshop::{WorkshopState, build_workshop_router};
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

    // Core: AppState HANYA generik atas Repository Core — tidak lagi
    // mengenal ServiceOrder/Workshop sama sekali.
    let core_state = AppState::new(
        tenant_repository,
        business_repository,
        customer_repository,
        transaction_repository,
        relationship_repository,
        interaction_repository,
    );

    // Ambil salinan `business_service` SEBELUM `core_state` dipindah
    // (consumed) ke `build_router` — Workshop butuh `BusinessService` yang
    // sama (instance Repository yang sama) untuk `get_business`, bukan
    // koneksi terpisah. `BusinessService` derive `Clone` murah (cuma
    // menyalin Arc/handle di dalam Repository-nya, bukan data).
    let business_service_for_workshop = core_state.business_service.clone();

    let core_router = build_router(core_state);

    // Workshop: router HTTP mandiri, generik HANYA atas dependency yang
    // dia butuhkan (BusinessRepository lewat BusinessService + repository
    // ServiceOrder miliknya sendiri) — bukan lagi lewat parameter generik
    // gabungan di `AppState`.
    let workshop_state =
        WorkshopState::new(business_service_for_workshop, service_order_repository);
    let workshop_router = build_workshop_router(workshop_state);

    let app = core_router.merge(workshop_router);

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
