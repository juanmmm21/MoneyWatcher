//! Lectura de extractos bancarios. El importador no conoce la base de datos:
//! devuelve filas ya interpretadas para que la capa superior decida qué hacer
//! con ellas, lo que permite enseñar una vista previa antes de guardar nada.

mod csv_statement;
mod dates;
mod decode;
mod mapping;

pub use csv_statement::{parse_csv, ImportError, ParsedRow, SkippedRow, StatementPreview};
pub use mapping::{AmountColumns, ColumnMapping};
