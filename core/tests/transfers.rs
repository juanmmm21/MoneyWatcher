//! Detección de traspasos entre cuentas propias, de punta a punta: emparejar,
//! excluir de las agregaciones y descartar un par que el usuario no reconoce.

use chrono::NaiveDate;
use moneywatcher_core::domain::{
    AccountId, AccountKind, Money, NewAccount, NewTransaction, TransactionSource,
};
use moneywatcher_core::storage::{Database, TransactionFilter};
use moneywatcher_core::transfers::{self, TransferDetection};

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 4, day).expect("fecha válida")
}

fn movement(account_id: AccountId, day: u32, description: &str, minor: i64) -> NewTransaction {
    NewTransaction {
        account_id,
        booked_on: date(day),
        value_on: None,
        description: description.into(),
        counterparty: None,
        amount: Money::from_minor_units(minor),
        balance_after: None,
        category_id: None,
        notes: None,
        source: TransactionSource::Imported,
        import_id: None,
    }
}

/// Una nómina, un gasto y un traspaso de 300 € de la corriente a la de ahorro
/// que el banco abona al día siguiente.
fn seeded_database() -> (Database, AccountId, AccountId) {
    let mut database = Database::open_in_memory().expect("base de datos en memoria");

    let checking = database
        .create_account(&NewAccount {
            name: "Nómina".into(),
            bank: "Santander".into(),
            kind: AccountKind::Checking,
        })
        .unwrap();
    let savings = database
        .create_account(&NewAccount {
            name: "Ahorro".into(),
            bank: "BBVA".into(),
            kind: AccountKind::Savings,
        })
        .unwrap();

    database
        .insert_transactions(&[
            movement(checking.id, 1, "NOMINA ABRIL", 180_000),
            movement(checking.id, 5, "MERCADONA", -4_512),
            movement(checking.id, 10, "TRASPASO A AHORRO", -30_000),
            movement(savings.id, 11, "TRASPASO DESDE NOMINA", 30_000),
        ])
        .unwrap();

    (database, checking.id, savings.id)
}

fn totals(database: &Database, exclude_transfers: bool) -> (i64, i64) {
    let totals = database
        .flow_totals(&TransactionFilter {
            exclude_transfers,
            ..Default::default()
        })
        .unwrap();
    (totals.income.minor_units(), totals.expense.minor_units())
}

#[test]
fn detects_a_transfer_and_keeps_it_out_of_the_totals() {
    let (mut database, _, _) = seeded_database();

    assert_eq!(
        transfers::detect_transfers(&mut database).unwrap(),
        TransferDetection {
            linked: 1,
            active: 1
        }
    );

    // Sin excluir, el traspaso cuenta como ingreso y como gasto a la vez.
    assert_eq!(totals(&database, false), (210_000, 34_512));
    assert_eq!(totals(&database, true), (180_000, 4_512));
}

/// Volver a pasar el detector no puede duplicar enlaces ni encontrar pares
/// nuevos: los movimientos ya emparejados dejan de ser candidatos.
#[test]
fn detecting_twice_changes_nothing() {
    let (mut database, _, _) = seeded_database();
    transfers::detect_transfers(&mut database).unwrap();

    assert_eq!(
        transfers::detect_transfers(&mut database).unwrap(),
        TransferDetection {
            linked: 0,
            active: 1
        }
    );
}

#[test]
fn a_dismissed_link_counts_again_and_is_not_proposed_twice() {
    let (mut database, _, _) = seeded_database();
    transfers::detect_transfers(&mut database).unwrap();

    let link = database
        .transfer_links(10)
        .unwrap()
        .pop()
        .expect("un enlace");
    assert_eq!(link.amount, Money::from_minor_units(30_000));
    assert_eq!(link.day_gap, 1);
    assert_eq!(link.from_account, "Santander · Nómina");
    assert_eq!(link.to_account, "BBVA · Ahorro");

    database.set_transfer_dismissed(link.id, true).unwrap();
    assert_eq!(database.count_active_transfers().unwrap(), 0);
    assert_eq!(totals(&database, true), (210_000, 34_512));

    // El par descartado no vuelve a proponerse: seguiría siendo el mejor
    // candidato y la app estaría discutiendo con el usuario.
    assert_eq!(
        transfers::detect_transfers(&mut database).unwrap(),
        TransferDetection {
            linked: 0,
            active: 0
        }
    );

    // Y se puede reconocer de nuevo sin volver a detectarlo.
    database.set_transfer_dismissed(link.id, false).unwrap();
    assert_eq!(database.count_active_transfers().unwrap(), 1);
}

#[test]
fn marks_which_transactions_of_a_page_are_transfers() {
    let (mut database, _, _) = seeded_database();
    transfers::detect_transfers(&mut database).unwrap();

    let page = database
        .transactions(&TransactionFilter::default())
        .unwrap();
    let ids: Vec<_> = page.iter().map(|transaction| transaction.id).collect();
    let linked = database.transfer_transaction_ids(&ids).unwrap();

    assert_eq!(linked.len(), 2);
    for transaction in &page {
        let is_transfer = transaction.description.starts_with("TRASPASO");
        assert_eq!(
            linked.contains(&transaction.id),
            is_transfer,
            "«{}» está marcado al revés",
            transaction.description
        );
    }
}

/// Borrar un movimiento se lleva por delante el enlace: un traspaso con una
/// sola cara no significa nada.
#[test]
fn deleting_a_transaction_removes_its_link() {
    let (mut database, _, _) = seeded_database();
    transfers::detect_transfers(&mut database).unwrap();

    let link = database
        .transfer_links(10)
        .unwrap()
        .pop()
        .expect("un enlace");
    database.delete_transaction(link.outgoing_id).unwrap();

    assert_eq!(database.count_active_transfers().unwrap(), 0);
    assert!(database.transfer_links(10).unwrap().is_empty());
}

#[test]
fn the_setting_is_off_until_the_user_turns_it_on() {
    let (database, _, _) = seeded_database();
    assert!(!transfers::detection_enabled(&database).unwrap());

    transfers::set_detection_enabled(&database, true).unwrap();
    assert!(transfers::detection_enabled(&database).unwrap());

    transfers::set_detection_enabled(&database, false).unwrap();
    assert!(!transfers::detection_enabled(&database).unwrap());
}

/// Las agregaciones por banco montan el filtro dos veces, una por subconsulta,
/// así que la exclusión tiene que sobrevivir a esa reescritura.
#[test]
fn bank_summaries_also_leave_transfers_out() {
    let (mut database, _, _) = seeded_database();
    transfers::detect_transfers(&mut database).unwrap();

    let summaries = database
        .bank_summaries(&TransactionFilter {
            exclude_transfers: true,
            ..Default::default()
        })
        .unwrap();

    let bbva = summaries.iter().find(|s| s.bank == "BBVA").unwrap();
    assert_eq!(bbva.income, Money::from_minor_units(0));

    let santander = summaries.iter().find(|s| s.bank == "Santander").unwrap();
    assert_eq!(santander.income, Money::from_minor_units(180_000));
    assert_eq!(santander.expense, Money::from_minor_units(4_512));
}
