//! Lectura de extractos en CSV: encoding, delimitador y celdas. Todo lo que
//! pasa después (cabecera, columnas, importes) es común a todos los formatos y
//! vive en `statement`.

use super::decode::decode;
use super::statement::{
    build_preview, detect_date_order, find_header_in, Candidate, GridRow, ImportError,
    StatementPreview, StatementSource,
};

/// Delimitadores que se prueban al abrir un extracto, en orden de frecuencia
/// en la banca europea: punto y coma, coma, tabulador y barra vertical.
const DELIMITERS: [u8; 4] = *b";,\t|";

/// Lee un extracto CSV detectando por su cuenta delimitador, codificación,
/// fila de cabecera, mapeo de columnas y formato de fecha.
pub fn parse_csv(bytes: &[u8]) -> Result<StatementPreview, ImportError> {
    let text = decode(bytes);
    if text.trim().is_empty() {
        return Err(ImportError::Empty);
    }

    // Se prueban todos los delimitadores y gana el que produce una tabla con
    // sentido: uno equivocado deja casi todo en una sola columna.
    let mut best: Option<(usize, Candidate)> = None;
    for delimiter in DELIMITERS {
        let rows = read_rows(&text, delimiter)?;
        let Some(candidate) = find_header_in(
            &rows,
            StatementSource::Csv {
                delimiter: delimiter as char,
            },
        ) else {
            continue;
        };
        let score = candidate.score();
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, candidate));
        }
    }

    let candidate = best.ok_or(ImportError::HeaderNotFound)?.1;
    let order = detect_date_order(&candidate);
    let preview = build_preview(candidate, order);

    if preview.rows.is_empty() {
        return Err(ImportError::NoValidRows);
    }

    Ok(preview)
}

/// Celdas del fichero con su número de línea. `flexible` permite que las filas
/// del preámbulo tengan menos columnas que la tabla sin abortar la lectura.
fn read_rows(text: &str, delimiter: u8) -> Result<Vec<GridRow>, ImportError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut rows: Vec<GridRow> = Vec::new();
    for record in reader.records() {
        let record = record?;
        let line = record
            .position()
            .map(|position| position.line())
            .unwrap_or(0);
        rows.push((
            line,
            record
                .iter()
                .map(|field| field.trim().to_string())
                .collect(),
        ));
    }
    Ok(rows)
}
