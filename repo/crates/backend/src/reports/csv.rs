//! CSV report rendering using the `csv` crate.

use std::path::Path;

use serde_json::Value;

use crate::errors::AppResult;

/// Write `rows` as a CSV file to `output_path`.
///
/// Column headers are derived from the keys of the first row. All values are
/// serialized as strings.
pub fn render(rows: &[Value], output_path: &Path) -> AppResult<()> {
    let file = std::fs::File::create(output_path)
        .map_err(|e| crate::errors::AppError::Internal(format!("csv create: {}", e)))?;
    let mut wtr = ::csv::Writer::from_writer(file);

    if rows.is_empty() {
        wtr.flush()
            .map_err(|e| crate::errors::AppError::Internal(format!("csv flush: {}", e)))?;
        return Ok(());
    }

    let columns: Vec<String> = rows[0]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    // Write header
    wtr.write_record(&columns)
        .map_err(|e| crate::errors::AppError::Internal(format!("csv header: {}", e)))?;

    // Write rows
    for row in rows {
        if let Some(obj) = row.as_object() {
            let record: Vec<String> = columns
                .iter()
                .map(|col| {
                    obj.get(col)
                        .map(|v| {
                            if v.is_string() {
                                v.as_str().unwrap_or("").to_string()
                            } else {
                                v.to_string()
                            }
                        })
                        .unwrap_or_default()
                })
                .collect();
            wtr.write_record(&record)
                .map_err(|e| crate::errors::AppError::Internal(format!("csv row: {}", e)))?;
        }
    }

    wtr.flush()
        .map_err(|e| crate::errors::AppError::Internal(format!("csv flush: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render;
    use serde_json::json;

    fn tmpfile(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("terraops-csv-test-{}-{}.csv", name, std::process::id()));
        p
    }

    #[test]
    fn empty_rows_writes_empty_file() {
        let p = tmpfile("empty");
        render(&[], &p).unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn renders_headers_and_rows() {
        let rows = vec![
            json!({"a":"1","b":2}),
            json!({"a":"x","b":7}),
        ];
        let p = tmpfile("rows");
        render(&rows, &p).unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.lines().next().unwrap().contains("a"));
        assert!(s.lines().next().unwrap().contains("b"));
        assert!(s.contains("1,2"));
        assert!(s.contains("x,7"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_keys_become_empty() {
        let rows = vec![
            json!({"a":"1","b":2}),
            json!({"a":"z"}),
        ];
        let p = tmpfile("missing");
        render(&rows, &p).unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        // Second row's b column is empty → "z," ends with a trailing blank.
        assert!(s.contains("z,"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn error_path_bad_dir() {
        let p = std::path::PathBuf::from("/no/such/dir/xxx.csv");
        let rows = vec![json!({"a":1})];
        let err = render(&rows, &p).unwrap_err();
        let s = format!("{:?}", err);
        assert!(s.contains("csv create"));
    }
}
