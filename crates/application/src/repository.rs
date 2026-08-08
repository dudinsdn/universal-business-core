use std::future::Future;

use chrono::{DateTime, Utc};
use domain::{
    Business, BusinessId, BusinessName, Customer, CustomerId, Interaction, InteractionId,
    Relationship, RelationshipId, Tenant, TenantId, Transaction, TransactionId,
};

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

/// Port untuk menyimpan/mengambil Customer.
///
/// Tidak ada method sejenis `find_active_names_by_tenant` seperti di
/// `BusinessRepository` — nama Customer sengaja TIDAK unik (keputusan
/// domain yang sudah diambil), jadi tidak ada business rule keunikan yang
/// perlu dicek lewat Repository di sini.
pub trait CustomerRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: CustomerId,
    ) -> impl Future<Output = Result<Option<Customer>, RepositoryError>> + Send;

    fn save(&self, customer: &Customer)
    -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Sama seperti `BusinessRepository::find_updated_since_by_tenant`, tapi
    /// dibatasi pada satu Business (Customer bernaung di bawah Business,
    /// bukan langsung di bawah Tenant).
    fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Customer>, RepositoryError>> + Send;
}

/// Port untuk menyimpan/mengambil Transaction.
///
/// Sama seperti `CustomerRepository`: tidak ada method keunikan nama —
/// Transaction tidak punya nama, dan tidak ada business rule keunikan
/// apa pun di level ini.
pub trait TransactionRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: TransactionId,
    ) -> impl Future<Output = Result<Option<Transaction>, RepositoryError>> + Send;

    fn save(
        &self,
        transaction: &Transaction,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Sama seperti `CustomerRepository::find_updated_since_by_business` —
    /// Transaction bernaung di bawah Business, bukan langsung di bawah
    /// Tenant.
    fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Transaction>, RepositoryError>> + Send;
}

/// Sama seperti `TransactionRepository`: tidak ada method keunikan apa
/// pun di level ini. Pencegahan relationship duplikat (pasangan Customer
/// + jenis yang sama) BELUM diimplementasikan.
///
/// Belum ada keputusan eksplisit soal itu (lihat catatan di
/// `RelationshipService`).
pub trait RelationshipRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: RelationshipId,
    ) -> impl Future<Output = Result<Option<Relationship>, RepositoryError>> + Send;

    fn save(
        &self,
        relationship: &Relationship,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Sama seperti `TransactionRepository::find_updated_since_by_business`
    /// — Relationship bernaung di bawah Business, bukan langsung di bawah
    /// Tenant.
    fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Relationship>, RepositoryError>> + Send;
}

/// Port untuk menyimpan/mengambil Interaction.
///
/// Sama seperti `TransactionRepository`/`RelationshipRepository`: tidak
/// ada method keunikan apa pun di level ini.
pub trait InteractionRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: InteractionId,
    ) -> impl Future<Output = Result<Option<Interaction>, RepositoryError>> + Send;

    fn save(
        &self,
        interaction: &Interaction,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Sama seperti `RelationshipRepository::find_updated_since_by_business`
    /// — Interaction bernaung di bawah Business, bukan langsung di bawah
    /// Tenant.
    fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Interaction>, RepositoryError>> + Send;
}
