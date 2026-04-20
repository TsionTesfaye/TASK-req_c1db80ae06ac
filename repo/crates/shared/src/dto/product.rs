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
    /// Standard Product Unit — grouping key that buckets multiple SKUs
    /// under one operational product (migration 0012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spu: Option<String>,
    /// GTIN/UPC/EAN barcode used at register + receiving (migration 0012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Operational freshness window in days (migration 0012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shelf_life_days: Option<i32>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shelf_life_days: Option<i32>,
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
    #[serde(default)]
    pub spu: Option<String>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub shelf_life_days: Option<i32>,
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

/// PATCH body for products.
///
/// Optional master-data pointers (`spu`, `barcode`, `shelf_life_days`,
/// `description`, `category_id`, `brand_id`, `unit_id`, `site_id`,
/// `department_id`) use tri-state semantics so the analyst can reassign
/// **or clear** the pointer in a PATCH:
///   * field omitted  → `None`            → leave as-is
///   * `"field": null`→ `Some(None)`      → clear to NULL
///   * `"field": v`   → `Some(Some(v))`   → set to `v`
///
/// Scalar fields (`sku`, `name`, `price_cents`, `currency`) remain plain
/// `Option<T>` because clearing them is not a valid operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UpdateProductRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub spu: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub barcode: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub shelf_life_days: Option<Option<i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub category_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub brand_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub site_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub department_id: Option<Option<Uuid>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_cents: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
