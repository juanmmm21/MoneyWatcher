mod commands;
mod error;
mod state;

use tauri::Manager;

/// Arranca la aplicación de escritorio.
///
/// La base de datos se abre una sola vez al inicio: si no se puede abrir, no
/// tiene sentido seguir, y es preferible fallar aquí de forma visible que
/// dejar una ventana en blanco sin explicación.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info::app_info,
            commands::accounts::list_accounts,
            commands::accounts::create_account,
            commands::accounts::rename_account,
            commands::accounts::set_account_archived,
            commands::accounts::delete_account,
            commands::categories::list_categories,
            commands::categories::create_category,
            commands::categories::update_category,
            commands::categories::delete_category,
            commands::transactions::list_transactions,
            commands::transactions::create_transaction,
            commands::transactions::set_transaction_category,
            commands::transactions::set_transaction_notes,
            commands::transactions::categorize_transactions,
            commands::transactions::delete_transaction,
            commands::imports::preview_statement,
            commands::imports::import_statement,
            commands::imports::list_imports,
            commands::imports::revert_import,
            commands::rules::list_rules,
            commands::rules::create_rule,
            commands::rules::delete_rule,
            commands::rules::run_rules,
            commands::rules::correct_transaction_category,
            commands::analytics::dashboard_overview,
            commands::analytics::monthly_flow,
            commands::analytics::category_breakdown,
            commands::analytics::bank_summaries,
            commands::analytics::top_counterparties,
            commands::widgets::list_widgets,
            commands::widgets::create_widget,
            commands::widgets::update_widget,
            commands::widgets::save_widget_layout,
            commands::widgets::delete_widget,
            commands::transfers::transfer_settings,
            commands::transfers::set_transfer_detection,
            commands::transfers::detect_transfers,
            commands::transfers::list_transfers,
            commands::transfers::set_transfer_dismissed,
            commands::assistant::assistant_settings,
            commands::assistant::set_assistant_settings,
            commands::assistant::assistant_status,
            commands::assistant::suggest_categories,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MoneyWatcher");
}
