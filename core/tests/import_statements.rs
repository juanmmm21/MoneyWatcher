//! Tests de integración del importador contra extractos sintéticos que imitan
//! los formatos reales de la banca española, británica y estadounidense.

use moneywatcher_core::domain::{AccountId, Money};
use moneywatcher_core::importer::{
    parse_csv, parse_statement, AmountColumns, ImportError, StatementSource,
};

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

    assert_eq!(preview.source, StatementSource::Csv { delimiter: ';' });
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

    assert_eq!(preview.source, StatementSource::Csv { delimiter: ',' });
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

    assert_eq!(preview.source, StatementSource::Csv { delimiter: ',' });
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

/// La comprobación que decide si un formato nuevo se ha entendido bien: el
/// salto entre dos saldos consecutivos tiene que ser el importe de en medio.
#[test]
fn balance_check_confirms_a_statement_that_adds_up() {
    let preview = parse_csv(&fixture("bank_es_semicolon.csv")).expect("extracto legible");
    let check = preview.balance_check().expect("el extracto trae saldos");

    assert!(check.is_consistent(), "{:?}", check.mismatches);
    assert!(
        check.oldest_first,
        "las filas van del más antiguo al más reciente"
    );
    assert_eq!(check.matched, 4);

    let uk = parse_csv(&fixture("bank_uk_debit_credit.csv")).expect("extracto legible");
    assert!(uk.balance_check().expect("trae saldos").is_consistent());
}

/// Sin columna de saldo no hay nada que contrastar, y decirlo es mejor que
/// dar por bueno un extracto que nadie ha comprobado.
#[test]
fn balance_check_is_absent_without_a_balance_column() {
    let preview = parse_csv(&fixture("bank_us_iso.csv")).expect("extracto legible");
    assert!(preview.balance_check().is_none());
}

/// Extracto en orden inverso con la comisión en su propia columna: sale de la
/// cuenta además del importe, así que hay que descontarla o el saldo no cuadra.
#[test]
fn fees_charged_apart_are_subtracted_from_the_amount() {
    let preview = parse_csv(&fixture("bank_newest_first_fee.csv")).expect("extracto legible");

    assert!(preview.fee_applied);
    let check = preview.balance_check().expect("el extracto trae saldos");
    assert!(check.is_consistent(), "{:?}", check.mismatches);
    assert!(
        !check.oldest_first,
        "las filas van de la más reciente a la más antigua"
    );

    let transfer = &preview.rows[1];
    assert_eq!(transfer.amount, Money::from_minor_units(-50_000));
    assert_eq!(transfer.fee, Some(Money::from_minor_units(198)));

    // Una fila cuyo único movimiento es la comisión también sale de la cuenta.
    assert_eq!(preview.rows[3].amount, Money::from_minor_units(-1_599));
}

/// El mismo nombre de columna, el uso contrario: aquí el importe ya trae la
/// comisión dentro y restarla otra vez la cobraría dos veces. Lo decide el
/// saldo del extracto, no el nombre del banco.
#[test]
fn fees_already_inside_the_amount_are_left_alone() {
    let preview = parse_csv(&fixture("bank_fee_already_included.csv")).expect("extracto legible");

    assert!(!preview.fee_applied);
    assert_eq!(preview.rows[1].amount, Money::from_minor_units(-10_000));
    assert_eq!(preview.rows[1].fee, Some(Money::from_minor_units(200)));
    assert!(preview
        .balance_check()
        .expect("trae saldos")
        .is_consistent());
}

/// Sin columna de saldo no hay con qué comprobarlo, y tocar el importe a ciegas
/// es peor que dejarlo como viene.
#[test]
fn fees_are_not_touched_without_a_balance_to_check_against() {
    let preview = parse_csv(&fixture("bank_fee_without_balance.csv")).expect("extracto legible");

    assert!(!preview.fee_applied);
    assert_eq!(preview.rows[1].amount, Money::from_minor_units(-10_000));
    assert_eq!(preview.rows[1].fee, Some(Money::from_minor_units(200)));
}

/// Varios bancos españoles solo dejan descargar el extracto en Excel. La hoja
/// no trae texto: las fechas son fechas de Excel y los importes números, así
/// que hay que convertirlas antes de que el resto del importador las vea.
#[test]
fn reads_a_spreadsheet_with_real_dates_and_numeric_amounts() {
    let preview = parse_statement(&fixture("bank_es_workbook.xlsx")).expect("libro legible");

    assert_eq!(
        preview.source,
        StatementSource::Excel {
            sheet: "Movimientos".into()
        },
        "de un libro con portada y movimientos se elige la hoja con la tabla"
    );
    assert_eq!(preview.header_line, 3, "la cabecera va tras el título");
    assert_eq!(preview.rows.len(), 4);

    let salary = &preview.rows[0];
    assert_eq!(salary.description, "NOMINA MARZO EMPRESA SL");
    assert_eq!(salary.booked_on.to_string(), "2026-03-01");
    assert_eq!(
        salary.value_on.map(|date| date.to_string()),
        Some("2026-03-01".to_string())
    );
    assert_eq!(salary.amount, Money::from_minor_units(185_000));
    assert_eq!(salary.balance_after, Some(Money::from_minor_units(215_045)));

    // Un importe con decimales no puede perder céntimos al pasar por la hoja.
    assert_eq!(preview.rows[1].amount, Money::from_minor_units(-4_512));
    assert_eq!(preview.rows[2].amount, Money::from_minor_units(-7_290));

    // La fila de totales no tiene fecha y se descarta explicando por qué.
    assert_eq!(preview.skipped.len(), 1);
    assert!(preview.skipped[0].reason.contains("Total periodo"));

    assert!(preview
        .balance_check()
        .expect("la hoja trae saldos")
        .is_consistent());
}

/// El formato se decide por el contenido: un CSV sigue leyéndose como CSV
/// aunque ahora exista el camino de las hojas de cálculo.
#[test]
fn plain_csv_still_goes_through_the_csv_reader() {
    let preview = parse_statement(&fixture("bank_us_iso.csv")).expect("extracto legible");
    assert_eq!(preview.source, StatementSource::Csv { delimiter: ',' });
}
