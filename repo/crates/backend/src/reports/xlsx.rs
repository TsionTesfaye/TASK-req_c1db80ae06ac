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

#[cfg(test)]
mod tests {
    use super::render;
    use serde_json::json;

    fn tmpfile(n: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("terraops-xlsx-{}-{}.xlsx", n, std::process::id()));
        p
    }

    #[test]
    fn empty_rows_saves_empty_workbook() {
        let p = tmpfile("empty");
        render("Report", &[], &p).unwrap();
        assert!(p.exists());
        let md = std::fs::metadata(&p).unwrap();
        assert!(md.len() > 0);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn writes_numbers_bools_strings() {
        let rows = vec![
            json!({"n": 42, "b": true, "s": "hi", "x": null}),
            json!({"n": 3.14, "b": false, "s": "there"}),
        ];
        let p = tmpfile("mixed");
        render("My Rep", &rows, &p).unwrap();
        let md = std::fs::metadata(&p).unwrap();
        assert!(md.len() > 100);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn invalid_sheet_name_errors() {
        // Sheet names longer than 31 chars fail set_name.
        let long = "x".repeat(200);
        let p = tmpfile("badname");
        let err = render(&long, &[json!({"a":1})], &p);
        assert!(err.is_err());
    }

    #[test]
    fn bad_path_errors() {
        let p = std::path::PathBuf::from("/no/such/dir/xx.xlsx");
        let err = render("T", &[json!({"a":1})], &p);
        assert!(err.is_err());
    }
}
