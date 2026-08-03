//! Application layer: orkestrasi use-case.
//!
//! Crate ini menjembatani Domain (murni, tanpa I/O) dengan dunia luar
//! (database, API) lewat trait Repository. Repository di sini masih
//! berupa *interface* (port) — implementasi konkret (Postgres, dll)
//! menyusul di crate terpisah nanti, bukan di sini.
//!
//! Repository sengaja dibuat SYNC dulu (bukan async) karena belum ada
//! kebutuhan nyata untuk async — akan direvisi saat Postgres benar-benar
//! disambungkan.

pub mod business_service;
pub mod error;
pub mod repository;
pub mod tenant_service;

#[cfg(test)]
mod test_support;

pub use business_service::BusinessService;
pub use error::{ApplicationError, RepositoryError};
pub use repository::{BusinessRepository, TenantRepository};
pub use tenant_service::TenantService;
