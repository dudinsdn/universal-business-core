use std::future::Future;

use chrono::{DateTime, Utc};
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

    /// Semua Tenant yang berubah (dibuat/diubah/dihapus) sejak `since`,
    /// diurutkan menaik berdasarkan `updated_at`. TERMASUK Tenant yang
    /// sudah di-soft-delete — client offline butuh tahu itu juga, supaya
    /// bisa menghapus salinan lokalnya, bukan cuma menerima yang aktif
    /// saja. Ini fondasi endpoint incremental sync (`GET /tenants
    /// ?updated_since=...`) untuk kebutuhan Offline First.
    fn find_updated_since(
        &self,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Tenant>, RepositoryError>> + Send;
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

    /// Sama seperti `TenantRepository::find_updated_since`, tapi dibatasi
    /// pada satu Tenant (konsisten dengan resource path
    /// `/tenants/{tenant_id}/businesses`).
    fn find_updated_since_by_tenant(
        &self,
        tenant_id: TenantId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Business>, RepositoryError>> + Send;
}
