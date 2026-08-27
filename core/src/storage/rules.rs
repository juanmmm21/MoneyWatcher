use rusqlite::{params, Row};

use crate::domain::{
    AccountId, CategoryId, Direction, Money, NewRule, Rule, RuleId, RuleMatcher, RuleOrigin,
};

use super::{Database, StorageError, StorageResult};

impl Database {
    pub fn create_rule(&self, rule: &NewRule) -> StorageResult<Rule> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO rules
                 (matcher, pattern, account_id, direction, min_amount, max_amount,
                  category_id, priority, origin, hits, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)",
            params![
                rule.matcher.as_str(),
                rule.pattern.trim(),
                rule.account_id.map(AccountId::value),
                rule.direction.map(direction_as_str),
                rule.min_amount.map(Money::minor_units),
                rule.max_amount.map(Money::minor_units),
                rule.category_id.value(),
                rule.priority,
                rule.origin.as_str(),
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        self.rule(RuleId(conn.last_insert_rowid()))
    }

    pub fn rule(&self, id: RuleId) -> StorageResult<Rule> {
        self.connection()
            .query_row(&format!("{SELECT_RULE} WHERE id = ?1"), params![id.value()], map_rule)
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                    entity: "rule",
                    id: id.value(),
                },
                other => other.into(),
            })?
    }

    /// Reglas en orden de evaluación: prioridad descendente y, a igualdad, la
    /// más antigua primero para que el resultado sea reproducible.
    pub fn rules(&self) -> StorageResult<Vec<Rule>> {
        let mut statement = self
            .connection()
            .prepare(&format!("{SELECT_RULE} ORDER BY priority DESC, id ASC"))?;
        let rows = statement.query_map([], map_rule)?;
        let mut rules = Vec::new();
        for row in rows {
            rules.push(row??);
        }
        Ok(rules)
    }

    pub fn delete_rule(&self, id: RuleId) -> StorageResult<()> {
        let deleted = self
            .connection()
            .execute("DELETE FROM rules WHERE id = ?1", params![id.value()])?;
        if deleted == 0 {
            return Err(StorageError::NotFound {
                entity: "rule",
                id: id.value(),
            });
        }
        Ok(())
    }

    /// Suma los aciertos acumulados de cada regla tras aplicar el motor.
    pub fn record_rule_hits(&mut self, hits: &[(RuleId, i64)]) -> StorageResult<()> {
        if hits.is_empty() {
            return Ok(());
        }

        let tx = self.connection_mut().transaction()?;
        {
            let mut statement = tx.prepare("UPDATE rules SET hits = hits + ?2 WHERE id = ?1")?;
            for (id, count) in hits {
                statement.execute(params![id.value(), count])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// ¿Existe ya una regla equivalente? Evita acumular duplicados cada vez que
    /// el usuario corrige un movimiento parecido.
    pub fn find_equivalent_rule(
        &self,
        matcher: RuleMatcher,
        pattern: &str,
        account_id: Option<AccountId>,
    ) -> StorageResult<Option<Rule>> {
        let mut statement = self.connection().prepare(&format!(
            "{SELECT_RULE} WHERE matcher = ?1 AND pattern = ?2 COLLATE NOCASE
             AND IFNULL(account_id, -1) = IFNULL(?3, -1) LIMIT 1"
        ))?;
        let mut rows = statement.query_map(
            params![matcher.as_str(), pattern.trim(), account_id.map(AccountId::value)],
            map_rule,
        )?;
        match rows.next() {
            Some(row) => Ok(Some(row??)),
            None => Ok(None),
        }
    }
}

const SELECT_RULE: &str = "SELECT id, matcher, pattern, account_id, direction, min_amount,
        max_amount, category_id, priority, origin, hits
 FROM rules";

fn direction_as_str(direction: Direction) -> &'static str {
    match direction {
        Direction::Income => "income",
        Direction::Expense => "expense",
    }
}

fn map_rule(row: &Row<'_>) -> rusqlite::Result<StorageResult<Rule>> {
    let raw_matcher: String = row.get(1)?;
    let matcher = match RuleMatcher::from_str_opt(&raw_matcher) {
        Some(matcher) => matcher,
        None => {
            return Ok(Err(StorageError::CorruptValue {
                field: "rule matcher",
                value: raw_matcher,
            }))
        }
    };

    let direction = match row.get::<_, Option<String>>(4)? {
        Some(raw) => match raw.as_str() {
            "income" => Some(Direction::Income),
            "expense" => Some(Direction::Expense),
            other => {
                return Ok(Err(StorageError::CorruptValue {
                    field: "rule direction",
                    value: other.to_string(),
                }))
            }
        },
        None => None,
    };

    let raw_origin: String = row.get(9)?;
    let origin = match RuleOrigin::from_str_opt(&raw_origin) {
        Some(origin) => origin,
        None => {
            return Ok(Err(StorageError::CorruptValue {
                field: "rule origin",
                value: raw_origin,
            }))
        }
    };

    Ok(Ok(Rule {
        id: RuleId(row.get(0)?),
        matcher,
        pattern: row.get(2)?,
        account_id: row.get::<_, Option<i64>>(3)?.map(AccountId),
        direction,
        min_amount: row.get::<_, Option<i64>>(5)?.map(Money::from_minor_units),
        max_amount: row.get::<_, Option<i64>>(6)?.map(Money::from_minor_units),
        category_id: CategoryId(row.get(7)?),
        priority: row.get(8)?,
        origin,
        hits: row.get(10)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    fn new_rule(db: &Database, pattern: &str, priority: i64) -> NewRule {
        let category = db.category_by_name("Groceries").unwrap().unwrap();
        NewRule {
            matcher: RuleMatcher::Contains,
            pattern: pattern.into(),
            account_id: None,
            direction: None,
            min_amount: None,
            max_amount: None,
            category_id: category.id,
            priority,
            origin: RuleOrigin::User,
        }
    }

    #[test]
    fn orders_rules_by_priority_then_age() {
        let db = Database::open_in_memory().unwrap();
        let low = db.create_rule(&new_rule(&db, "mercadona", 10)).unwrap();
        let high = db.create_rule(&new_rule(&db, "carrefour", 90)).unwrap();
        let same_priority = db.create_rule(&new_rule(&db, "lidl", 90)).unwrap();

        let ordered = db.rules().unwrap();
        assert_eq!(
            ordered.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![high.id, same_priority.id, low.id]
        );
    }

    #[test]
    fn detects_equivalent_rules() {
        let db = Database::open_in_memory().unwrap();
        db.create_rule(&new_rule(&db, "MERCADONA", 50)).unwrap();

        let found = db
            .find_equivalent_rule(RuleMatcher::Contains, "mercadona", None)
            .unwrap();
        assert!(found.is_some(), "la comparación de patrones ignora mayúsculas");

        let missing = db
            .find_equivalent_rule(RuleMatcher::Equals, "mercadona", None)
            .unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn accumulates_hits() {
        let mut db = Database::open_in_memory().unwrap();
        let rule = db.create_rule(&new_rule(&db, "mercadona", 50)).unwrap();
        db.record_rule_hits(&[(rule.id, 3)]).unwrap();
        db.record_rule_hits(&[(rule.id, 2)]).unwrap();
        assert_eq!(db.rule(rule.id).unwrap().hits, 5);
    }
}
