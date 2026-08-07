use serde::{Deserialize, Serialize};

use domain::{Business, Customer, Tenant};

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    /// Id yang ditentukan client (opsional). Kalau diisi, dipakai untuk
    /// idempotent create: retry request create yang sama (Id sama) tidak
    /// akan membuat Tenant duplikat — Tenant yang sudah ada dikembalikan.
    /// Kalau kosong, server yang generate Id baru (perilaku lama, tanpa
    /// jaminan idempotent).
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub name: String,
    pub expected_version: u32,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    pub expected_version: u32,
}

#[derive(Debug, Deserialize)]
pub struct CreateBusinessRequest {
    /// Sama seperti `CreateTenantRequest::id` — opsional, untuk idempotent
    /// create.
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub business_type: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomerRequest {
    /// Sama seperti `CreateBusinessRequest::id` — opsional, untuk
    /// idempotent create.
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    /// Opsional — Customer boleh dibuat dulu tanpa nomor telepon, dilengkapi
    /// belakangan lewat `PATCH /customers/{id}/phone`.
    #[serde(default)]
    pub phone: Option<String>,
}

/// Body untuk `PATCH /customers/{id}/phone`. Terpisah dari `RenameRequest`
/// karena mengganti nomor telepon adalah use-case berbeda dari mengganti
/// nama (lihat `CustomerService::update_customer_phone`) — juga supaya
/// client tidak perlu mengirim `name` hanya untuk mengganti telepon,
/// sesuai Development Rules: "jangan mengharuskan client selalu mengirim
/// seluruh data".
#[derive(Debug, Deserialize)]
pub struct UpdateCustomerPhoneRequest {
    /// `None`/tidak dikirim berarti menghapus nomor telepon yang tersimpan.
    #[serde(default)]
    pub phone: Option<String>,
    pub expected_version: u32,
}

/// Query param untuk endpoint incremental sync (`GET /tenants
/// ?updated_since=...` dan `GET /tenants/{id}/businesses?updated_since=...`).
#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    /// Timestamp RFC 3339, mis. "2026-08-01T00:00:00Z". Kosong berarti
    /// "sejak awal waktu" — dipakai client untuk full sync pertama kali
    /// (belum pernah sinkron sebelumnya).
    #[serde(default)]
    pub updated_since: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub is_deleted: bool,
}

impl From<&Tenant> for TenantResponse {
    fn from(tenant: &Tenant) -> Self {
        Self {
            id: tenant.id().to_string(),
            name: tenant.name().as_str().to_string(),
            version: tenant.version(),
            is_deleted: tenant.is_deleted(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BusinessResponse {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub business_type: String,
    pub version: u32,
    pub is_deleted: bool,
}

impl From<&Business> for BusinessResponse {
    fn from(business: &Business) -> Self {
        Self {
            id: business.id().to_string(),
            tenant_id: business.tenant_id().to_string(),
            name: business.name().as_str().to_string(),
            business_type: business.business_type().as_str().to_string(),
            version: business.version(),
            is_deleted: business.is_deleted(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CustomerResponse {
    pub id: String,
    pub business_id: String,
    pub name: String,
    pub phone: Option<String>,
    pub version: u32,
    pub is_deleted: bool,
}

impl From<&Customer> for CustomerResponse {
    fn from(customer: &Customer) -> Self {
        Self {
            id: customer.id().to_string(),
            business_id: customer.business_id().to_string(),
            name: customer.name().as_str().to_string(),
            phone: customer.phone().map(|p| p.as_str().to_string()),
            version: customer.version(),
            is_deleted: customer.is_deleted(),
        }
    }
}
