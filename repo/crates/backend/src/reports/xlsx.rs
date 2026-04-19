//! XLSX report rendering using `rust_xlsxwriter`.

use std::path::Path;

use rust_xlsxwriter::Workbook;
use serde_json::Value;

use crate::errors::AppResult;

/// Write `rows` as an XLSX workbook to `output_path`.
pub fn render(title: &str, rows: &[Value], output_path: &Path) -> AppResult<()> {
    let mut workbook = Workbook::new();
    let worksheet = workbook
        .add_worksheet()
        .set_name(title)
        .map_err(|e| crate::errors::AppError::Internal(format!("xlsx sheet name: {}", e)))?;

    if rows.is_empty() {
        workbook
            .save(output_path)
            .map_err(|e| crate::errors::AppError::Internal(format!("xlsx save: {}", e)))?;
        return Ok(());
    }

    let columns: Vec<String> = rows[0]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    // Header row
    for (col_idx, col) in columns.iter().enumerate() {
        worksheet
            .write_string(0, col_idx as u16, col.as_str())
            .map_err(|e| crate::errors::AppError::Internal(format!("xlsx header: {}", e)))?;
    }

    // Data rows
    for (row_idx, row) in rows.iter().enumerate() {
        if let Some(obj) = row.as_object() {
            for (col_idx, col) in columns.iter().enumerate() {
                let val = obj.get(col);
                match val {
                    Some(Value::Number(n)) => {
                        if let Some(f) = n.as_f64() {
                            worksheet
                                .write_number((row_idx + 1) as u32, col_idx as u16, f)
                                .map_err(|e| {
                                    crate::errors::AppError::Internal(format!(
                                        "xlsx number: {}",
                                        e
                                    ))
                                })?;
                        }
                    }
                    Some(Value::Bool(b)) => {
                        worksheet
                            .write_boolean((row_idx + 1) as u32, col_idx as u16, *b)
                            .map_err(|e| {
                                crate::errors::AppError::Internal(format!("xlsx bool: {}", e))
                            })?;
                    }
                    Some(other) => {
                        let s = if other.is_string() {
                            other.as_str().unwrap_or("").to_string()
                        } else {
                            other.to_string()
                        };
                        worksheet
                            .write_string((row_idx + 1) as u32, col_idx as u16, &s)
                            .map_err(|e| {
                                crate::errors::AppError::Internal(format!("xlsx string: {}", e))
                            })?;
                    }
                    None => {}
                }
            }
        }
    }

    workbook
        .save(output_path)
        .map_err(|e| crate::errors::AppError::Internal(format!("xlsx save: {}", e)))?;
    Ok(())
}
