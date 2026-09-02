//! Lectura de extractos en hoja de cálculo (`.xlsx`, `.xls`, `.ods`).
//!
//! Varios bancos españoles solo ofrecen la descarga en Excel, así que pedirle
//! al usuario que convierta el fichero a CSV a mano sería trasladarle un
//! trabajo que la app puede hacer. Una vez convertidas las celdas a texto, el
//! resto del camino es el mismo que el de un CSV.

use std::io::Cursor;

use calamine::{Data, Range, Reader, Sheets};

use super::statement::{
    build_preview, detect_date_order, find_header_in, Candidate, GridRow, ImportError,
    StatementPreview, StatementSource,
};

pub fn parse_excel(bytes: &[u8]) -> Result<StatementPreview, ImportError> {
    let mut workbook: Sheets<Cursor<&[u8]>> =
        calamine::open_workbook_auto_from_rs(Cursor::new(bytes))
            .map_err(|error| ImportError::Excel(error.to_string()))?;

    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err(ImportError::Empty);
    }

    // Un libro puede traer una hoja de portada, otra de resúmenes y la de
    // movimientos: se leen todas y gana la que más se parece a una tabla de
    // movimientos, en vez de dar por hecho que es la primera.
    let mut best: Option<(usize, Candidate)> = None;
    for name in &sheet_names {
        let Ok(range) = workbook.worksheet_range(name) else {
            continue;
        };
        let rows = read_rows(&range);
        let Some(candidate) = find_header_in(
            &rows,
            StatementSource::Excel {
                sheet: name.clone(),
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

/// Convierte la hoja en celdas de texto, numeradas desde 1 como las líneas de
/// un CSV para que la vista previa pueda señalar «la fila 24» y el usuario la
/// encuentre en su hoja.
fn read_rows(range: &Range<Data>) -> Vec<GridRow> {
    range
        .rows()
        .enumerate()
        .map(|(index, cells)| {
            (
                index as u64 + 1,
                cells.iter().map(cell_to_string).collect::<Vec<String>>(),
            )
        })
        .collect()
}

/// Una celda de Excel no es texto: las fechas son números de serie y los
/// importes son coma flotante. Se convierten al formato que ya entiende el
/// resto del importador (fecha ISO y decimal con punto).
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(text) => text.trim().to_string(),
        Data::Int(value) => value.to_string(),
        // Dos decimales fijos: el valor de la hoja ya viene con la precisión
        // del banco, y dejar que Rust imprima el `f64` completo sacaría colas
        // como `12.710000000000001` que el parser de importes rechazaría.
        Data::Float(value) => format!("{value:.2}"),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(|moment| moment.date().to_string())
            .unwrap_or_default(),
        // Llega como `2026-03-01T00:00:00`: al importador le vale la fecha y
        // la hora solo estorbaría al detectar el formato.
        Data::DateTimeIso(text) => text
            .trim()
            .split('T')
            .next()
            .unwrap_or_default()
            .to_string(),
        Data::DurationIso(text) => text.trim().to_string(),
        // Una celda con `#N/A` o `#REF!` no tiene valor que leer; se deja
        // vacía y la fila se descarta más adelante si le hacía falta.
        Data::Error(_) => String::new(),
    }
}
