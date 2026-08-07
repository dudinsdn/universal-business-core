pub mod business_routes;
pub mod customer_routes;
pub mod dto;
pub mod error;
pub mod state;
pub mod sync_routes;
pub mod tenant_routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};

use application::{BusinessRepository, CustomerRepository, TenantRepository};

pub use state::AppState;

/// Menyusun seluruh route jadi satu `Router`. Dipisah dari `main.rs` supaya
/// bisa dipakai ulang oleh test integrasi tanpa perlu menjalankan server
/// TCP sungguhan.
///
/// Generik atas `TR`/`BR`/`CR`: dipanggil dengan `PgTenantRepository`/
/// `PgBusinessRepository`/dst dari `main.rs` (production), atau dengan
/// repository in-memory dari test — tidak ada percabangan kode di sini
/// sama sekali, cuma tipe konkret yang beda di titik pemanggilan.
pub fn build_router<TR, BR, CR>(state: AppState<TR, BR, CR>) -> Router
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
    CR: CustomerRepository + Clone + 'static,
{
    Router::new()
        .route(
            "/tenants",
            post(tenant_routes::create_tenant::<TR, BR, CR>)
                .get(sync_routes::list_tenants_updated_since::<TR, BR, CR>),
        )
        .route(
            "/tenants/{id}",
            get(tenant_routes::get_tenant::<TR, BR, CR>)
                .patch(tenant_routes::rename_tenant::<TR, BR, CR>)
                .delete(tenant_routes::delete_tenant::<TR, BR, CR>),
        )
        .route(
            "/tenants/{tenant_id}/businesses",
            post(business_routes::create_business::<TR, BR, CR>)
                .get(sync_routes::list_businesses_updated_since::<TR, BR, CR>),
        )
        .route(
            "/businesses/{id}",
            patch(business_routes::rename_business::<TR, BR, CR>)
                .delete(business_routes::delete_business::<TR, BR, CR>),
        )
        .route(
            "/businesses/{business_id}/customers",
            post(customer_routes::create_customer::<TR, BR, CR>)
                .get(sync_routes::list_customers_updated_since::<TR, BR, CR>),
        )
        .route(
            "/customers/{id}",
            patch(customer_routes::rename_customer::<TR, BR, CR>)
                .delete(customer_routes::delete_customer::<TR, BR, CR>),
        )
        .route(
            "/customers/{id}/phone",
            patch(customer_routes::update_customer_phone::<TR, BR, CR>),
        )
        .with_state(Arc::new(state))
}
