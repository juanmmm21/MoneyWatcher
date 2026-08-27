use serde::{Deserialize, Serialize};

use super::{AccountId, Money};

/// Naturaleza de la cuenta. Determina cómo se interpreta su balance: en una
/// tarjeta de crédito un saldo negativo es deuda pendiente, no un descubierto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Checking,
    Savings,
    Credit,
    Cash,
    Investment,
}

impl AccountKind {
    pub const ALL: [AccountKind; 5] = [
        AccountKind::Checking,
        AccountKind::Savings,
        AccountKind::Credit,
        AccountKind::Cash,
        AccountKind::Investment,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AccountKind::Checking => "checking",
            AccountKind::Savings => "savings",
            AccountKind::Credit => "credit",
            AccountKind::Cash => "cash",
            AccountKind::Investment => "investment",
        }
    }

    pub fn from_str_opt(raw: &str) -> Option<Self> {
        AccountKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == raw)
    }
}

/// Una cuenta bancaria del usuario. `bank` se guarda aparte del nombre porque
/// la organización que pide el producto es "por banco": todas las vistas
/// agrupan ingresos y gastos por entidad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub bank: String,
    pub kind: AccountKind,
    /// Código ISO 4217 en mayúsculas (`"EUR"`).
    pub currency: String,
    pub opening_balance: Money,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAccount {
    pub name: String,
    pub bank: String,
    pub kind: AccountKind,
    pub currency: String,
    pub opening_balance: Money,
}
