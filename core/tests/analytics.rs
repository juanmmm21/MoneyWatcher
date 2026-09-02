//! Tests de las agregaciones que alimentan los widgets del dashboard.

use chrono::NaiveDate;
use moneywatcher_core::domain::{
    AccountId, AccountKind, Direction, Money, NewAccount, NewTransaction, TransactionSource,
};
use moneywatcher_core::storage::{Database, TransactionFilter};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("fecha válida")
}

fn movement(
    account_id: AccountId,
    booked_on: NaiveDate,
    description: &str,
    counterparty: Option<&str>,
    minor: i64,
) -> NewTransaction {
    NewTransaction {
        account_id,
        booked_on,
        value_on: None,
        description: description.into(),
        counterparty: counterparty.map(str::to_string),
        amount: Money::from_minor_units(minor),
        balance_after: None,
        category_id: None,
        notes: None,
        source: TransactionSource::Imported,
        import_id: None,
    }
}

/// Dos bancos, tres meses de movimientos y una categoría asignada, que es el
/// escenario mínimo con el que tiene sentido mirar el dashboard.
fn seeded_database() -> (Database, AccountId, AccountId) {
    let mut database = Database::open_in_memory().expect("base de datos en memoria");

    let santander = database
        .create_account(&NewAccount {
            name: "Nómina".into(),
            bank: "Santander".into(),
            kind: AccountKind::Checking,
            currency: "EUR".into(),
            opening_balance: Money::from_minor_units(100_000),
        })
        .unwrap();

    let bbva = database
        .create_account(&NewAccount {
            name: "Ahorro".into(),
            bank: "BBVA".into(),
            kind: AccountKind::Savings,
            currency: "EUR".into(),
            opening_balance: Money::from_minor_units(500_000),
        })
        .unwrap();

    database
        .insert_transactions(&[
            movement(
                santander.id,
                date(2026, 1, 31),
                "NOMINA ENERO",
                Some("ACME SL"),
                185_000,
            ),
            movement(
                santander.id,
                date(2026, 1, 5),
                "MERCADONA",
                Some("MERCADONA"),
                -4_512,
            ),
            movement(
                santander.id,
                date(2026, 2, 28),
                "NOMINA FEBRERO",
                Some("ACME SL"),
                185_000,
            ),
            movement(
                santander.id,
                date(2026, 2, 10),
                "MERCADONA",
                Some("MERCADONA"),
                -6_020,
            ),
            movement(
                santander.id,
                date(2026, 2, 12),
                "IBERDROLA",
                Some("IBERDROLA"),
                -7_290,
            ),
            movement(
                santander.id,
                date(2026, 3, 31),
                "NOMINA MARZO",
                Some("ACME SL"),
                185_000,
            ),
            movement(
                santander.id,
                date(2026, 3, 15),
                "MERCADONA",
                Some("MERCADONA"),
                -5_100,
            ),
            movement(bbva.id, date(2026, 3, 1), "TRASPASO AHORRO", None, 50_000),
        ])
        .unwrap();

    (database, santander.id, bbva.id)
}

#[test]
fn totals_split_income_from_expense_and_compute_savings_rate() {
    let (database, _, _) = seeded_database();
    let totals = database.flow_totals(&TransactionFilter::default()).unwrap();

    assert_eq!(totals.income, Money::from_minor_units(605_000));
    assert_eq!(totals.expense, Money::from_minor_units(22_922));
    assert_eq!(totals.net, Money::from_minor_units(582_078));
    // (605000 - 22922) / 605000 = 96,21 %
    assert_eq!(totals.savings_rate_bps, 9_621);
}

#[test]
fn monthly_flow_returns_one_row_per_month_in_chronological_order() {
    let (database, santander, _) = seeded_database();
    let months = database
        .monthly_flow(&TransactionFilter {
            account_ids: vec![santander],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(months.len(), 3);
    assert_eq!(months[0].month, "2026-01");
    assert_eq!(months[0].income, Money::from_minor_units(185_000));
    assert_eq!(months[0].expense, Money::from_minor_units(4_512));
    assert_eq!(months[1].month, "2026-02");
    assert_eq!(months[1].expense, Money::from_minor_units(13_310));
    assert_eq!(months[2].net, Money::from_minor_units(179_900));
}

#[test]
fn category_breakdown_groups_uncategorized_movements_instead_of_hiding_them() {
    let (mut database, santander, _) = seeded_database();
    let groceries = database.category_by_name("Supermercado").unwrap().unwrap();

    let grocery_ids: Vec<_> = database
        .transactions(&TransactionFilter {
            search: Some("MERCADONA".into()),
            ..Default::default()
        })
        .unwrap()
        .iter()
        .map(|transaction| transaction.id)
        .collect();
    database
        .categorize_many(&grocery_ids, Some(groceries.id))
        .unwrap();

    let breakdown = database
        .category_breakdown(&TransactionFilter {
            account_ids: vec![santander],
            direction: Some(Direction::Expense),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(breakdown.len(), 2);
    assert_eq!(breakdown[0].name, "Supermercado");
    assert_eq!(breakdown[0].total, Money::from_minor_units(15_632));
    assert_eq!(breakdown[0].transactions, 3);
    assert_eq!(breakdown[1].name, "Uncategorized");
    assert_eq!(
        breakdown[0].share_bps + breakdown[1].share_bps,
        9_999,
        "los porcentajes reparten el total salvo el redondeo entero"
    );
}

#[test]
fn bank_summaries_include_opening_balance_and_period_flow() {
    let (database, _, _) = seeded_database();
    let summaries = database
        .bank_summaries(&TransactionFilter {
            from: Some(date(2026, 3, 1)),
            to: Some(date(2026, 3, 31)),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(summaries.len(), 2);

    let bbva = &summaries[0];
    assert_eq!(bbva.bank, "BBVA");
    assert_eq!(bbva.currency, "EUR");
    assert_eq!(bbva.balance, Money::from_minor_units(550_000));
    assert_eq!(bbva.income, Money::from_minor_units(50_000));

    let santander = &summaries[1];
    assert_eq!(santander.bank, "Santander");
    // 1.000,00 de apertura + 5.820,78 de movimientos históricos.
    assert_eq!(santander.balance, Money::from_minor_units(632_078));
    // El flujo, en cambio, solo cuenta marzo.
    assert_eq!(santander.income, Money::from_minor_units(185_000));
    assert_eq!(santander.expense, Money::from_minor_units(5_100));
}

/// El caso que hoy daba números falsos: una entidad con una cuenta en euros y
/// otra en libras. Ni los totales ni la tabla de bancos pueden mezclarlas.
fn multi_currency_database() -> (Database, AccountId, AccountId) {
    let mut database = Database::open_in_memory().expect("base de datos en memoria");

    let euros = database
        .create_account(&NewAccount {
            name: "Euros".into(),
            bank: "Revolut".into(),
            kind: AccountKind::Checking,
            currency: "EUR".into(),
            opening_balance: Money::from_minor_units(20_000),
        })
        .unwrap();

    let pounds = database
        .create_account(&NewAccount {
            name: "Libras".into(),
            bank: "Revolut".into(),
            kind: AccountKind::Checking,
            currency: "GBP".into(),
            opening_balance: Money::from_minor_units(10_000),
        })
        .unwrap();

    database
        .insert_transactions(&[
            movement(euros.id, date(2026, 3, 2), "MERCADONA", None, -3_000),
            movement(euros.id, date(2026, 3, 3), "NOMINA", None, 100_000),
            movement(pounds.id, date(2026, 3, 4), "TESCO", None, -1_500),
            movement(pounds.id, date(2026, 3, 5), "SALARY", None, 50_000),
        ])
        .unwrap();

    (database, euros.id, pounds.id)
}

#[test]
fn totals_only_add_up_accounts_of_the_requested_currency() {
    let (database, _, _) = multi_currency_database();

    let pounds = database
        .flow_totals(&TransactionFilter {
            currency: Some("GBP".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(pounds.income, Money::from_minor_units(50_000));
    assert_eq!(pounds.expense, Money::from_minor_units(1_500));

    let euros = database
        .flow_totals(&TransactionFilter {
            currency: Some("EUR".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(euros.income, Money::from_minor_units(100_000));
    assert_eq!(euros.expense, Money::from_minor_units(3_000));
}

#[test]
fn bank_summaries_split_one_bank_into_one_row_per_currency() {
    let (database, _, _) = multi_currency_database();

    let all = database
        .bank_summaries(&TransactionFilter::default())
        .unwrap();
    assert_eq!(all.len(), 2, "una misma entidad da una fila por divisa");
    assert_eq!(all[0].currency, "EUR");
    assert_eq!(all[0].balance, Money::from_minor_units(117_000));
    assert_eq!(all[1].currency, "GBP");
    assert_eq!(all[1].balance, Money::from_minor_units(58_500));

    let only_pounds = database
        .bank_summaries(&TransactionFilter {
            currency: Some("GBP".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(only_pounds.len(), 1);
    assert_eq!(only_pounds[0].currency, "GBP");
    assert_eq!(only_pounds[0].income, Money::from_minor_units(50_000));
    assert_eq!(only_pounds[0].expense, Money::from_minor_units(1_500));
}

#[test]
fn monthly_flow_and_categories_stay_within_one_currency() {
    let (database, _, _) = multi_currency_database();

    let months = database
        .monthly_flow(&TransactionFilter {
            currency: Some("GBP".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(months.len(), 1);
    assert_eq!(months[0].net, Money::from_minor_units(48_500));

    let breakdown = database
        .category_breakdown(&TransactionFilter {
            currency: Some("GBP".into()),
            direction: Some(Direction::Expense),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(breakdown.len(), 1);
    assert_eq!(breakdown[0].total, Money::from_minor_units(1_500));

    let top = database
        .top_counterparties(
            &TransactionFilter {
                currency: Some("GBP".into()),
                direction: Some(Direction::Expense),
                ..Default::default()
            },
            5,
        )
        .unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].label, "TESCO");
}

#[test]
fn top_counterparties_ranks_recurring_spending() {
    let (database, _, _) = seeded_database();
    let top = database
        .top_counterparties(
            &TransactionFilter {
                direction: Some(Direction::Expense),
                ..Default::default()
            },
            2,
        )
        .unwrap();

    assert_eq!(top.len(), 2);
    assert_eq!(top[0].label, "MERCADONA");
    assert_eq!(top[0].total, Money::from_minor_units(15_632));
    assert_eq!(top[0].transactions, 3);
    assert_eq!(top[1].label, "IBERDROLA");
}

#[test]
fn empty_database_returns_neutral_aggregates() {
    let database = Database::open_in_memory().unwrap();
    let totals = database.flow_totals(&TransactionFilter::default()).unwrap();

    assert_eq!(totals.income, Money::ZERO);
    assert_eq!(totals.savings_rate_bps, 0);
    assert!(database
        .monthly_flow(&TransactionFilter::default())
        .unwrap()
        .is_empty());
    assert!(database
        .bank_summaries(&TransactionFilter::default())
        .unwrap()
        .is_empty());
}
