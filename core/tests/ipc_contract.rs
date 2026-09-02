//! El frontend consume estos tipos tal cual salen de los comandos Tauri, y su
//! contrato TypeScript (`src/types/ipc.ts`) está escrito en camelCase. Un
//! struct del dominio sin `rename_all = "camelCase"` rompe la IPC en silencio
//! al leer y con un error duro al escribir, así que aquí se fija la convención.

use chrono::NaiveDate;
use moneywatcher_core::analytics::BankSummary;
use moneywatcher_core::domain::{
    Account, AccountId, AccountKind, Category, CategoryId, CategoryKind, Money, NewAccount,
    NewTransaction, Transaction, TransactionSource,
};
use moneywatcher_core::storage::TransactionFilter;

fn keys(value: &serde_json::Value) -> Vec<String> {
    value
        .as_object()
        .expect("el tipo serializa como objeto JSON")
        .keys()
        .cloned()
        .collect()
}

fn assert_camel_case(value: &serde_json::Value, context: &str) {
    for key in keys(value) {
        assert!(
            !key.contains('_'),
            "{context} expone `{key}` en snake_case; el contrato de la IPC es camelCase"
        );
    }
}

fn sample_account() -> Account {
    Account {
        id: AccountId(1),
        name: "Cuenta nómina".into(),
        bank: "BBVA".into(),
        kind: AccountKind::Checking,
        opening_balance: Money::from_minor_units(210_045),
        archived: false,
    }
}

fn sample_transaction() -> Transaction {
    Transaction {
        id: moneywatcher_core::domain::TransactionId(7),
        account_id: AccountId(1),
        booked_on: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
        value_on: None,
        description: "NOMINA MENSUAL DATAFORGE SL".into(),
        counterparty: None,
        amount: Money::from_minor_units(235_000),
        balance_after: Some(Money::from_minor_units(445_045)),
        category_id: Some(CategoryId(1)),
        notes: None,
        source: TransactionSource::Imported,
        import_id: None,
        fingerprint: "abc123".into(),
    }
}

#[test]
fn account_types_serialize_in_camel_case() {
    let account = serde_json::to_value(sample_account()).unwrap();
    assert_camel_case(&account, "Account");
    assert_eq!(account["openingBalance"], serde_json::json!("2100.45"));

    let new_account = serde_json::to_value(NewAccount {
        name: "Cuenta nómina".into(),
        bank: "BBVA".into(),
        kind: AccountKind::Checking,
        opening_balance: Money::from_minor_units(210_045),
    })
    .unwrap();
    assert_camel_case(&new_account, "NewAccount");
}

#[test]
fn transaction_types_serialize_in_camel_case() {
    let transaction = serde_json::to_value(sample_transaction()).unwrap();
    assert_camel_case(&transaction, "Transaction");
    assert_eq!(transaction["accountId"], serde_json::json!(1));
    assert_eq!(transaction["bookedOn"], serde_json::json!("2026-03-01"));

    let new_transaction = serde_json::to_value(NewTransaction {
        account_id: AccountId(1),
        booked_on: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
        value_on: None,
        description: "NOMINA MENSUAL DATAFORGE SL".into(),
        counterparty: None,
        amount: Money::from_minor_units(235_000),
        balance_after: None,
        category_id: None,
        notes: None,
        source: TransactionSource::Imported,
        import_id: None,
    })
    .unwrap();
    assert_camel_case(&new_transaction, "NewTransaction");
}

#[test]
fn category_serializes_in_camel_case() {
    let category = serde_json::to_value(Category {
        id: CategoryId(1),
        name: "Groceries".into(),
        kind: CategoryKind::Expense,
        color: "#d4694a".into(),
        is_system: true,
    })
    .unwrap();
    assert_camel_case(&category, "Category");
    assert_eq!(category["isSystem"], serde_json::json!(true));
}

/// Los comandos que crean entidades deserializan lo que manda el frontend, así
/// que la convención tiene que valer también en sentido contrario.
#[test]
fn new_account_deserializes_from_the_frontend_payload() {
    let payload = serde_json::json!({
        "name": "Cuenta nómina",
        "bank": "BBVA",
        "kind": "checking",
        "openingBalance": "2100.45",
    });

    let account: NewAccount = serde_json::from_value(payload).expect("payload del frontend válido");
    assert_eq!(account.opening_balance, Money::from_minor_units(210_045));
}

#[test]
fn bank_summary_serializes_in_camel_case() {
    let summary = serde_json::to_value(BankSummary {
        bank: "Revolut".into(),
        accounts: 2,
        balance: Money::from_minor_units(58_500),
        income: Money::from_minor_units(50_000),
        expense: Money::from_minor_units(1_500),
    })
    .unwrap();
    assert_camel_case(&summary, "BankSummary");
}

/// El filtro viaja del frontend al núcleo en cada consulta del dashboard y de
/// la tabla de movimientos: si un campo no se deserializa, la vista enseña
/// datos de otro periodo sin decirlo.
#[test]
fn transaction_filter_reads_the_payload_sent_by_the_frontend() {
    let payload = serde_json::json!({
        "accountIds": [1, 2],
        "from": "2026-01-01",
        "to": "2026-03-31",
        "uncategorizedOnly": true,
    });
    let filter: TransactionFilter = serde_json::from_value(payload).expect("filtro válido");
    assert_eq!(filter.account_ids.len(), 2);
    assert!(filter.uncategorized_only);
    assert_eq!(
        filter.to.map(|date| date.to_string()).as_deref(),
        Some("2026-03-31")
    );
}
