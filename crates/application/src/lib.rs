//! Application layer: orkestrasi use-case.
//!
//! Crate ini menjembatani Domain (murni, tanpa I/O) dengan dunia luar
//! (database, API) lewat trait Repository. Repository di sini masih
//! berupa *interface* (port) — implementasi konkret ada di crate terpisah
//! (`infra-postgres` untuk Postgres, `in_memory` di bawah untuk test/API
//! sebelum database tersambung).
//!
//! Repository bersifat async (lihat `repository.rs`) — driver Postgres
//! (sqlx) async, jadi trait ini ikut async supaya tidak perlu blocking di
//! dalam runtime async.

pub mod business_service;
pub mod customer_service;
pub mod error;
pub mod in_memory;
pub mod repository;
pub mod tenant_service;
pub mod transaction_service;

pub use business_service::BusinessService;
pub use customer_service::CustomerService;
pub use error::{ApplicationError, RepositoryError};
pub use in_memory::{
    InMemoryBusinessRepository, InMemoryCustomerRepository, InMemoryTenantRepository,
    InMemoryTransactionRepository,
};
pub use repository::{
    BusinessRepository, CustomerRepository, TenantRepository, TransactionRepository,
};
pub use tenant_service::TenantService;
pub use transaction_service::TransactionService;
