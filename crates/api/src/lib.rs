pub mod business_routes;
pub mod dto;
pub mod error;
pub mod state;
pub mod tenant_routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};

pub use state::AppState;

/// Menyusun seluruh route jadi satu `Router`. Dipisah dari `main.rs` supaya
/// bisa dipakai ulang oleh test integrasi tanpa perlu menjalankan server
/// TCP sungguhan.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/tenants", post(tenant_routes::create_tenant))
        .route(
            "/tenants/{id}",
            get(tenant_routes::get_tenant)
                .patch(tenant_routes::rename_tenant)
                .delete(tenant_routes::delete_tenant),
        )
        .route(
            "/tenants/{tenant_id}/businesses",
            post(business_routes::create_business),
        )
        .route(
            "/businesses/{id}",
            patch(business_routes::rename_business).delete(business_routes::delete_business),
        )
        .with_state(Arc::new(state))
}
