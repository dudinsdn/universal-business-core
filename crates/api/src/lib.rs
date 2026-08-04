pub mod business_routes;
pub mod dto;
pub mod error;
pub mod state;
pub mod tenant_routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};

use application::{BusinessRepository, TenantRepository};

pub use state::AppState;

/// Menyusun seluruh route jadi satu `Router`. Dipisah dari `main.rs` supaya
/// bisa dipakai ulang oleh test integrasi tanpa perlu menjalankan server
/// TCP sungguhan.
///
/// Generik atas `TR`/`BR`: dipanggil dengan `PgTenantRepository`/
/// `PgBusinessRepository` dari `main.rs` (production), atau dengan
/// repository in-memory dari test — tidak ada percabangan kode di sini
/// sama sekali, cuma tipe konkret yang beda di titik pemanggilan.
pub fn build_router<TR, BR>(state: AppState<TR, BR>) -> Router
where
    TR: TenantRepository + Clone + 'static,
    BR: BusinessRepository + Clone + 'static,
{
    Router::new()
        .route("/tenants", post(tenant_routes::create_tenant::<TR, BR>))
        .route(
            "/tenants/{id}",
            get(tenant_routes::get_tenant::<TR, BR>)
                .patch(tenant_routes::rename_tenant::<TR, BR>)
                .delete(tenant_routes::delete_tenant::<TR, BR>),
        )
        .route(
            "/tenants/{tenant_id}/businesses",
            post(business_routes::create_business::<TR, BR>),
        )
        .route(
            "/businesses/{id}",
            patch(business_routes::rename_business::<TR, BR>)
                .delete(business_routes::delete_business::<TR, BR>),
        )
        .with_state(Arc::new(state))
}
