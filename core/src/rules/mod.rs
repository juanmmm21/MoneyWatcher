//! Categorización determinista: reglas del usuario aplicadas en orden de
//! prioridad, más el aprendizaje de nuevas reglas a partir de sus correcciones.

mod engine;
mod learning;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::{
    CategoryId, NewRule, Rule, RuleId, RuleMatcher, RuleOrigin, Transaction, TransactionId,
};
use crate::storage::{Database, StorageResult, TransactionFilter};

pub use engine::{RuleEngine, RuleInput, RuleMatch};
pub use learning::suggest_pattern;

/// Prioridad por defecto de una regla aprendida: por debajo de las que el
/// usuario escribe a mano, para que una regla explícita siempre gane.
pub const LEARNED_RULE_PRIORITY: i64 = 50;
pub const USER_RULE_PRIORITY: i64 = 100;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorizationSummary {
    /// Movimientos que han recibido categoría en esta pasada.
    pub categorized: usize,
    /// Movimientos que siguen sin categoría y esperan revisión.
    pub pending: usize,
}

/// Aplica las reglas a todos los movimientos sin categoría.
///
/// Se ejecuta después de cada importación y también a mano desde la interfaz,
/// para que una regla nueva ordene de golpe el historial ya guardado.
pub fn apply_rules(database: &mut Database) -> StorageResult<CategorizationSummary> {
    let engine = RuleEngine::new(database.rules()?);
    if engine.is_empty() {
        let pending = database.count_transactions(&TransactionFilter {
            uncategorized_only: true,
            ..Default::default()
        })?;
        return Ok(CategorizationSummary {
            categorized: 0,
            pending: pending as usize,
        });
    }

    let pending_transactions = database.transactions(&TransactionFilter {
        uncategorized_only: true,
        ..Default::default()
    })?;

    let mut by_category: HashMap<CategoryId, Vec<TransactionId>> = HashMap::new();
    let mut hits: HashMap<RuleId, i64> = HashMap::new();

    for transaction in &pending_transactions {
        if let Some(matched) = engine.categorize(transaction) {
            by_category
                .entry(matched.category_id)
                .or_default()
                .push(transaction.id);
            *hits.entry(matched.rule_id).or_default() += 1;
        }
    }

    let mut categorized = 0;
    for (category_id, ids) in &by_category {
        categorized += database.categorize_many(ids, Some(*category_id))?;
    }

    let hit_counts: Vec<(RuleId, i64)> = hits.into_iter().collect();
    database.record_rule_hits(&hit_counts)?;

    Ok(CategorizationSummary {
        categorized,
        pending: pending_transactions.len() - categorized,
    })
}

/// Crea (si aporta algo) la regla que se deduce de una corrección manual.
///
/// Devuelve `None` cuando no hay un patrón fiable que extraer o cuando ya
/// existe una regla equivalente, para no llenar la lista de duplicados.
pub fn learn_from_correction(
    database: &Database,
    transaction: &Transaction,
    category_id: CategoryId,
) -> StorageResult<Option<Rule>> {
    let Some(pattern) = suggest_pattern(
        &transaction.description,
        transaction.counterparty.as_deref(),
    ) else {
        return Ok(None);
    };

    if database
        .find_equivalent_rule(RuleMatcher::Contains, &pattern, None)?
        .is_some()
    {
        return Ok(None);
    }

    let rule = database.create_rule(&NewRule {
        matcher: RuleMatcher::Contains,
        pattern,
        account_id: None,
        direction: None,
        min_amount: None,
        max_amount: None,
        category_id,
        priority: LEARNED_RULE_PRIORITY,
        origin: RuleOrigin::Learned,
    })?;

    Ok(Some(rule))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountKind, Money, NewAccount, NewTransaction, TransactionSource};
    use chrono::NaiveDate;

    fn database_with_movements() -> (Database, Vec<Transaction>) {
        let mut database = Database::open_in_memory().unwrap();
        let account = database
            .create_account(&NewAccount {
                name: "Main".into(),
                bank: "Santander".into(),
                kind: AccountKind::Checking,
                opening_balance: Money::ZERO,
            })
            .unwrap();

        let movements = [
            ("COMPRA TARJ. *1234 MERCADONA VALENCIA", -4_512),
            ("COMPRA TARJ. *1234 MERCADONA CENTRO", -2_010),
            ("PEAJE AP-7", -1_240),
        ];

        let batch: Vec<NewTransaction> = movements
            .iter()
            .enumerate()
            .map(|(index, (description, minor))| NewTransaction {
                account_id: account.id,
                booked_on: NaiveDate::from_ymd_opt(2026, 3, index as u32 + 1).unwrap(),
                value_on: None,
                description: (*description).into(),
                counterparty: None,
                amount: Money::from_minor_units(*minor),
                balance_after: None,
                category_id: None,
                notes: None,
                source: TransactionSource::Imported,
                import_id: None,
            })
            .collect();

        database.insert_transactions(&batch).unwrap();
        let stored = database
            .transactions(&TransactionFilter::default())
            .unwrap();
        (database, stored)
    }

    #[test]
    fn learns_a_rule_from_a_manual_correction_and_applies_it_to_the_rest() {
        let (mut database, transactions) = database_with_movements();
        let groceries = database.category_by_name("Supermercado").unwrap().unwrap();
        let corrected = transactions
            .iter()
            .find(|t| t.description.contains("MERCADONA VALENCIA"))
            .unwrap();

        database
            .set_transaction_category(corrected.id, Some(groceries.id))
            .unwrap();
        let learned = learn_from_correction(&database, corrected, groceries.id)
            .unwrap()
            .expect("se aprende una regla");
        assert_eq!(learned.pattern, "mercadona");
        assert_eq!(learned.origin, RuleOrigin::Learned);

        let summary = apply_rules(&mut database).unwrap();
        assert_eq!(
            summary.categorized, 1,
            "el otro Mercadona queda categorizado"
        );
        assert_eq!(summary.pending, 1, "el peaje sigue esperando revisión");
        assert_eq!(database.rule(learned.id).unwrap().hits, 1);
    }

    #[test]
    fn does_not_duplicate_an_existing_rule() {
        let (database, transactions) = database_with_movements();
        let groceries = database.category_by_name("Supermercado").unwrap().unwrap();
        let corrected = &transactions[0];

        assert!(learn_from_correction(&database, corrected, groceries.id)
            .unwrap()
            .is_some());
        assert!(learn_from_correction(&database, corrected, groceries.id)
            .unwrap()
            .is_none());
        assert_eq!(database.rules().unwrap().len(), 1);
    }

    #[test]
    fn without_rules_everything_stays_pending() {
        let (mut database, _) = database_with_movements();
        let summary = apply_rules(&mut database).unwrap();
        assert_eq!(
            summary,
            CategorizationSummary {
                categorized: 0,
                pending: 3
            }
        );
    }
}
