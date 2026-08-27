use serde::{Deserialize, Serialize};

use super::CategoryId;

/// Lado del flujo al que pertenece una categoría. El usuario organiza su dinero
/// en dos listas por banco (ingresos y gastos), así que la categoría lleva su
/// lado explícito en vez de deducirlo del signo de cada movimiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryKind {
    Income,
    Expense,
    /// Traspaso entre cuentas propias: no es ingreso ni gasto y se excluye de
    /// los totales para no contar el mismo dinero dos veces.
    Transfer,
}

impl CategoryKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            CategoryKind::Income => "income",
            CategoryKind::Expense => "expense",
            CategoryKind::Transfer => "transfer",
        }
    }

    pub fn from_str_opt(raw: &str) -> Option<Self> {
        match raw {
            "income" => Some(CategoryKind::Income),
            "expense" => Some(CategoryKind::Expense),
            "transfer" => Some(CategoryKind::Transfer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub kind: CategoryKind,
    /// Color hex (`"#e0b0ff"`) usado por los widgets del dashboard.
    pub color: String,
    pub is_system: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewCategory {
    pub name: String,
    pub kind: CategoryKind,
    pub color: String,
}
