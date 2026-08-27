use crate::domain::{
    normalize_description, AccountId, CategoryId, Direction, Money, NewTransaction, Rule, RuleId,
    RuleMatcher, Transaction,
};

/// Datos mínimos que necesita el motor para decidir. Se usa el mismo tipo para
/// movimientos ya guardados y para filas recién importadas.
#[derive(Debug, Clone, Copy)]
pub struct RuleInput<'a> {
    pub account_id: AccountId,
    pub description: &'a str,
    pub counterparty: Option<&'a str>,
    pub amount: Money,
}

impl<'a> From<&'a Transaction> for RuleInput<'a> {
    fn from(transaction: &'a Transaction) -> Self {
        RuleInput {
            account_id: transaction.account_id,
            description: &transaction.description,
            counterparty: transaction.counterparty.as_deref(),
            amount: transaction.amount,
        }
    }
}

impl<'a> From<&'a NewTransaction> for RuleInput<'a> {
    fn from(transaction: &'a NewTransaction) -> Self {
        RuleInput {
            account_id: transaction.account_id,
            description: &transaction.description,
            counterparty: transaction.counterparty.as_deref(),
            amount: transaction.amount,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMatch {
    pub rule_id: RuleId,
    pub category_id: CategoryId,
}

/// Evalúa las reglas del usuario en orden de prioridad y devuelve la primera
/// que encaja. Es la vía por defecto de categorización: rápida, offline y
/// completamente predecible, sin depender de ningún modelo.
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    /// Espera las reglas ya ordenadas por prioridad (como las devuelve el
    /// almacenamiento); las reordena igualmente para no depender de eso.
    pub fn new(mut rules: Vec<Rule>) -> Self {
        rules.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.value().cmp(&b.id.value())));
        RuleEngine { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn categorize<'a>(&self, input: impl Into<RuleInput<'a>>) -> Option<RuleMatch> {
        let input = input.into();
        let haystack = searchable_text(&input);

        self.rules
            .iter()
            .find(|rule| matches(rule, &input, &haystack))
            .map(|rule| RuleMatch {
                rule_id: rule.id,
                category_id: rule.category_id,
            })
    }
}

/// Texto sobre el que se buscan los patrones: concepto y contraparte juntos y
/// normalizados, porque el nombre del comercio a veces solo está en uno de los dos.
fn searchable_text(input: &RuleInput<'_>) -> String {
    match input.counterparty {
        Some(counterparty) if !counterparty.trim().is_empty() => {
            normalize_description(&format!("{} {}", input.description, counterparty))
        }
        _ => normalize_description(input.description),
    }
}

fn matches(rule: &Rule, input: &RuleInput<'_>, haystack: &str) -> bool {
    if let Some(account_id) = rule.account_id {
        if account_id != input.account_id {
            return false;
        }
    }

    if let Some(direction) = rule.direction {
        let actual = if input.amount.is_negative() {
            Direction::Expense
        } else {
            Direction::Income
        };
        if direction != actual {
            return false;
        }
    }

    // Los límites se comparan sobre el importe absoluto: "gastos de entre 10 y
    // 50 €" es más natural de escribir que "entre -50 y -10".
    let magnitude = input.amount.abs();
    if let Some(min) = rule.min_amount {
        if magnitude < min.abs() {
            return false;
        }
    }
    if let Some(max) = rule.max_amount {
        if magnitude > max.abs() {
            return false;
        }
    }

    let pattern = normalize_description(&rule.pattern);
    if pattern.is_empty() {
        return false;
    }

    match rule.matcher {
        RuleMatcher::Contains => haystack.contains(&pattern),
        RuleMatcher::StartsWith => haystack.starts_with(&pattern),
        RuleMatcher::EndsWith => haystack.ends_with(&pattern),
        RuleMatcher::Equals => haystack == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CategoryId, RuleOrigin};

    fn rule(id: i64, pattern: &str, category: i64, priority: i64) -> Rule {
        Rule {
            id: RuleId(id),
            matcher: RuleMatcher::Contains,
            pattern: pattern.into(),
            account_id: None,
            direction: None,
            min_amount: None,
            max_amount: None,
            category_id: CategoryId(category),
            priority,
            origin: RuleOrigin::User,
            hits: 0,
        }
    }

    fn input<'a>(description: &'a str, minor: i64) -> RuleInput<'a> {
        RuleInput {
            account_id: AccountId(1),
            description,
            counterparty: None,
            amount: Money::from_minor_units(minor),
        }
    }

    #[test]
    fn matches_ignoring_case_accents_and_punctuation() {
        let engine = RuleEngine::new(vec![rule(1, "MERCADONA", 5, 100)]);
        let matched = engine
            .categorize(input("Compra tarj. *1234 mercadona/valencia", -4_512))
            .expect("la regla encaja");
        assert_eq!(matched.category_id, CategoryId(5));
    }

    #[test]
    fn higher_priority_rule_wins() {
        let engine = RuleEngine::new(vec![
            rule(1, "compra", 5, 10),
            rule(2, "mercadona", 9, 90),
        ]);
        let matched = engine.categorize(input("COMPRA MERCADONA", -1_000)).unwrap();
        assert_eq!(matched.rule_id, RuleId(2));
    }

    #[test]
    fn respects_direction_and_amount_bounds() {
        let mut only_income = rule(1, "transferencia", 5, 100);
        only_income.direction = Some(Direction::Income);

        let mut small_expense = rule(2, "transferencia", 7, 90);
        small_expense.direction = Some(Direction::Expense);
        small_expense.max_amount = Some(Money::from_minor_units(5_000));

        let engine = RuleEngine::new(vec![only_income, small_expense]);

        assert_eq!(
            engine.categorize(input("TRANSFERENCIA RECIBIDA", 20_000)).unwrap().category_id,
            CategoryId(5)
        );
        assert_eq!(
            engine.categorize(input("TRANSFERENCIA ENVIADA", -3_000)).unwrap().category_id,
            CategoryId(7)
        );
        assert!(
            engine.categorize(input("TRANSFERENCIA ENVIADA", -90_000)).is_none(),
            "un gasto por encima del máximo no debe encajar"
        );
    }

    #[test]
    fn account_scoped_rules_do_not_leak_to_other_banks() {
        let mut scoped = rule(1, "nomina", 5, 100);
        scoped.account_id = Some(AccountId(2));
        let engine = RuleEngine::new(vec![scoped]);

        assert!(engine.categorize(input("NOMINA MARZO", 180_000)).is_none());
        assert!(engine
            .categorize(RuleInput {
                account_id: AccountId(2),
                description: "NOMINA MARZO",
                counterparty: None,
                amount: Money::from_minor_units(180_000),
            })
            .is_some());
    }

    #[test]
    fn searches_counterparty_too() {
        let engine = RuleEngine::new(vec![rule(1, "iberdrola", 5, 100)]);
        let matched = engine.categorize(RuleInput {
            account_id: AccountId(1),
            description: "RECIBO DOMICILIADO",
            counterparty: Some("IBERDROLA CLIENTES SAU"),
            amount: Money::from_minor_units(-7_290),
        });
        assert!(matched.is_some());
    }

    #[test]
    fn exact_and_prefix_matchers() {
        let mut equals = rule(1, "spotify", 5, 100);
        equals.matcher = RuleMatcher::Equals;
        let mut starts = rule(2, "pago", 7, 50);
        starts.matcher = RuleMatcher::StartsWith;

        let engine = RuleEngine::new(vec![equals, starts]);
        assert_eq!(engine.categorize(input("Spotify", -1_099)).unwrap().category_id, CategoryId(5));
        assert_eq!(
            engine.categorize(input("PAGO SPOTIFY AB", -1_099)).unwrap().category_id,
            CategoryId(7)
        );
    }

    #[test]
    fn returns_nothing_when_no_rule_applies() {
        let engine = RuleEngine::new(vec![rule(1, "mercadona", 5, 100)]);
        assert!(engine.categorize(input("PEAJE AP-7", -1_240)).is_none());
        assert!(RuleEngine::new(Vec::new()).is_empty());
    }
}
