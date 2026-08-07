pub mod business_routes;
pub mod customer_routes;
pub mod dto;
pub mod error;
pub mod state;
pub mod sync_routes;
pub mod tenant_routes;
pub mod transaction_routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};

use application::{
    BusinessRepository, CustomerRepository, TenantRepository, TransactionRepository,
};

pub use state::AppState;

/// Menyusun seluruh route jadi satu `Router`. Dipisah dari `main.rs` supaya
/// bisa dipakai ulang oleh test integrasi tanpa perlu menjalankan server
/// TCP sungguhan.
///
/// Generik atas `TR`/`BR`/`CR`/`TxR`: dipanggil dengan `PgTenantRepository`/
/// `PgBusinessRepository`/dst dari `main.rs` (production), atau dengan
/// repository in-memory dari test — tidak ada percabangan kode di sini
/// sama sekali, cuma tipe konkret yang beda di titik pemanggilan.
pub fn build_router<TR, BR, CR, TxR>(state: AppState<TR, BR, CR, TxR>) -> Router
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
    TxR: TransactionRepository + Clone + 'static,
{
    Router::new()
        .route(
            "/tenants",
            post(tenant_routes::create_tenant::<TR, BR, CR, TxR>)
                .get(sync_routes::list_tenants_updated_since::<TR, BR, CR, TxR>),
        )
        .route(
            "/tenants/{id}",
            get(tenant_routes::get_tenant::<TR, BR, CR, TxR>)
                .patch(tenant_routes::rename_tenant::<TR, BR, CR, TxR>)
                .delete(tenant_routes::delete_tenant::<TR, BR, CR, TxR>),
        )
        .route(
            "/tenants/{tenant_id}/businesses",
            post(business_routes::create_business::<TR, BR, CR, TxR>)
                .get(sync_routes::list_businesses_updated_since::<TR, BR, CR, TxR>),
        )
        .route(
            "/businesses/{id}",
            patch(business_routes::rename_business::<TR, BR, CR, TxR>)
                .delete(business_routes::delete_business::<TR, BR, CR, TxR>),
        )
        .route(
            "/businesses/{business_id}/customers",
            post(customer_routes::create_customer::<TR, BR, CR, TxR>)
                .get(sync_routes::list_customers_updated_since::<TR, BR, CR, TxR>),
        )
        .route(
            "/customers/{id}",
            patch(customer_routes::rename_customer::<TR, BR, CR, TxR>)
                .delete(customer_routes::delete_customer::<TR, BR, CR, TxR>),
        )
        .route(
            "/customers/{id}/phone",
            patch(customer_routes::update_customer_phone::<TR, BR, CR, TxR>),
        )
        .route(
            "/businesses/{business_id}/transactions",
            post(transaction_routes::create_transaction::<TR, BR, CR, TxR>)
                .get(sync_routes::list_transactions_updated_since::<TR, BR, CR, TxR>),
        )
        .route(
            "/transactions/{id}",
            axum::routing::delete(transaction_routes::delete_transaction::<TR, BR, CR, TxR>),
        )
        .with_state(Arc::new(state))
}
