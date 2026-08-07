//! Core Domain: Tenant, Business, Customer & Transaction.
//!
//! Modul ini murni domain logic:
//! - Tidak bergantung pada framework, database, HTTP, atau UI.
//! - Semua Value Object memvalidasi dirinya sendiri saat dibuat.
//! - Business rule lintas-entity (mis. keunikan nama per Tenant) ada di
//!   modul `rules`, berupa fungsi murni yang menerima data sebagai
//!   parameter — bukan mengakses database secara langsung.

pub mod business;
pub mod customer;
pub mod error;
pub mod rules;
pub mod tenant;
pub mod transaction;

pub use business::{Business, BusinessId, BusinessName, BusinessType};
pub use customer::{Customer, CustomerId, CustomerName, CustomerPhone};
pub use error::DomainError;
pub use tenant::{Tenant, TenantId, TenantName};
pub use transaction::{Transaction, TransactionAmount, TransactionId, TransactionKind};
