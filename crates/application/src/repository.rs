use std::future::Future;

use domain::{Business, BusinessId, BusinessName, Tenant, TenantId};

use crate::error::RepositoryError;

/// Port untuk menyimpan/mengambil Tenant. Implementasi konkret (Postgres,
/// in-memory untuk test, dll) ada di luar crate ini — crate ini hanya
/// mendefinisikan kontraknya.
///
/// Method ditulis sebagai `fn ... -> impl Future<..> + Send` (bukan
/// `async fn` langsung) supaya bound `Send` bisa dinyatakan eksplisit di
/// signature trait. Ini dibutuhkan karena Tenant/BusinessService dipanggil
/// dari handler HTTP (axum, multi-thread) — tanpa `Send` di sini,
/// kompilator tidak bisa membuktikan future-nya aman dipindah antar-thread.
/// Pendekatan ini menghindari penambahan dependency (`async-trait`) untuk
/// kasus yang sebenarnya bisa diselesaikan lewat fitur bahasa native
/// (stabil sejak Rust 1.75).
pub trait TenantRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: TenantId,
    ) -> impl Future<Output = Result<Option<Tenant>, RepositoryError>> + Send;

    fn save(&self, tenant: &Tenant) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}

/// Port untuk menyimpan/mengambil Business.
pub trait BusinessRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: BusinessId,
    ) -> impl Future<Output = Result<Option<Business>, RepositoryError>> + Send;

    /// Nama-nama Business AKTIF (belum di-soft-delete) pada satu Tenant.
    /// Dipakai untuk mengecek keunikan nama sebelum create/rename.
    fn find_active_names_by_tenant(
        &self,
        tenant_id: TenantId,
    ) -> impl Future<Output = Result<Vec<BusinessName>, RepositoryError>> + Send;

    /// Jumlah Business AKTIF pada satu Tenant. Dipakai sebelum menghapus Tenant.
    fn count_active_by_tenant(
        &self,
        tenant_id: TenantId,
    ) -> impl Future<Output = Result<usize, RepositoryError>> + Send;

    fn save(&self, business: &Business)
    -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
