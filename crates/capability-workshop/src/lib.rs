//! Capability Workshop/Bengkel.
//!
//! Dibangun DI ATAS Core Domain (`domain`), BUKAN bagian dari Core.
//! Core Domain (Tenant, Business, Customer, Transaction, Relationship,
//! Interaction) tidak diubah untuk mengakomodasi Workshop — kalau
//! Workshop menemukan kebutuhan yang terasa "generik", itu didiskusikan
//! dulu apakah benar-benar universal sebelum dipindah ke Core.
//!
//! Struktur crate ini SENGAJA belum dipecah domain/application/infra
//! seperti Core — cakupannya masih satu entity (`ServiceOrder`). Dipecah
//! jadi beberapa crate nanti kalau memang tumbuh (keputusan refactor,
//! bukan keputusan di awal).

pub mod dto;
pub mod error;
pub mod http_error;
pub mod in_memory;
pub mod repository;
pub mod routes;
pub mod rules;
pub mod service_order;
pub mod service_order_service;
pub mod state;

pub use error::{RepositoryError, ServiceOrderError, WorkshopError};
pub use http_error::WorkshopApiError;
pub use in_memory::InMemoryServiceOrderRepository;
pub use repository::ServiceOrderRepository;
pub use routes::build_workshop_router;
pub use service_order::{
    PersistedServiceOrder, ServiceOrder, ServiceOrderDescription, ServiceOrderId,
    ServiceOrderStatus,
};
pub use service_order_service::ServiceOrderService;
pub use state::{SharedWorkshopState, WorkshopState};
