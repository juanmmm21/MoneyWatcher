//! Agregaciones que alimentan los widgets del dashboard.
//!
//! Todos los cálculos se hacen en SQL sobre enteros y se devuelven ya listos
//! para pintar: el frontend no vuelve a sumar dinero por su cuenta.

use rusqlite::types::Value;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};

use crate::domain::{CategoryId, Money};
use crate::storage::{build_where, Database, StorageResult, TransactionFilter};

/// Totales del periodo consultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowTotals {
    pub income: Money,
    /// Positivo: es la magnitud gastada, no el importe con signo.
    pub expense: Money,
    pub net: Money,
    /// Porcentaje ahorrado sobre lo ingresado, en puntos básicos (2550 = 25,5 %).
    /// Se expresa en enteros para no introducir flotantes en el núcleo.
    pub savings_rate_bps: i64,
}

/// Ingresos y gastos de un mes natural.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyFlow {
    /// Mes en formato `YYYY-MM`.
    pub month: String,
    pub income: Money,
    pub expense: Money,
    pub net: Money,
}

/// Peso de una categoría dentro del periodo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySlice {
    pub category_id: Option<CategoryId>,
    pub name: String,
    pub color: String,
    pub total: Money,
    pub share_bps: i64,
    pub transactions: i64,
}

/// Resumen por banco: es la vista que el usuario tiene en la cabeza cuando
/// organiza su dinero (una lista de ingresos y otra de gastos por entidad).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankSummary {
    pub bank: String,
    pub accounts: i64,
    pub balance: Money,
    pub income: Money,
    pub expense: Money,
}

/// Comercio o contraparte recurrente, ordenado por gasto acumulado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CounterpartyTotal {
    pub label: String,
    pub total: Money,
    pub transactions: i64,
}

impl Database {
    pub fn flow_totals(&self, filter: &TransactionFilter) -> StorageResult<FlowTotals> {
        let (where_clause, values) = build_where(filter);
        let sql = format!(
            "SELECT
                 COALESCE(SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN t.amount < 0 THEN -t.amount ELSE 0 END), 0)
             FROM transactions t{where_clause}"
        );

        let (income, expense): (i64, i64) = self.connection().query_row(
            &sql,
            params_from_iter(values.iter()),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        Ok(FlowTotals {
            income: Money::from_minor_units(income),
            expense: Money::from_minor_units(expense),
            net: Money::from_minor_units(income - expense),
            savings_rate_bps: savings_rate_bps(income, expense),
        })
    }

    /// Serie mensual, del mes más antiguo al más reciente.
    pub fn monthly_flow(&self, filter: &TransactionFilter) -> StorageResult<Vec<MonthlyFlow>> {
        let (where_clause, values) = build_where(filter);
        let sql = format!(
            "SELECT
                 substr(t.booked_on, 1, 7) AS month,
                 COALESCE(SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN t.amount < 0 THEN -t.amount ELSE 0 END), 0)
             FROM transactions t{where_clause}
             GROUP BY month
             ORDER BY month ASC"
        );

        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            let month: String = row.get(0)?;
            let income: i64 = row.get(1)?;
            let expense: i64 = row.get(2)?;
            Ok(MonthlyFlow {
                month,
                income: Money::from_minor_units(income),
                expense: Money::from_minor_units(expense),
                net: Money::from_minor_units(income - expense),
            })
        })?;

        let mut months = Vec::new();
        for row in rows {
            months.push(row?);
        }
        Ok(months)
    }

    /// Reparto por categoría de un lado del flujo. Los movimientos sin
    /// categorizar se agrupan aparte en vez de desaparecer del gráfico.
    pub fn category_breakdown(
        &self,
        filter: &TransactionFilter,
    ) -> StorageResult<Vec<CategorySlice>> {
        let (where_clause, values) = build_where(filter);
        let sql = format!(
            "SELECT
                 t.category_id,
                 COALESCE(c.name, 'Uncategorized'),
                 COALESCE(c.color, '#8a8a8a'),
                 COALESCE(SUM(ABS(t.amount)), 0),
                 COUNT(*)
             FROM transactions t
             LEFT JOIN categories c ON c.id = t.category_id{where_clause}
             GROUP BY t.category_id
             ORDER BY 4 DESC"
        );

        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;

        let mut slices = Vec::new();
        for row in rows {
            let (category_id, name, color, total, transactions) = row?;
            slices.push(CategorySlice {
                category_id: category_id.map(CategoryId),
                name,
                color,
                total: Money::from_minor_units(total),
                share_bps: 0,
                transactions,
            });
        }

        let grand_total: i64 = slices.iter().map(|slice| slice.total.minor_units()).sum();
        if grand_total > 0 {
            for slice in &mut slices {
                slice.share_bps = slice.total.minor_units() * 10_000 / grand_total;
            }
        }

        Ok(slices)
    }

    /// Balance y flujo agrupados por entidad bancaria.
    pub fn bank_summaries(&self, filter: &TransactionFilter) -> StorageResult<Vec<BankSummary>> {
        let (where_clause, mut values) = build_where(filter);
        // El balance incluye el saldo de apertura de cada cuenta, así que se
        // calcula sobre todos los movimientos y no solo sobre los del filtro.
        let flow_clause = where_clause.replace(" WHERE ", " AND ");
        let placeholder_offset = values.len();
        let mut flow_values: Vec<Value> = values.clone();
        values.append(&mut flow_values);

        let shifted_clause = shift_placeholders(&flow_clause, placeholder_offset);

        let sql = format!(
            "SELECT
                 a.bank,
                 COUNT(DISTINCT a.id),
                 COALESCE(SUM(a.opening_balance), 0) + COALESCE((
                     SELECT SUM(t.amount) FROM transactions t
                     JOIN accounts inner_a ON inner_a.id = t.account_id
                     WHERE inner_a.bank = a.bank
                 ), 0),
                 COALESCE((
                     SELECT SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END)
                     FROM transactions t
                     JOIN accounts inner_a ON inner_a.id = t.account_id
                     WHERE inner_a.bank = a.bank{shifted_clause}
                 ), 0),
                 COALESCE((
                     SELECT SUM(CASE WHEN t.amount < 0 THEN -t.amount ELSE 0 END)
                     FROM transactions t
                     JOIN accounts inner_a ON inner_a.id = t.account_id
                     WHERE inner_a.bank = a.bank{flow_clause}
                 ), 0)
             FROM accounts a
             WHERE a.archived = 0
             GROUP BY a.bank
             ORDER BY a.bank COLLATE NOCASE"
        );

        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok(BankSummary {
                bank: row.get(0)?,
                accounts: row.get(1)?,
                balance: Money::from_minor_units(row.get(2)?),
                income: Money::from_minor_units(row.get(3)?),
                expense: Money::from_minor_units(row.get(4)?),
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    /// Contrapartes con más dinero acumulado en el periodo (gastos recurrentes,
    /// suscripciones olvidadas, el supermercado de siempre).
    pub fn top_counterparties(
        &self,
        filter: &TransactionFilter,
        limit: u32,
    ) -> StorageResult<Vec<CounterpartyTotal>> {
        let (where_clause, mut values) = build_where(filter);
        values.push(Value::from(i64::from(limit)));
        let limit_placeholder = values.len();

        let sql = format!(
            "SELECT
                 COALESCE(NULLIF(TRIM(t.counterparty), ''), t.description),
                 COALESCE(SUM(ABS(t.amount)), 0),
                 COUNT(*)
             FROM transactions t{where_clause}
             GROUP BY 1 COLLATE NOCASE
             ORDER BY 2 DESC
             LIMIT ?{limit_placeholder}"
        );

        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok(CounterpartyTotal {
                label: row.get(0)?,
                total: Money::from_minor_units(row.get(1)?),
                transactions: row.get(2)?,
            })
        })?;

        let mut totals = Vec::new();
        for row in rows {
            totals.push(row?);
        }
        Ok(totals)
    }
}

/// Tasa de ahorro en puntos básicos. Sin ingresos no hay tasa que calcular.
fn savings_rate_bps(income: i64, expense: i64) -> i64 {
    if income <= 0 {
        return 0;
    }
    (income - expense) * 10_000 / income
}

/// Reescribe `?1`, `?2`... sumando un desplazamiento, para poder reutilizar el
/// mismo `WHERE` en dos subconsultas con parámetros distintos.
fn shift_placeholders(clause: &str, offset: usize) -> String {
    if offset == 0 {
        return clause.to_string();
    }

    let mut result = String::with_capacity(clause.len());
    let mut chars = clause.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '?' {
            result.push(ch);
            continue;
        }

        let mut digits = String::new();
        while let Some(next) = chars.peek() {
            if next.is_ascii_digit() {
                digits.push(*next);
                chars.next();
            } else {
                break;
            }
        }

        match digits.parse::<usize>() {
            Ok(index) => result.push_str(&format!("?{}", index + offset)),
            Err(_) => {
                result.push('?');
                result.push_str(&digits);
            }
        }
    }

    result
}
