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
            match preview.balance_check() {
                Some(check) if check.is_consistent() => println!(
                    "saldo         : cuadra en {} saltos ({})",
                    check.matched,
                    if check.oldest_first {
                        "del más antiguo al más reciente"
                    } else {
                        "del más reciente al más antiguo"
                    }
                ),
                Some(check) => {
                    println!(
                        "saldo         : {} saltos cuadran y {} no",
                        check.matched,
                        check.mismatches.len()
                    );
                    for mismatch in check.mismatches.iter().take(5) {
                        println!(
                            "  línea {}: el saldo se mueve {} pero el importe dice {}",
                            mismatch.line,
                            mismatch.expected.to_decimal_string(),
                            mismatch.found.to_decimal_string()
                        );
                    }
                }
                None => println!("saldo         : el extracto no trae columna de saldo"),
            }

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
