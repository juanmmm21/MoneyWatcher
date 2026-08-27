use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{AccountId, CategoryId, ImportId, Money, TransactionId};

/// Origen del movimiento. Se conserva para poder deshacer una importación
/// completa sin tocar lo que el usuario introdujo a mano.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionSource {
    Imported,
    Manual,
}

impl TransactionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            TransactionSource::Imported => "imported",
            TransactionSource::Manual => "manual",
        }
    }

    pub fn from_str_opt(raw: &str) -> Option<Self> {
        match raw {
            "imported" => Some(TransactionSource::Imported),
            "manual" => Some(TransactionSource::Manual),
            _ => None,
        }
    }
}

/// Sentido del movimiento, derivado del signo del importe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Income,
    Expense,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub account_id: AccountId,
    /// Fecha contable (la que usa el banco para ordenar el extracto).
    pub booked_on: NaiveDate,
    /// Fecha valor, cuando el extracto la trae por separado.
    pub value_on: Option<NaiveDate>,
    pub description: String,
    pub counterparty: Option<String>,
    /// Positivo para ingresos, negativo para gastos.
    pub amount: Money,
    pub balance_after: Option<Money>,
    pub category_id: Option<CategoryId>,
    pub notes: Option<String>,
    pub source: TransactionSource,
    pub import_id: Option<ImportId>,
    /// Huella estable del movimiento, usada para no duplicar al reimportar.
    pub fingerprint: String,
}

impl Transaction {
    pub fn direction(&self) -> Direction {
        if self.amount.is_negative() {
            Direction::Expense
        } else {
            Direction::Income
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTransaction {
    pub account_id: AccountId,
    pub booked_on: NaiveDate,
    pub value_on: Option<NaiveDate>,
    pub description: String,
    pub counterparty: Option<String>,
    pub amount: Money,
    pub balance_after: Option<Money>,
    pub category_id: Option<CategoryId>,
    pub notes: Option<String>,
    pub source: TransactionSource,
    pub import_id: Option<ImportId>,
}

impl NewTransaction {
    pub fn fingerprint(&self) -> String {
        fingerprint(
            self.account_id,
            self.booked_on,
            self.amount,
            &self.description,
        )
    }
}

/// Huella de deduplicación: mismo banco, día, importe y concepto normalizado.
///
/// Se usa SHA-256 en lugar del hasher por defecto de la librería estándar
/// porque el valor se persiste en SQLite y debe seguir siendo el mismo entre
/// versiones de Rust y entre máquinas.
pub fn fingerprint(
    account_id: AccountId,
    booked_on: NaiveDate,
    amount: Money,
    description: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(account_id.value().to_le_bytes());
    hasher.update(booked_on.to_string().as_bytes());
    hasher.update(amount.minor_units().to_le_bytes());
    hasher.update(normalize_description(description).as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Normaliza el concepto para comparar: los bancos cambian espacios, mayúsculas
/// y puntuación entre exportaciones del mismo movimiento.
pub fn normalize_description(description: &str) -> String {
    let mut normalized = String::with_capacity(description.len());
    let mut last_was_space = true;

    for ch in description.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                normalized.push(lower);
            }
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("fecha de test válida")
    }

    #[test]
    fn normalizes_description_noise() {
        assert_eq!(
            normalize_description("  COMPRA   TARJ. *1234  MERCADONA/VALENCIA "),
            "compra tarj 1234 mercadona valencia"
        );
    }

    #[test]
    fn fingerprint_is_stable_across_formatting_noise() {
        let a = fingerprint(
            AccountId(1),
            date(2026, 3, 14),
            Money::from_minor_units(-4_512),
            "PAGO  TARJETA   *1234 MERCADONA",
        );
        let b = fingerprint(
            AccountId(1),
            date(2026, 3, 14),
            Money::from_minor_units(-4_512),
            "Pago tarjeta *1234, Mercadona",
        );
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_separates_different_accounts_and_amounts() {
        let base = fingerprint(
            AccountId(1),
            date(2026, 3, 14),
            Money::from_minor_units(-4_512),
            "MERCADONA",
        );
        let other_account = fingerprint(
            AccountId(2),
            date(2026, 3, 14),
            Money::from_minor_units(-4_512),
            "MERCADONA",
        );
        let other_amount = fingerprint(
            AccountId(1),
            date(2026, 3, 14),
            Money::from_minor_units(-4_513),
            "MERCADONA",
        );
        assert_ne!(base, other_account);
        assert_ne!(base, other_amount);
    }

    #[test]
    fn direction_follows_amount_sign() {
        let tx = Transaction {
            id: TransactionId(1),
            account_id: AccountId(1),
            booked_on: date(2026, 3, 14),
            value_on: None,
            description: "Nómina".into(),
            counterparty: None,
            amount: Money::from_minor_units(180_000),
            balance_after: None,
            category_id: None,
            notes: None,
            source: TransactionSource::Manual,
            import_id: None,
            fingerprint: String::new(),
        };
        assert_eq!(tx.direction(), Direction::Income);

        let expense = Transaction {
            amount: Money::from_minor_units(-1),
            ..tx
        };
        assert_eq!(expense.direction(), Direction::Expense);
    }
}
