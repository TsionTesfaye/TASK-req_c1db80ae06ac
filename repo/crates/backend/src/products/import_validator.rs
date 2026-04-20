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

    // Optional: shelf_life_days — must be a non-negative integer if present
    if let Some(v) = raw.get("shelf_life_days") {
        if !v.is_null() {
            let ok = if let Some(n) = v.as_i64() {
                n >= 0
            } else if let Some(s) = v.as_str() {
                if s.trim().is_empty() {
                    true
                } else {
                    s.trim().parse::<i64>().map(|n| n >= 0).unwrap_or(false)
                }
            } else {
                false
            };
            if !ok {
                errors.push("shelf_life_days: must be a non-negative integer".into());
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
///
/// Returns tuple with extended fields so the commit path can persist the
/// full product master-data model (SKU, SPU grouping key, barcode, shelf
/// life window) in one pass:
///   (sku, spu, barcode, shelf_life_days, name, on_shelf, price_cents, currency)
pub fn to_product_fields(
    raw: &Value,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<i32>,
    String,
    bool,
    i32,
    String,
) {
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

    let spu = raw
        .get("spu")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let barcode = raw
        .get("barcode")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let shelf_life_days = raw.get("shelf_life_days").and_then(|v| {
        if let Some(n) = v.as_i64() {
            Some(n as i32)
        } else if let Some(s) = v.as_str() {
            s.trim().parse::<i32>().ok()
        } else {
            None
        }
    });

    (sku, spu, barcode, shelf_life_days, name, on_shelf, price_cents, currency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_full_row() {
        let r = json!({"sku":"A1","name":"Widget","price_cents":1299,"currency":"USD","on_shelf":true});
        assert!(validate_row(&r).is_empty());
    }

    #[test]
    fn valid_minimal_row() {
        let r = json!({"sku":"B","name":"N"});
        assert!(validate_row(&r).is_empty());
    }

    #[test]
    fn missing_sku_errors() {
        let r = json!({"name":"Widget"});
        assert!(validate_row(&r).iter().any(|e| e.contains("sku: required")));
    }

    #[test]
    fn blank_sku_errors() {
        let r = json!({"sku":"   ","name":"Widget"});
        assert!(validate_row(&r).iter().any(|e| e.contains("sku: must not be blank")));
    }

    #[test]
    fn missing_name_errors() {
        let r = json!({"sku":"A1"});
        assert!(validate_row(&r).iter().any(|e| e.contains("name: required")));
    }

    #[test]
    fn blank_name_errors() {
        let r = json!({"sku":"A1","name":""});
        assert!(validate_row(&r).iter().any(|e| e.contains("name: required")
            || e.contains("name: must not be blank")));
    }

    #[test]
    fn negative_price_errors() {
        let r = json!({"sku":"A","name":"n","price_cents":-1});
        assert!(validate_row(&r).iter().any(|e| e.contains("price_cents")));
    }

    #[test]
    fn price_string_ok_when_parseable() {
        let r = json!({"sku":"A","name":"n","price_cents":"42"});
        assert!(validate_row(&r).is_empty());
    }

    #[test]
    fn price_unparseable_errors() {
        let r = json!({"sku":"A","name":"n","price_cents":"abc"});
        assert!(validate_row(&r).iter().any(|e| e.contains("price_cents")));
    }

    #[test]
    fn price_bool_errors() {
        let r = json!({"sku":"A","name":"n","price_cents":true});
        assert!(validate_row(&r).iter().any(|e| e.contains("price_cents")));
    }

    #[test]
    fn currency_wrong_length_errors() {
        let r = json!({"sku":"A","name":"n","currency":"USDX"});
        assert!(validate_row(&r).iter().any(|e| e.contains("currency")));
    }

    #[test]
    fn currency_non_alpha_errors() {
        let r = json!({"sku":"A","name":"n","currency":"US1"});
        assert!(validate_row(&r).iter().any(|e| e.contains("currency")));
    }

    #[test]
    fn currency_empty_is_ok() {
        let r = json!({"sku":"A","name":"n","currency":""});
        assert!(validate_row(&r).is_empty());
    }

    #[test]
    fn on_shelf_bool_ok() {
        let r = json!({"sku":"A","name":"n","on_shelf":false});
        assert!(validate_row(&r).is_empty());
    }

    #[test]
    fn on_shelf_string_truthy_ok() {
        for v in ["true","false","1","0","yes","no"] {
            let r = json!({"sku":"A","name":"n","on_shelf":v});
            assert!(validate_row(&r).is_empty(), "value {v}");
        }
    }

    #[test]
    fn on_shelf_bad_string_errors() {
        let r = json!({"sku":"A","name":"n","on_shelf":"maybe"});
        assert!(validate_row(&r).iter().any(|e| e.contains("on_shelf")));
    }

    #[test]
    fn on_shelf_number_errors() {
        let r = json!({"sku":"A","name":"n","on_shelf":42});
        assert!(validate_row(&r).iter().any(|e| e.contains("on_shelf")));
    }

    #[test]
    fn to_fields_defaults() {
        let r = json!({"sku":"  x ","name":" Y  "});
        let (sku, spu, barcode, shelf, name, on_shelf, price, cur) = to_product_fields(&r);
        assert_eq!(sku, "x");
        assert_eq!(name, "Y");
        assert!(spu.is_none());
        assert!(barcode.is_none());
        assert!(shelf.is_none());
        assert!(on_shelf); // default true
        assert_eq!(price, 0);
        assert_eq!(cur, "USD");
    }

    #[test]
    fn to_fields_string_on_shelf_false() {
        let r = json!({"sku":"a","name":"b","on_shelf":"false"});
        let (_, _, _, _, _, on_shelf, _, _) = to_product_fields(&r);
        assert!(!on_shelf);
    }

    #[test]
    fn to_fields_string_on_shelf_true_variants() {
        for v in ["true","1","yes","YES","True"] {
            let r = json!({"sku":"a","name":"b","on_shelf":v});
            let (_, _, _, _, _, on_shelf, _, _) = to_product_fields(&r);
            assert!(on_shelf, "value {v}");
        }
    }

    #[test]
    fn to_fields_price_from_string() {
        let r = json!({"sku":"a","name":"b","price_cents":"99"});
        let (_, _, _, _, _, _, price, _) = to_product_fields(&r);
        assert_eq!(price, 99);
    }

    #[test]
    fn to_fields_price_garbage_is_zero() {
        let r = json!({"sku":"a","name":"b","price_cents":"abc"});
        let (_, _, _, _, _, _, price, _) = to_product_fields(&r);
        assert_eq!(price, 0);
    }

    #[test]
    fn to_fields_currency_normalization() {
        let r = json!({"sku":"a","name":"b","currency":" eur "});
        let (_, _, _, _, _, _, _, cur) = to_product_fields(&r);
        assert_eq!(cur, "EUR");
    }

    #[test]
    fn to_fields_currency_bad_falls_back_usd() {
        let r = json!({"sku":"a","name":"b","currency":"EUROS"});
        let (_, _, _, _, _, _, _, cur) = to_product_fields(&r);
        assert_eq!(cur, "USD");
    }

    #[test]
    fn to_fields_extended_product_master_data() {
        let r = json!({
            "sku": "EXT-1", "spu": "GROUP-A",
            "barcode": " 0123456789012 ", "shelf_life_days": 7,
            "name": "Milk"
        });
        let (sku, spu, barcode, shelf, name, _, _, _) = to_product_fields(&r);
        assert_eq!(sku, "EXT-1");
        assert_eq!(spu.as_deref(), Some("GROUP-A"));
        assert_eq!(barcode.as_deref(), Some("0123456789012"));
        assert_eq!(shelf, Some(7));
        assert_eq!(name, "Milk");
    }

    #[test]
    fn to_fields_shelf_life_from_string() {
        let r = json!({"sku":"a","name":"b","shelf_life_days":"14"});
        let (_, _, _, shelf, _, _, _, _) = to_product_fields(&r);
        assert_eq!(shelf, Some(14));
    }

    #[test]
    fn negative_shelf_life_errors() {
        let r = json!({"sku":"a","name":"b","shelf_life_days":-3});
        assert!(validate_row(&r).iter().any(|e| e.contains("shelf_life_days")));
    }

    #[test]
    fn unparseable_shelf_life_errors() {
        let r = json!({"sku":"a","name":"b","shelf_life_days":"xyz"});
        assert!(validate_row(&r).iter().any(|e| e.contains("shelf_life_days")));
    }
}
