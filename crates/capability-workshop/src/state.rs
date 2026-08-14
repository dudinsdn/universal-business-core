//! State HTTP milik Capability Workshop.
//!
//! Generik atas dependency yang benar-benar dipakai Workshop:
//! - `BR` (`BusinessRepository`) lewat `BusinessService<BR>` dari Core —
//!   dibutuhkan untuk `get_business` sebelum membuat ServiceOrder.
//! - `CR` (`CustomerRepository`) lewat `CustomerService<CR>` dari Core —
//!   dibutuhkan untuk `get_customer` sebelum membuat ServiceOrder, supaya
//!   `customer_id` bisa divalidasi benar-benar milik Business yang sama
//!   (gap #3: validasi customer_id lintas-aggregate).
//! - `TxR` (`TransactionRepository`) lewat `TransactionService<TxR>` dari
//!   Core — dibutuhkan untuk `get_transaction` sebelum
//!   `complete_service_order`, supaya `transaction_id` yang dikirim client
//!   bisa divalidasi benar-benar milik Business yang sama (pola sama
//!   persis seperti validasi `customer_id`, menutup celah yang sama untuk
//!   `transaction_id`).
//! - `SR` (`ServiceOrderRepository`) milik Workshop sendiri.
//!
//! Ini yang membuat `api::AppState` (Core) tidak perlu tahu Workshop
//! sama sekali, dan sebaliknya Workshop tidak perlu tahu
//! Tenant/Relationship/Interaction Repository — cuma yang dipakainya.

use std::sync::Arc;

use application::{
    BusinessRepository, BusinessService, CustomerRepository, CustomerService,
    TransactionRepository, TransactionService,
};

use crate::repository::ServiceOrderRepository;
use crate::service_order_service::ServiceOrderService;

pub struct WorkshopState<
    BR: BusinessRepository,
    CR: CustomerRepository,
    TxR: TransactionRepository,
    SR: ServiceOrderRepository,
> {
    pub business_service: BusinessService<BR>,
    pub customer_service: CustomerService<CR>,
    pub transaction_service: TransactionService<TxR>,
    pub service_order_service: ServiceOrderService<SR>,
}

/// Alias untuk `Arc<WorkshopState<...>>` — dipakai di parameter
/// `State<...>` setiap handler, pola sama seperti `SharedState` di Core.
pub type SharedWorkshopState<BR, CR, TxR, SR> = Arc<WorkshopState<BR, CR, TxR, SR>>;

impl<
    BR: BusinessRepository,
    CR: CustomerRepository,
    TxR: TransactionRepository,
    SR: ServiceOrderRepository,
> WorkshopState<BR, CR, TxR, SR>
{
    /// `business_service`/`customer_service`/`transaction_service`
    /// diterima sudah jadi (bukan Repository mentah) — dipanggil dengan
    /// `core_state.business_service.clone()`/dst di titik komposisi
    /// (`main.rs`/test), supaya Workshop memakai instance Repository YANG
    /// SAMA dengan Core, bukan koneksi/state terpisah yang bisa berbeda
    /// data.
    pub fn new(
        business_service: BusinessService<BR>,
        customer_service: CustomerService<CR>,
        transaction_service: TransactionService<TxR>,
        service_order_repository: SR,
    ) -> Self {
        Self {
            business_service,
            customer_service,
            transaction_service,
            service_order_service: ServiceOrderService::new(service_order_repository),
        }
    }
}
