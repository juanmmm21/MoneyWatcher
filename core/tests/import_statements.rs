//! Tests de integración del importador contra extractos sintéticos que imitan
//! los formatos reales de la banca española, británica y estadounidense.

use moneywatcher_core::domain::{AccountId, Money};
use moneywatcher_core::importer::{parse_csv, AmountColumns, ImportError};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|error| panic!("no se pudo leer la fixture {name}: {error}"))
}

#[test]
fn parses_spanish_statement_with_preamble_and_semicolons() {
    let preview = parse_csv(&fixture("bank_es_semicolon.csv")).expect("extracto legible");

    assert_eq!(preview.delimiter, ';');
    assert_eq!(
        preview.header_line, 5,
        "la cabecera va tras el preámbulo del banco"
    );
    assert_eq!(preview.rows.len(), 5);

    let salary = &preview.rows[0];
    assert_eq!(salary.description, "NOMINA MARZO EMPRESA SL");
    assert_eq!(salary.amount, Money::from_minor_units(185_000));
    assert_eq!(salary.balance_after, Some(Money::from_minor_units(215_045)));
    assert_eq!(salary.booked_on.to_string(), "2026-03-01");

    let groceries = &preview.rows[1];
    assert_eq!(groceries.amount, Money::from_minor_units(-4_512));
    assert_eq!(
        groceries.value_on.map(|d| d.to_string()),
        Some("2026-03-05".to_string())
    );

    // La fila de totales no tiene fecha y se descarta explicando por qué.
    assert_eq!(preview.skipped.len(), 1);
    assert!(preview.skipped[0].reason.contains("unreadable date"));
    assert_eq!(
        preview.skipped[0].line, 12,
        "la fila descartada se identifica por su línea"
    );
}

#[test]
fn parses_debit_and_credit_columns_into_signed_amounts() {
    let preview = parse_csv(&fixture("bank_uk_debit_credit.csv")).expect("extracto legible");

    assert_eq!(preview.delimiter, ',');
    assert!(matches!(
        preview.mapping.amount,
        AmountColumns::DebitCredit { .. }
    ));
    assert_eq!(preview.rows.len(), 4);

    assert_eq!(preview.rows[0].amount, Money::from_minor_units(185_000));
    assert_eq!(preview.rows[1].amount, Money::from_minor_units(-4_512));
    assert_eq!(preview.rows[3].amount, Money::from_minor_units(1_250));
    assert!(preview.skipped.is_empty());
}

#[test]
fn parses_iso_dates_and_reports_broken_rows() {
    let preview = parse_csv(&fixture("bank_us_iso.csv")).expect("extracto legible");

    assert_eq!(preview.rows.len(), 3);
    assert_eq!(preview.rows[0].booked_on.to_string(), "2026-03-01");
    // Sin columna de concepto, la de contraparte hace de concepto.
    assert_eq!(preview.rows[0].description, "ACME PAYROLL");
    assert_eq!(preview.rows[2].description, "(no description)");
    assert_eq!(preview.skipped.len(), 1);
}

#[test]
fn totals_match_the_sum_of_every_row() {
    let preview = parse_csv(&fixture("bank_es_semicolon.csv")).expect("extracto legible");
    assert_eq!(preview.total_amount(), Money::from_minor_units(122_099));
}

#[test]
fn reads_windows_1252_statements() {
    let latin1: Vec<u8> = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"Fecha;Concepto;Importe\n");
        bytes.extend_from_slice(b"01/03/2026;N");
        bytes.push(0xf3); // "ó" en Windows-1252
        bytes.extend_from_slice(b"MINA;1.850,00\n");
        bytes
    };

    let preview = parse_csv(&latin1).expect("extracto legible");
    assert_eq!(preview.rows[0].description, "NóMINA");
}

#[test]
fn converts_rows_into_transactions_ready_to_store() {
    let preview = parse_csv(&fixture("bank_es_semicolon.csv")).expect("extracto legible");
    let account = AccountId(7);

    let transactions: Vec<_> = preview
        .rows
        .iter()
        .map(|row| row.to_new_transaction(account, None))
        .collect();

    assert_eq!(transactions.len(), 5);
    assert!(transactions.iter().all(|t| t.account_id == account));
    assert!(transactions.iter().all(|t| t.category_id.is_none()));
}

#[test]
fn rejects_files_that_are_not_statements() {
    assert!(matches!(parse_csv(b""), Err(ImportError::Empty)));
    assert!(matches!(
        parse_csv(b"just some text\nwithout any table\n"),
        Err(ImportError::HeaderNotFound)
    ));
}

/// Informe de banca electrónica (el formato que exporta Revolut en español):
/// preámbulo largo con los datos de la cuenta, fechas con el mes escrito, una
/// sola columna de importe llamada "Entradas/salidas de dinero" y una tabla por
/// divisa. La fixture es sintética; el caso salió de un extracto real.
#[test]
fn parses_an_electronic_banking_report_with_named_months() {
    let preview = parse_csv(&fixture("bank_report_named_months.csv")).expect("informe legible");

    assert_eq!(preview.delimiter, ',');
    // La cabecera no está en las primeras líneas, sino tras todo el preámbulo.
    assert_eq!(preview.header_line, 21);
    assert_eq!(
        preview.mapping.amount,
        AmountColumns::Single { index: 3 },
        "«Entradas/salidas de dinero» es una columna con signo, no un par cargo/abono"
    );

    assert_eq!(preview.rows.len(), 5);
    assert_eq!(
        preview.rows[0].booked_on,
        chrono::NaiveDate::from_ymd_opt(2026, 2, 12).unwrap()
    );
    assert_eq!(preview.rows[0].amount, Money::from_minor_units(120_000));
    assert_eq!(
        preview.rows[3].booked_on,
        chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        "«30 sept 2026» tiene el mes en abreviatura de cuatro letras"
    );
    assert_eq!(preview.rows[4].amount, Money::from_minor_units(-94_544));
}

/// La segunda tabla es de otra divisa: importarla junto a la primera metería
/// dólares en una cuenta en euros.
#[test]
fn stops_at_the_second_table_and_says_so() {
    let preview = parse_csv(&fixture("bank_report_named_months.csv")).expect("informe legible");

    assert!(
        preview
            .rows
            .iter()
            .all(|row| row.description != "Compra en dólares"),
        "la tabla en dólares no debe entrar en la importación"
    );
    assert!(
        preview
            .skipped
            .iter()
            .any(|skipped| skipped.reason.contains("another table")),
        "la vista previa debe avisar de que el fichero traía más tablas"
    );
}
