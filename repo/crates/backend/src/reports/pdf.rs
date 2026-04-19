//! PDF report rendering via `printpdf`.
//!
//! Generates a simple single-page report with a title section and a data
//! table rendered as text. The `printpdf` crate is fully offline — no
//! external process or headless-browser dependency.

use std::path::Path;

use printpdf::*;
use serde_json::Value;

use crate::errors::AppResult;

/// Render a report to `output_path` as a PDF file.
///
/// `title` is the report name shown at the top. `rows` is a slice of JSON
/// objects where keys become column headers and values are the cell data.
pub fn render(title: &str, rows: &[Value], output_path: &Path) -> AppResult<()> {
    let (doc, page1, layer1) =
        PdfDocument::new(title, Mm(210.0), Mm(297.0), "Layer 1");
    let page = doc.get_page(page1);
    let layer = page.get_layer(layer1);

    let font = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?;
    let font_regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?;

    // Title
    layer.use_text(title, 18.0, Mm(15.0), Mm(280.0), &font);

    // Generated-at line
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    layer.use_text(
        format!("Generated: {}", now),
        9.0,
        Mm(15.0),
        Mm(273.0),
        &font_regular,
    );

    if rows.is_empty() {
        layer.use_text("No data available.", 11.0, Mm(15.0), Mm(263.0), &font_regular);
    } else {
        // Collect column names from the first row
        let columns: Vec<String> = rows[0]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        // Header row
        let col_width = 35.0f32;
        let row_height = 7.0f32;
        let start_y = 260.0f32;
        let start_x = 15.0f32;

        for (i, col) in columns.iter().enumerate() {
            layer.use_text(
                col.as_str(),
                9.0,
                Mm(start_x + (i as f32 * col_width)),
                Mm(start_y),
                &font,
            );
        }

        // Data rows (max 30 to avoid page overflow)
        for (row_idx, row) in rows.iter().enumerate().take(30) {
            let y = start_y - (row_idx as f32 + 1.0) * row_height;
            if let Some(obj) = row.as_object() {
                for (col_idx, col) in columns.iter().enumerate() {
                    let val = obj
                        .get(col)
                        .map(|v| {
                            if v.is_string() {
                                v.as_str().unwrap_or("").to_string()
                            } else {
                                v.to_string()
                            }
                        })
                        .unwrap_or_default();
                    // Truncate long strings so they fit in the cell
                    let display: String = val.chars().take(18).collect();
                    layer.use_text(
                        display,
                        8.0,
                        Mm(start_x + (col_idx as f32 * col_width)),
                        Mm(y),
                        &font_regular,
                    );
                }
            }
        }
        if rows.len() > 30 {
            let y = start_y - 31.0 * row_height;
            layer.use_text(
                format!("... {} more rows", rows.len() - 30),
                8.0,
                Mm(start_x),
                Mm(y),
                &font_regular,
            );
        }
    }

    // Write to file
    let file = std::fs::File::create(output_path)
        .map_err(|e| crate::errors::AppError::Internal(format!("pdf create: {}", e)))?;
    let mut buf = std::io::BufWriter::new(file);
    doc.save(&mut buf)
        .map_err(|e| crate::errors::AppError::Internal(format!("pdf save: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render;
    use serde_json::json;

    fn tmpfile(n: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("terraops-pdf-{}-{}.pdf", n, std::process::id()));
        p
    }

    #[test]
    fn empty_rows_renders() {
        let p = tmpfile("empty");
        render("Empty Report", &[], &p).unwrap();
        let md = std::fs::metadata(&p).unwrap();
        assert!(md.len() > 100);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn with_rows_renders() {
        let rows: Vec<_> = (0..3)
            .map(|i| json!({"id": i, "name": format!("row-{}", i), "flag": true}))
            .collect();
        let p = tmpfile("rows");
        render("Rep", &rows, &p).unwrap();
        let md = std::fs::metadata(&p).unwrap();
        assert!(md.len() > 200);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn many_rows_trigger_truncation_line() {
        let rows: Vec<_> = (0..50)
            .map(|i| json!({"id": i, "name": format!("r{}", i)}))
            .collect();
        let p = tmpfile("trunc");
        render("Big", &rows, &p).unwrap();
        let md = std::fs::metadata(&p).unwrap();
        assert!(md.len() > 300);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn bad_path_errors() {
        let p = std::path::PathBuf::from("/no/such/dir/xx.pdf");
        let err = render("T", &[], &p);
        assert!(err.is_err());
    }
}
