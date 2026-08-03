use domain::{Business, BusinessId, BusinessName, Tenant, TenantId};

use crate::error::RepositoryError;

/// Port untuk menyimpan/mengambil Tenant. Implementasi konkret (Postgres,
/// in-memory untuk test, dll) ada di luar crate ini — crate ini hanya
/// mendefinisikan kontraknya.
pub trait TenantRepository {
    fn find_by_id(&self, id: TenantId) -> Result<Option<Tenant>, RepositoryError>;
    fn save(&self, tenant: &Tenant) -> Result<(), RepositoryError>;
}

/// Port untuk menyimpan/mengambil Business.
pub trait BusinessRepository {
    fn find_by_id(&self, id: BusinessId) -> Result<Option<Business>, RepositoryError>;

    /// Nama-nama Business AKTIF (belum di-soft-delete) pada satu Tenant.
    /// Dipakai untuk mengecek keunikan nama sebelum create/rename.
    fn find_active_names_by_tenant(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<BusinessName>, RepositoryError>;

    /// Jumlah Business AKTIF pada satu Tenant. Dipakai sebelum menghapus Tenant.
    fn count_active_by_tenant(&self, tenant_id: TenantId) -> Result<usize, RepositoryError>;

    fn save(&self, business: &Business) -> Result<(), RepositoryError>;
}
