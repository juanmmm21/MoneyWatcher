use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::{AccountId, ImportId, Money, NewTransaction, TransactionSource};

use super::dates::{self, DateOrder};
use super::decode::decode;
use super::mapping::{self, AmountColumns, ColumnMapping};

/// Delimitadores que se prueban al abrir un extracto, en orden de frecuencia
/// en la banca europea.
const DELIMITERS: [u8; 4] = [b';', b',', b'\t', b'|'];
/// Cuántas filas iniciales se inspeccionan buscando la cabecera. Los extractos
/// suelen traer antes un preámbulo con titular, IBAN y fechas del periodo.
const MAX_HEADER_SCAN: usize = 30;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("the file is empty")]
    Empty,
    #[error("could not find a header row with a date, a description and an amount")]
    HeaderNotFound,
    #[error("the file has a header but no readable movements")]
    NoValidRows,
    #[error("malformed csv: {0}")]
    Csv(#[from] csv::Error),
}

/// Una fila del extracto ya interpretada, antes de tocar la base de datos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRow {
    /// Línea del fichero original (1-based), para poder señalarla en la UI.
    pub line: u64,
    pub booked_on: NaiveDate,
    pub value_on: Option<NaiveDate>,
    pub description: String,
    pub counterparty: Option<String>,
    pub amount: Money,
    pub balance_after: Option<Money>,
}

impl ParsedRow {
    pub fn to_new_transaction(
        &self,
        account_id: AccountId,
        import_id: Option<ImportId>,
    ) -> NewTransaction {
        NewTransaction {
            account_id,
            booked_on: self.booked_on,
            value_on: self.value_on,
            description: self.description.clone(),
            counterparty: self.counterparty.clone(),
            amount: self.amount,
            balance_after: self.balance_after,
            category_id: None,
            notes: None,
            source: TransactionSource::Imported,
            import_id,
        }
    }
}

/// Fila que no se pudo interpretar, con el motivo, para enseñárselo al usuario
/// en vez de descartarla en silencio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRow {
    pub line: u64,
    pub reason: String,
}

/// Resultado de leer un extracto: qué se entendió del fichero y qué filas salen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementPreview {
    pub delimiter: char,
    /// Línea del fichero en la que está la cabecera (ver `ParsedRow::line`).
    pub header_line: u64,
    pub headers: Vec<String>,
    pub mapping: ColumnMapping,
    pub rows: Vec<ParsedRow>,
    pub skipped: Vec<SkippedRow>,
}

impl StatementPreview {
    pub fn total_amount(&self) -> Money {
        self.rows.iter().map(|row| row.amount).sum()
    }
}

/// Lee un extracto CSV detectando por su cuenta delimitador, codificación,
/// fila de cabecera, mapeo de columnas y formato de fecha.
pub fn parse_csv(bytes: &[u8]) -> Result<StatementPreview, ImportError> {
    let text = decode(bytes);
    if text.trim().is_empty() {
        return Err(ImportError::Empty);
    }

    let mut best: Option<(usize, Candidate)> = None;
    for delimiter in DELIMITERS {
        let Some(candidate) = find_header(&text, delimiter)? else {
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

struct Candidate {
    delimiter: u8,
    header_line: u64,
    headers: Vec<String>,
    mapping: ColumnMapping,
    /// Filas de datos con la línea del fichero en la que aparecen, para poder
    /// señalar en la interfaz exactamente qué línea no se pudo leer.
    records: Vec<(u64, Vec<String>)>,
}

impl Candidate {
    /// Un delimitador acertado produce cabeceras reconocibles y filas de datos;
    /// uno equivocado deja casi todo en una sola columna.
    fn score(&self) -> usize {
        let mapped = 3
            + usize::from(self.mapping.value_on.is_some())
            + usize::from(self.mapping.counterparty.is_some())
            + usize::from(self.mapping.balance.is_some());
        mapped * 10 + self.records.len().min(50)
    }
}

fn find_header(text: &str, delimiter: u8) -> Result<Option<Candidate>, ImportError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut rows: Vec<(u64, Vec<String>)> = Vec::new();
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

    for (index, (line, row)) in rows.iter().take(MAX_HEADER_SCAN).enumerate() {
        if row.iter().filter(|cell| !cell.is_empty()).count() < 3 {
            continue;
        }

        let Some(mapping) = mapping::detect(row) else {
            continue;
        };

        let records: Vec<(u64, Vec<String>)> = rows[index + 1..]
            .iter()
            .filter(|(_, data)| data.iter().any(|cell| !cell.is_empty()))
            .cloned()
            .collect();

        if records.is_empty() {
            continue;
        }

        return Ok(Some(Candidate {
            delimiter,
            header_line: *line,
            headers: row.clone(),
            mapping,
            records,
        }));
    }

    Ok(None)
}

fn detect_date_order(candidate: &Candidate) -> DateOrder {
    let samples: Vec<String> = candidate
        .records
        .iter()
        .filter_map(|(_, row)| row.get(candidate.mapping.booked_on).cloned())
        .collect();
    dates::detect_order(&samples)
}

fn build_preview(candidate: Candidate, order: DateOrder) -> StatementPreview {
    let mut rows = Vec::new();
    let mut skipped = Vec::new();

    for (line, record) in &candidate.records {
        let line = *line;
        let raw_date = record
            .get(candidate.mapping.booked_on)
            .cloned()
            .unwrap_or_default();
        let Some(booked_on) = dates::parse(&raw_date, order) else {
            skipped.push(SkippedRow {
                line,
                reason: format!("unreadable date `{raw_date}`"),
            });
            continue;
        };

        let amount = match read_amount(record, &candidate.mapping.amount) {
            Ok(amount) => amount,
            Err(reason) => {
                skipped.push(SkippedRow { line, reason });
                continue;
            }
        };

        let description = cell(record, Some(candidate.mapping.description))
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "(no description)".to_string());

        rows.push(ParsedRow {
            line,
            booked_on,
            value_on: candidate
                .mapping
                .value_on
                .and_then(|index| record.get(index))
                .and_then(|raw| dates::parse(raw, order)),
            description,
            counterparty: cell(record, candidate.mapping.counterparty).filter(|t| !t.is_empty()),
            amount,
            balance_after: cell(record, candidate.mapping.balance)
                .and_then(|raw| Money::parse_flexible(&raw).ok()),
        });
    }

    StatementPreview {
        delimiter: candidate.delimiter as char,
        header_line: candidate.header_line,
        headers: candidate.headers,
        mapping: candidate.mapping,
        rows,
        skipped,
    }
}

fn read_amount(record: &[String], columns: &AmountColumns) -> Result<Money, String> {
    match columns {
        AmountColumns::Single { index } => {
            let raw = record.get(*index).cloned().unwrap_or_default();
            Money::parse_flexible(&raw).map_err(|error| error.to_string())
        }
        AmountColumns::DebitCredit { debit, credit } => {
            let raw_debit = record.get(*debit).cloned().unwrap_or_default();
            let raw_credit = record.get(*credit).cloned().unwrap_or_default();

            // El signo lo decide la columna, no el texto: hay bancos que
            // escriben el cargo en positivo y otros que lo traen ya en negativo.
            let credit_value = Money::parse_flexible(&raw_credit).ok().map(Money::abs);
            let debit_value = Money::parse_flexible(&raw_debit).ok().map(|v| -v.abs());

            match (credit_value, debit_value) {
                (Some(credit), _) if !credit.is_zero() => Ok(credit),
                (_, Some(debit)) if !debit.is_zero() => Ok(debit),
                _ => Err("row has neither debit nor credit amount".to_string()),
            }
        }
    }
}

fn cell(record: &[String], index: Option<usize>) -> Option<String> {
    index
        .and_then(|index| record.get(index))
        .map(|value| value.trim().to_string())
}
