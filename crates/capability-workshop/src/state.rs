//! State HTTP milik Capability Workshop.
//!
//! Generik atas dependency yang benar-benar dipakai Workshop:
//! - `BR` (`BusinessRepository`) lewat `BusinessService<BR>` dari Core —
//!   dibutuhkan untuk `get_business` sebelum membuat ServiceOrder.
//! - `CR` (`CustomerRepository`) lewat `CustomerService<CR>` dari Core —
//!   dibutuhkan untuk `get_customer` sebelum membuat ServiceOrder, supaya
//!   `customer_id` bisa divalidasi benar-benar milik Business yang sama
//!   (gap #3: validasi customer_id lintas-aggregate).
//! - `SR` (`ServiceOrderRepository`) milik Workshop sendiri.
//!
//! Ini yang membuat `api::AppState` (Core) tidak perlu tahu Workshop
//! sama sekali, dan sebaliknya Workshop tidak perlu tahu
//! Tenant/Transaction/Relationship/Interaction Repository — cuma yang
//! dipakainya.

use std::sync::Arc;

use application::{BusinessRepository, BusinessService, CustomerRepository, CustomerService};

use crate::repository::ServiceOrderRepository;
use crate::service_order_service::ServiceOrderService;

pub struct WorkshopState<BR: BusinessRepository, CR: CustomerRepository, SR: ServiceOrderRepository>
{
    pub business_service: BusinessService<BR>,
    pub customer_service: CustomerService<CR>,
    pub service_order_service: ServiceOrderService<SR>,
}

/// Alias untuk `Arc<WorkshopState<...>>` — dipakai di parameter
/// `State<...>` setiap handler, pola sama seperti `SharedState` di Core.
pub type SharedWorkshopState<BR, CR, SR> = Arc<WorkshopState<BR, CR, SR>>;

impl<BR: BusinessRepository, CR: CustomerRepository, SR: ServiceOrderRepository>
    WorkshopState<BR, CR, SR>
{
    /// `business_service`/`customer_service` diterima sudah jadi (bukan
    /// Repository mentah) — dipanggil dengan
    /// `core_state.business_service.clone()`/`core_state.customer_service.clone()`
    /// di titik komposisi (`main.rs`/test), supaya Workshop memakai
    /// instance Repository YANG SAMA dengan Core, bukan koneksi/state
    /// terpisah yang bisa berbeda data.
    pub fn new(
        business_service: BusinessService<BR>,
        customer_service: CustomerService<CR>,
        service_order_repository: SR,
    ) -> Self {
        Self {
            business_service,
            customer_service,
            service_order_service: ServiceOrderService::new(service_order_repository),
        }
    }
}
