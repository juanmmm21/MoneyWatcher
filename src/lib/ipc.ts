import { invoke } from "@tauri-apps/api/core";

import type {
  Account,
  AiProvider,
  AppInfo,
  AssistantStatus,
  BankSummary,
  CategorizationSummary,
  Category,
  CategorySlice,
  CommandError,
  CorrectionResult,
  DashboardOverview,
  ImportRecord,
  ImportResult,
  MonthlyFlow,
  NewAccount,
  NewRule,
  NewWidget,
  Rule,
  StatementPreview,
  SuggestionBatch,
  Transaction,
  TransactionFilter,
  TransactionPage,
  TransferDetection,
  TransferSettings,
  Widget,
  WidgetPlacement,
} from "../types/ipc";

/**
 * Única puerta de entrada al núcleo. Ningún componente llama a `invoke`
 * directamente: así el contrato queda en un solo sitio y los errores del núcleo
 * llegan siempre con la misma forma.
 */

export function isCommandError(error: unknown): error is CommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error
  );
}

export function errorMessage(error: unknown): string {
  if (isCommandError(error)) {
    return error.message;
  }
  return error instanceof Error ? error.message : String(error);
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),

  listAccounts: (includeArchived = false) =>
    invoke<Account[]>("list_accounts", { includeArchived }),
  createAccount: (account: NewAccount) => invoke<Account>("create_account", { account }),
  renameAccount: (accountId: number, name: string, bank: string) =>
    invoke<Account>("rename_account", { accountId, name, bank }),
  setAccountArchived: (accountId: number, archived: boolean) =>
    invoke<Account>("set_account_archived", { accountId, archived }),
  deleteAccount: (accountId: number) => invoke<void>("delete_account", { accountId }),

  listCategories: () => invoke<Category[]>("list_categories"),

  listTransactions: (filter: TransactionFilter) =>
    invoke<TransactionPage>("list_transactions", { filter }),
  setTransactionCategory: (transactionId: number, categoryId: number | null) =>
    invoke<Transaction>("set_transaction_category", { transactionId, categoryId }),
  categorizeTransactions: (transactionIds: number[], categoryId: number | null) =>
    invoke<number>("categorize_transactions", { transactionIds, categoryId }),
  deleteTransaction: (transactionId: number) =>
    invoke<void>("delete_transaction", { transactionId }),

  previewStatement: (path: string) => invoke<StatementPreview>("preview_statement", { path }),
  importStatement: (accountId: number, path: string) =>
    invoke<ImportResult>("import_statement", { accountId, path }),
  listImports: (limit = 20) => invoke<ImportRecord[]>("list_imports", { limit }),
  revertImport: (importId: number) => invoke<number>("revert_import", { importId }),

  listRules: () => invoke<Rule[]>("list_rules"),
  createRule: (rule: NewRule) => invoke<Rule>("create_rule", { rule }),
  deleteRule: (ruleId: number) => invoke<void>("delete_rule", { ruleId }),
  runRules: () => invoke<CategorizationSummary>("run_rules"),
  correctTransactionCategory: (transactionId: number, categoryId: number, learn: boolean) =>
    invoke<CorrectionResult>("correct_transaction_category", {
      transactionId,
      categoryId,
      learn,
    }),

  dashboardOverview: (filter: TransactionFilter) =>
    invoke<DashboardOverview>("dashboard_overview", { filter }),
  monthlyFlow: (filter: TransactionFilter) => invoke<MonthlyFlow[]>("monthly_flow", { filter }),
  categoryBreakdown: (filter: TransactionFilter) =>
    invoke<CategorySlice[]>("category_breakdown", { filter }),
  bankSummaries: (filter: TransactionFilter) =>
    invoke<BankSummary[]>("bank_summaries", { filter }),

  listWidgets: () => invoke<Widget[]>("list_widgets"),
  createWidget: (widget: NewWidget) => invoke<Widget>("create_widget", { widget }),
  deleteWidget: (widgetId: number) => invoke<void>("delete_widget", { widgetId }),
  saveWidgetLayout: (layout: (WidgetPlacement & { id: number })[]) =>
    invoke<void>("save_widget_layout", { layout }),

  transferSettings: () => invoke<TransferSettings>("transfer_settings"),
  setTransferDetection: (enabled: boolean) =>
    invoke<TransferDetection>("set_transfer_detection", { enabled }),
  detectTransfers: () => invoke<TransferDetection>("detect_transfers"),
  setTransferDismissed: (linkId: number, dismissed: boolean) =>
    invoke<void>("set_transfer_dismissed", { linkId, dismissed }),

  assistantStatus: () => invoke<AssistantStatus>("assistant_status"),
  setAssistantSettings: (provider: AiProvider) =>
    invoke<AiProvider>("set_assistant_settings", { provider }),
  suggestCategories: (skipPatterns: string[]) =>
    invoke<SuggestionBatch>("suggest_categories", { skipPatterns }),
};
