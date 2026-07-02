use serde::Serialize;

pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        for h in headers {
            print!("  {h}  ");
        }
        println!();
        return;
    }

    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = col_widths.get_mut(i) {
                *w = (*w).max(cell.len());
            }
        }
    }

    for (i, h) in headers.iter().enumerate() {
        if let Some(w) = col_widths.get(i) {
            print!("  {h:width$}  ", width = w);
        }
    }
    println!();

    for w in &col_widths {
        print!("  {:->width$}  ", "", width = w);
    }
    println!();

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = col_widths.get(i) {
                print!("  {cell:width$}  ", width = w);
            }
        }
        println!();
    }
}

pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_yaml<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_table_empty_rows() {
        print_table(&["Name", "Age"], &[]);
    }

    #[test]
    fn test_print_table_single_row() {
        print_table(&["Name", "Age"], &[vec!["Alice".into(), "30".into()]]);
    }

    #[test]
    fn test_print_table_multiple_rows() {
        let rows = vec![
            vec!["Alice".into(), "30".into()],
            vec!["Bob".into(), "25".into()],
            vec!["Charlie".into(), "35".into()],
        ];
        print_table(&["Name", "Age"], &rows);
    }

    #[test]
    fn test_print_table_uneven_widths() {
        let rows = vec![
            vec!["tiny".into(), "very long cell content".into()],
            vec!["much longer text".into(), "short".into()],
        ];
        print_table(&["Col A", "Col B"], &rows);
    }

    #[test]
    fn test_print_table_single_column() {
        print_table(&["Only"], &[vec!["row1".into()], vec!["row2".into()]]);
    }

    #[test]
    fn test_print_json_valid() {
        let data = serde_json::json!({"name": "test", "value": 42});
        assert!(print_json(&data).is_ok());
    }

    #[test]
    fn test_print_json_array() {
        let data = serde_json::json!([1, 2, 3]);
        assert!(print_json(&data).is_ok());
    }

    #[test]
    fn test_print_yaml_valid() {
        let data = serde_json::json!({"key": "value"});
        assert!(print_yaml(&data).is_ok());
    }

    #[test]
    fn test_print_yaml_nested() {
        let data = serde_json::json!({"outer": {"inner": "v", "list": [1, 2]}});
        assert!(print_yaml(&data).is_ok());
    }
}
