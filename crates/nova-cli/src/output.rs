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
