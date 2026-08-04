use serde::{Deserialize, Serialize};

use domain::{Business, Tenant};

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
