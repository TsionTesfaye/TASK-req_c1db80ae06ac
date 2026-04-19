//! Import row validation rules.
//!
//! Each row in `import_rows.raw` must have these columns:
//!   sku (required, non-empty), name (required, non-empty),
//!   price_cents (optional, integer >= 0), currency (optional, 3-char),
//!   on_shelf (optional, "true"/"false"/"1"/"0")
//!
//! Returns a JSON array of error strings for the row. Empty = valid.

use serde_json::Value;

pub fn validate_row(raw: &Value) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    // Required: sku
    match raw.get("sku").and_then(|v| v.as_str()) {
        None => errors.push("sku: required".into()),
        Some(s) if s.trim().is_empty() => errors.push("sku: must not be blank".into()),
        _ => {}
    }

    // Required: name
    match raw.get("name").and_then(|v| v.as_str()) {
        None => errors.push("name: required".into()),
        Some(s) if s.trim().is_empty() => errors.push("name: must not be blank".into()),
        _ => {}
    }

    // Optional: price_cents — must be a non-negative integer if present
    if let Some(v) = raw.get("price_cents") {
        if !v.is_null() {
            let ok = if let Some(n) = v.as_i64() {
                n >= 0
            } else if let Some(s) = v.as_str() {
                s.trim().parse::<i64>().map(|n| n >= 0).unwrap_or(false)
            } else {
                false
            };
            if !ok {
                errors.push("price_cents: must be a non-negative integer".into());
            }
        }
    }

    // Optional: currency — must be exactly 3 uppercase chars if present
    if let Some(v) = raw.get("currency") {
        if let Some(s) = v.as_str() {
            if !s.is_empty() && (s.len() != 3 || !s.chars().all(|c| c.is_ascii_alphabetic())) {
                errors.push("currency: must be a 3-letter code (e.g. USD)".into());
            }
        }
    }

    // Optional: on_shelf — must be boolean-like if present
    if let Some(v) = raw.get("on_shelf") {
        if !v.is_null() && !v.is_boolean() {
            if let Some(s) = v.as_str() {
                let lower = s.trim().to_lowercase();
                if !["true", "false", "1", "0", "yes", "no"].contains(&lower.as_str()) {
                    errors.push("on_shelf: must be true/false/1/0".into());
                }
            } else {
                errors.push("on_shelf: must be true/false/1/0".into());
            }
        }
    }

    errors
}

/// Convert a validated raw row into a `products` INSERT payload.
pub fn to_product_fields(raw: &Value) -> (String, String, bool, i32, String) {
    let sku = raw
        .get("sku")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let name = raw
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let on_shelf = match raw.get("on_shelf") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => {
            matches!(s.trim().to_lowercase().as_str(), "true" | "1" | "yes")
        }
        _ => true,
    };

    let price_cents = match raw.get("price_cents") {
        Some(v) => {
            if let Some(n) = v.as_i64() {
                n as i32
            } else if let Some(s) = v.as_str() {
                s.trim().parse::<i32>().unwrap_or(0)
            } else {
                0
            }
        }
        None => 0,
    };

    let currency = raw
        .get("currency")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_uppercase())
        .filter(|s| s.len() == 3)
        .unwrap_or_else(|| "USD".to_string());

    (sku, name, on_shelf, price_cents, currency)
}
