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

pub mod error;
pub mod service_order;

pub use error::WorkshopError;
pub use service_order::{
    PersistedServiceOrder, ServiceOrder, ServiceOrderDescription, ServiceOrderId,
    ServiceOrderStatus,
};
