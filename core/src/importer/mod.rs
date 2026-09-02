//! Lectura de extractos bancarios. El importador no conoce la base de datos:
//! devuelve filas ya interpretadas para que la capa superior decida qué hacer
//! con ellas, lo que permite enseñar una vista previa antes de guardar nada.

mod csv_statement;
mod dates;
mod decode;
mod excel;
mod mapping;
mod statement;

pub use csv_statement::parse_csv;
pub use excel::parse_excel;
pub use mapping::{AmountColumns, ColumnMapping};
pub use statement::{
    BalanceCheck, BalanceMismatch, ImportError, ParsedRow, SkippedRow, StatementPreview,
    StatementSource,
};

/// Lee un extracto sea cual sea su formato. El formato se decide por el
/// contenido y no por la extensión: un fichero renombrado a `.csv` sigue siendo
/// un libro de Excel, y el usuario no tiene por qué saberlo.
pub fn parse_statement(bytes: &[u8]) -> Result<StatementPreview, ImportError> {
    if is_excel(bytes) {
        parse_excel(bytes)
    } else {
        parse_csv(bytes)
    }
}

/// `.xlsx` y `.ods` son ficheros ZIP; `.xls` es un Compound File Binary de los
/// de Office 97. Los dos se reconocen por sus primeros bytes.
fn is_excel(bytes: &[u8]) -> bool {
    const ZIP: &[u8] = b"PK\x03\x04";
    const COMPOUND_FILE: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

    bytes.starts_with(ZIP) || bytes.starts_with(COMPOUND_FILE)
}
