//! Herramienta de diagnóstico: enseña qué entiende el importador de un fichero
//! real sin tocar la base de datos ni la interfaz.
//!
//! `cargo run -p moneywatcher-core --example inspect_statement -- <ruta.csv>`

use moneywatcher_core::importer::parse_csv;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("uso: inspect_statement <ruta.csv>");
        std::process::exit(2);
    };

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("no se pudo leer {path}: {error}");
            std::process::exit(1);
        }
    };

    match parse_csv(&bytes) {
        Ok(preview) => {
            println!("delimitador   : {:?}", preview.delimiter);
            println!("línea cabecera: {}", preview.header_line);
            println!("cabeceras     : {:?}", preview.headers);
            println!("mapeo         : {:?}", preview.mapping);
            println!("filas leídas  : {}", preview.rows.len());
            println!("filas saltadas: {}", preview.skipped.len());
            println!(
                "suma importes : {}",
                preview.total_amount().to_decimal_string()
            );
            for skipped in preview.skipped.iter().take(5) {
                println!("  línea {}: {}", skipped.line, skipped.reason);
            }
            for row in preview.rows.iter().take(5) {
                println!(
                    "  {} | {} | {}",
                    row.booked_on,
                    row.description,
                    row.amount.to_decimal_string()
                );
            }
        }
        Err(error) => println!("ERROR del importador: {error}"),
    }
}
