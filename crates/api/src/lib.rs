pub mod business_routes;
pub mod customer_routes;
pub mod dto;
pub mod error;
pub mod interaction_routes;
pub mod relationship_routes;
pub mod state;
pub mod sync_routes;
pub mod tenant_routes;
pub mod transaction_routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};

use application::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    TenantRepository, TransactionRepository,
};

pub use state::AppState;

/// Menyusun seluruh route Core jadi satu `Router`. Dipisah dari
/// `main.rs` supaya bisa dipakai ulang oleh test integrasi tanpa perlu
/// menjalankan server TCP sungguhan.
///
/// SENGAJA HANYA generik atas Repository Core (`TR`/`BR`/`CR`/`TxR`/`RR`
/// /`IR`) — crate ini TIDAK LAGI mengenal Capability apa pun (tidak ada
/// import `capability_workshop` di sini sama sekali). Route Capability
/// dirakit terpisah oleh crate Capability masing-masing (lihat
/// `capability_workshop::build_workshop_router`) dan digabung ke hasil
/// `build_router` ini lewat `Router::merge` di titik komposisi
/// (`main.rs`/test) — bukan lewat parameter generik gabungan di sini.
/// Konsekuensinya: jumlah generik di fungsi ini TIDAK bertambah setiap
/// kali Capability baru ditambahkan (Laundry, Klinik, dst).
pub fn build_router<TR, BR, CR, TxR, RR, IR>(state: AppState<TR, BR, CR, TxR, RR, IR>) -> Router
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
{
    Router::new()
        .route(
            "/tenants",
            post(tenant_routes::create_tenant::<TR, BR, CR, TxR, RR, IR>)
                .get(sync_routes::list_tenants_updated_since::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/tenants/{id}",
            get(tenant_routes::get_tenant::<TR, BR, CR, TxR, RR, IR>)
                .patch(tenant_routes::rename_tenant::<TR, BR, CR, TxR, RR, IR>)
                .delete(tenant_routes::delete_tenant::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/tenants/{tenant_id}/businesses",
            post(business_routes::create_business::<TR, BR, CR, TxR, RR, IR>)
                .get(sync_routes::list_businesses_updated_since::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/businesses/{id}",
            patch(business_routes::rename_business::<TR, BR, CR, TxR, RR, IR>)
                .delete(business_routes::delete_business::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/businesses/{business_id}/customers",
            post(customer_routes::create_customer::<TR, BR, CR, TxR, RR, IR>)
                .get(sync_routes::list_customers_updated_since::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/customers/{id}",
            patch(customer_routes::rename_customer::<TR, BR, CR, TxR, RR, IR>)
                .delete(customer_routes::delete_customer::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/customers/{id}/phone",
            patch(customer_routes::update_customer_phone::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/businesses/{business_id}/transactions",
            post(transaction_routes::create_transaction::<TR, BR, CR, TxR, RR, IR>)
                .get(sync_routes::list_transactions_updated_since::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/transactions/{id}",
            axum::routing::delete(
                transaction_routes::delete_transaction::<TR, BR, CR, TxR, RR, IR>,
            ),
        )
        .route(
            "/businesses/{business_id}/relationships",
            post(relationship_routes::create_relationship::<TR, BR, CR, TxR, RR, IR>)
                .get(sync_routes::list_relationships_updated_since::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/relationships/{id}",
            axum::routing::delete(
                relationship_routes::delete_relationship::<TR, BR, CR, TxR, RR, IR>,
            ),
        )
        .route(
            "/businesses/{business_id}/interactions",
            post(interaction_routes::create_interaction::<TR, BR, CR, TxR, RR, IR>)
                .get(sync_routes::list_interactions_updated_since::<TR, BR, CR, TxR, RR, IR>),
        )
        .route(
            "/interactions/{id}",
            axum::routing::delete(
                interaction_routes::delete_interaction::<TR, BR, CR, TxR, RR, IR>,
            ),
        )
        .with_state(Arc::new(state))
}
