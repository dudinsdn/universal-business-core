//! State HTTP milik Capability Workshop.
//!
//! Beda dari versi lama (`api::AppState` yang generik atas SEMUA
//! repository Core + `SR`), state ini HANYA generik atas dependency yang
//! benar-benar dipakai Workshop:
//! - `BR` (`BusinessRepository`) lewat `BusinessService<BR>` dari Core —
//!   dibutuhkan untuk `get_business` sebelum membuat ServiceOrder.
//! - `SR` (`ServiceOrderRepository`) milik Workshop sendiri.
//!
//! Ini yang membuat `api::AppState` (Core) tidak perlu tahu Workshop
//! sama sekali, dan sebaliknya Workshop tidak perlu tahu
//! Tenant/Customer/Transaction/dst Repository — cuma yang dipakainya.

use std::sync::Arc;

use application::{BusinessRepository, BusinessService};

use crate::repository::ServiceOrderRepository;
use crate::service_order_service::ServiceOrderService;

pub struct WorkshopState<BR: BusinessRepository, SR: ServiceOrderRepository> {
    pub business_service: BusinessService<BR>,
    pub service_order_service: ServiceOrderService<SR>,
}

/// Alias untuk `Arc<WorkshopState<...>>` — dipakai di parameter
/// `State<...>` setiap handler, pola sama seperti `SharedState` di Core.
pub type SharedWorkshopState<BR, SR> = Arc<WorkshopState<BR, SR>>;

impl<BR: BusinessRepository, SR: ServiceOrderRepository> WorkshopState<BR, SR> {
    /// `business_service` diterima sudah jadi (bukan `BusinessRepository`
    /// mentah) — dipanggil dengan `core_state.business_service.clone()`
    /// di titik komposisi (`main.rs`/test), supaya Workshop memakai
    /// instance Repository YANG SAMA dengan Core, bukan koneksi/state
    /// terpisah yang bisa berbeda data.
    pub fn new(business_service: BusinessService<BR>, service_order_repository: SR) -> Self {
        Self {
            business_service,
            service_order_service: ServiceOrderService::new(service_order_repository),
        }
    }
}
