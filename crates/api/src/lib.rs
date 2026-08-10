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
pub mod workshop_error;
pub mod workshop_routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};

use application::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    TenantRepository, TransactionRepository,
};
use capability_workshop::ServiceOrderRepository;

pub use state::AppState;

/// Menyusun seluruh route jadi satu `Router`. Dipisah dari `main.rs` supaya
/// bisa dipakai ulang oleh test integrasi tanpa perlu menjalankan server
/// TCP sungguhan.
///
/// Generik atas `TR`/`BR`/`CR`/`TxR`/`RR`/`IR`/`SR`: dipanggil dengan
/// `PgTenantRepository`/`PgBusinessRepository`/dst dari `main.rs`
/// (production), atau dengan repository in-memory dari test — tidak ada
/// percabangan kode di sini sama sekali, cuma tipe konkret yang beda di
/// titik pemanggilan. `SR` (`ServiceOrderRepository`, Capability Workshop)
/// ikut generik dengan pola yang sama walau bukan bagian dari Core.
pub fn build_router<TR, BR, CR, TxR, RR, IR, SR>(
    state: AppState<TR, BR, CR, TxR, RR, IR, SR>,
) -> Router
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
    RR: RelationshipRepository + Clone + 'static,
    IR: InteractionRepository + Clone + 'static,
    SR: ServiceOrderRepository + Clone + 'static,
{
    Router::new()
        .route(
            "/tenants",
            post(tenant_routes::create_tenant::<TR, BR, CR, TxR, RR, IR, SR>)
                .get(sync_routes::list_tenants_updated_since::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/tenants/{id}",
            get(tenant_routes::get_tenant::<TR, BR, CR, TxR, RR, IR, SR>)
                .patch(tenant_routes::rename_tenant::<TR, BR, CR, TxR, RR, IR, SR>)
                .delete(tenant_routes::delete_tenant::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/tenants/{tenant_id}/businesses",
            post(business_routes::create_business::<TR, BR, CR, TxR, RR, IR, SR>)
                .get(sync_routes::list_businesses_updated_since::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/businesses/{id}",
            patch(business_routes::rename_business::<TR, BR, CR, TxR, RR, IR, SR>)
                .delete(business_routes::delete_business::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/businesses/{business_id}/customers",
            post(customer_routes::create_customer::<TR, BR, CR, TxR, RR, IR, SR>)
                .get(sync_routes::list_customers_updated_since::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/customers/{id}",
            patch(customer_routes::rename_customer::<TR, BR, CR, TxR, RR, IR, SR>)
                .delete(customer_routes::delete_customer::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/customers/{id}/phone",
            patch(customer_routes::update_customer_phone::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/businesses/{business_id}/transactions",
            post(transaction_routes::create_transaction::<TR, BR, CR, TxR, RR, IR, SR>)
                .get(sync_routes::list_transactions_updated_since::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/transactions/{id}",
            axum::routing::delete(
                transaction_routes::delete_transaction::<TR, BR, CR, TxR, RR, IR, SR>,
            ),
        )
        .route(
            "/businesses/{business_id}/relationships",
            post(relationship_routes::create_relationship::<TR, BR, CR, TxR, RR, IR, SR>)
                .get(sync_routes::list_relationships_updated_since::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/relationships/{id}",
            axum::routing::delete(
                relationship_routes::delete_relationship::<TR, BR, CR, TxR, RR, IR, SR>,
            ),
        )
        .route(
            "/businesses/{business_id}/interactions",
            post(interaction_routes::create_interaction::<TR, BR, CR, TxR, RR, IR, SR>)
                .get(sync_routes::list_interactions_updated_since::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/interactions/{id}",
            axum::routing::delete(
                interaction_routes::delete_interaction::<TR, BR, CR, TxR, RR, IR, SR>,
            ),
        )
        .route(
            "/businesses/{business_id}/service-orders",
            post(workshop_routes::create_service_order::<TR, BR, CR, TxR, RR, IR, SR>).get(
                workshop_routes::list_service_orders_updated_since::<TR, BR, CR, TxR, RR, IR, SR>,
            ),
        )
        .route(
            "/service-orders/{id}/start",
            patch(workshop_routes::start_service_order::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/service-orders/{id}/complete",
            patch(workshop_routes::complete_service_order::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/service-orders/{id}/cancel",
            patch(workshop_routes::cancel_service_order::<TR, BR, CR, TxR, RR, IR, SR>),
        )
        .route(
            "/service-orders/{id}",
            axum::routing::delete(
                workshop_routes::delete_service_order::<TR, BR, CR, TxR, RR, IR, SR>,
            ),
        )
        .with_state(Arc::new(state))
}
