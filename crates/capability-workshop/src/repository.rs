use std::future::Future;

use chrono::{DateTime, Utc};
use domain::BusinessId;

use crate::error::RepositoryError;
use crate::service_order::{ServiceOrder, ServiceOrderId};

/// Port untuk menyimpan/mengambil ServiceOrder. Implementasi konkret
/// (Postgres, in-memory untuk test) ada di luar modul ini — modul ini
/// hanya mendefinisikan kontraknya. Pola sama persis seperti trait
/// Repository di Core (`application::repository`).
pub trait ServiceOrderRepository: Send + Sync {
    fn find_by_id(
        &self,
        id: ServiceOrderId,
    ) -> impl Future<Output = Result<Option<ServiceOrder>, RepositoryError>> + Send;

    fn save(
        &self,
        order: &ServiceOrder,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Semua ServiceOrder di bawah satu Business yang berubah sejak
    /// `since` — dipakai endpoint incremental sync nanti, pola sama
    /// seperti `find_updated_since_by_business` di Core.
    fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<ServiceOrder>, RepositoryError>> + Send;
}
