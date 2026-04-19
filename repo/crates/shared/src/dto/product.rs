//! Product catalog DTOs (P1–P14).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Product list / detail
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductListItem {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub brand_id: Option<Uuid>,
    pub brand_name: Option<String>,
    pub on_shelf: bool,
    pub price_cents: i32,
    pub currency: String,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductDetail {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub brand_id: Option<Uuid>,
    pub brand_name: Option<String>,
    pub unit_id: Option<Uuid>,
    pub unit_code: Option<String>,
    pub site_id: Option<Uuid>,
    pub site_code: Option<String>,
    pub department_id: Option<Uuid>,
    pub department_code: Option<String>,
    pub on_shelf: bool,
    pub price_cents: i32,
    pub currency: String,
    pub tax_rates: Vec<ProductTaxRateDto>,
    pub images: Vec<ProductImageDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateProductRequest {
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub on_shelf: Option<bool>,
    pub price_cents: Option<i32>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UpdateProductRequest {
    pub sku: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub price_cents: Option<i32>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetOnShelfRequest {
    pub on_shelf: bool,
}

// ---------------------------------------------------------------------------
// Tax rates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductTaxRateDto {
    pub id: Uuid,
    pub product_id: Uuid,
    pub state_code: String,
    pub rate_bp: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateTaxRateRequest {
    pub state_code: String,
    /// Rate in basis points (1 bp = 0.01%).
    pub rate_bp: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UpdateTaxRateRequest {
    pub rate_bp: Option<i32>,
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductImageDto {
    pub id: Uuid,
    pub product_id: Uuid,
    /// Signed URL for reading the image bytes. Computed server-side on GET.
    pub signed_url: String,
    pub content_type: String,
    pub size_bytes: i32,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Change history
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductHistoryEntry {
    pub id: Uuid,
    pub product_id: Uuid,
    pub action: String,
    pub changed_by: Option<Uuid>,
    pub changed_by_name: Option<String>,
    pub changed_at: DateTime<Utc>,
    pub before_json: Option<serde_json::Value>,
    pub after_json: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportRequest {
    pub kind: ExportKind,
    pub filter: Option<ProductFilter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    Csv,
    Xlsx,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProductFilter {
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub on_shelf: Option<bool>,
    pub q: Option<String>,
}
