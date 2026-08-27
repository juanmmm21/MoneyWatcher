use serde::{Deserialize, Serialize};

use super::{AccountId, CategoryId, Direction, Money, RuleId};

/// Forma en que una regla compara su patrón con el concepto del movimiento.
/// Deliberadamente no hay expresiones regulares: las reglas las escribe (o
/// confirma) el usuario desde la interfaz, y una regex mal formada es una
/// fuente de errores silenciosos en la categorización.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleMatcher {
    Contains,
    StartsWith,
    EndsWith,
    Equals,
}

impl RuleMatcher {
    pub const fn as_str(self) -> &'static str {
        match self {
            RuleMatcher::Contains => "contains",
            RuleMatcher::StartsWith => "starts_with",
            RuleMatcher::EndsWith => "ends_with",
            RuleMatcher::Equals => "equals",
        }
    }

    pub fn from_str_opt(raw: &str) -> Option<Self> {
        match raw {
            "contains" => Some(RuleMatcher::Contains),
            "starts_with" => Some(RuleMatcher::StartsWith),
            "ends_with" => Some(RuleMatcher::EndsWith),
            "equals" => Some(RuleMatcher::Equals),
            _ => None,
        }
    }
}

/// De dónde salió la regla. Sirve para que la interfaz distinga lo que el
/// usuario escribió de lo que la app dedujo, y para poder revisar lo segundo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    /// Creada a mano en la interfaz.
    User,
    /// Aprendida de una corrección del usuario sobre un movimiento concreto.
    Learned,
    /// Aceptada a partir de una sugerencia del asistente de IA.
    Assistant,
}

impl RuleOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            RuleOrigin::User => "user",
            RuleOrigin::Learned => "learned",
            RuleOrigin::Assistant => "assistant",
        }
    }

    pub fn from_str_opt(raw: &str) -> Option<Self> {
        match raw {
            "user" => Some(RuleOrigin::User),
            "learned" => Some(RuleOrigin::Learned),
            "assistant" => Some(RuleOrigin::Assistant),
            _ => None,
        }
    }
}

/// Regla de categorización automática. Todas las condiciones presentes deben
/// cumplirse a la vez; las ausentes no restringen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: RuleId,
    pub matcher: RuleMatcher,
    pub pattern: String,
    /// Limita la regla a una cuenta concreta (por ejemplo, la nómina que solo
    /// entra en un banco).
    pub account_id: Option<AccountId>,
    pub direction: Option<Direction>,
    pub min_amount: Option<Money>,
    pub max_amount: Option<Money>,
    pub category_id: CategoryId,
    /// A mayor prioridad, antes se evalúa. Empates: gana la regla más antigua.
    pub priority: i64,
    pub origin: RuleOrigin,
    /// Veces que la regla ha categorizado un movimiento, para poder ordenarlas
    /// por utilidad real y detectar reglas muertas.
    pub hits: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRule {
    pub matcher: RuleMatcher,
    pub pattern: String,
    pub account_id: Option<AccountId>,
    pub direction: Option<Direction>,
    pub min_amount: Option<Money>,
    pub max_amount: Option<Money>,
    pub category_id: CategoryId,
    pub priority: i64,
    pub origin: RuleOrigin,
}
