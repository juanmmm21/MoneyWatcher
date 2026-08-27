/**
 * Contrato con el núcleo Rust. Estos tipos son el espejo exacto de los DTOs que
 * devuelven los comandos de Tauri: si cambia uno en Rust, se actualiza aquí en
 * el mismo commit.
 *
 * Los importes viajan siempre como cadena decimal ("-1234.56") y nunca como
 * number, para que ningún redondeo de coma flotante toque el dinero.
 */

export type MoneyString = string;
export type IsoDate = string;

export type AccountKind = "checking" | "savings" | "credit" | "cash" | "investment";
export type CategoryKind = "income" | "expense" | "transfer";
export type Direction = "income" | "expense";
export type TransactionSource = "imported" | "manual";
export type RuleMatcher = "contains" | "starts_with" | "ends_with" | "equals";
export type RuleOrigin = "user" | "learned" | "assistant";

export interface Account {
  id: number;
  name: string;
  bank: string;
  kind: AccountKind;
  currency: string;
  openingBalance: MoneyString;
  archived: boolean;
  balance: MoneyString;
}

export interface NewAccount {
  name: string;
  bank: string;
  kind: AccountKind;
  currency: string;
  openingBalance: MoneyString;
}

export interface Category {
  id: number;
  name: string;
  kind: CategoryKind;
  color: string;
  isSystem: boolean;
}

export interface Transaction {
  id: number;
  accountId: number;
  bookedOn: IsoDate;
  valueOn: IsoDate | null;
  description: string;
  counterparty: string | null;
  amount: MoneyString;
  balanceAfter: MoneyString | null;
  categoryId: number | null;
  notes: string | null;
  source: TransactionSource;
  importId: number | null;
  fingerprint: string;
}

export interface TransactionFilter {
  accountIds?: number[];
  categoryIds?: number[];
  from?: IsoDate | null;
  to?: IsoDate | null;
  direction?: Direction | null;
  search?: string | null;
  uncategorizedOnly?: boolean;
  limit?: number | null;
  offset?: number | null;
}

export interface TransactionPage {
  transactions: Transaction[];
  total: number;
}

export interface Rule {
  id: number;
  matcher: RuleMatcher;
  pattern: string;
  accountId: number | null;
  direction: Direction | null;
  minAmount: MoneyString | null;
  maxAmount: MoneyString | null;
  categoryId: number;
  priority: number;
  origin: RuleOrigin;
  hits: number;
}

export interface NewRule {
  matcher: RuleMatcher;
  pattern: string;
  accountId: number | null;
  direction: Direction | null;
  minAmount: MoneyString | null;
  maxAmount: MoneyString | null;
  categoryId: number;
  priority: number;
  origin: RuleOrigin;
}

export interface CategorizationSummary {
  categorized: number;
  pending: number;
}

export interface CorrectionResult {
  learnedRule: Rule | null;
  applied: CategorizationSummary;
}

export interface FlowTotals {
  income: MoneyString;
  expense: MoneyString;
  net: MoneyString;
  savingsRateBps: number;
}

export interface MonthlyFlow {
  month: string;
  income: MoneyString;
  expense: MoneyString;
  net: MoneyString;
}

export interface CategorySlice {
  categoryId: number | null;
  name: string;
  color: string;
  total: MoneyString;
  shareBps: number;
  transactions: number;
}

export interface BankSummary {
  bank: string;
  accounts: number;
  balance: MoneyString;
  income: MoneyString;
  expense: MoneyString;
}

export interface CounterpartyTotal {
  label: string;
  total: MoneyString;
  transactions: number;
}

export interface DashboardOverview {
  totals: FlowTotals;
  monthly: MonthlyFlow[];
  expensesByCategory: CategorySlice[];
  incomeByCategory: CategorySlice[];
  banks: BankSummary[];
  topCounterparties: CounterpartyTotal[];
  uncategorized: number;
}

export type AmountColumns =
  | { kind: "single"; index: number }
  | { kind: "debitCredit"; debit: number; credit: number };

export interface ColumnMapping {
  bookedOn: number;
  valueOn: number | null;
  description: number;
  counterparty: number | null;
  amount: AmountColumns;
  balance: number | null;
}

export interface ParsedRow {
  line: number;
  bookedOn: IsoDate;
  valueOn: IsoDate | null;
  description: string;
  counterparty: string | null;
  amount: MoneyString;
  balanceAfter: MoneyString | null;
}

export interface SkippedRow {
  line: number;
  reason: string;
}

export interface StatementPreview {
  delimiter: string;
  headerLine: number;
  headers: string[];
  mapping: ColumnMapping;
  rows: ParsedRow[];
  skipped: SkippedRow[];
}

export interface ImportRecord {
  id: number;
  accountId: number;
  sourceName: string;
  importedAt: string;
  importedCount: number;
  duplicateCount: number;
}

export interface ImportResult {
  import: ImportRecord;
  imported: number;
  duplicates: number;
  skipped: number;
  categorization: CategorizationSummary;
}

export interface WidgetPlacement {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Widget {
  id: number;
  kind: string;
  title: string;
  config: Record<string, unknown>;
  placement: WidgetPlacement;
}

export interface NewWidget {
  kind: string;
  title: string;
  config: Record<string, unknown>;
  placement: WidgetPlacement;
}

export type AiProvider =
  | { kind: "disabled" }
  | { kind: "ollama"; endpoint: string; model: string };

export interface AssistantStatus {
  provider: AiProvider;
  enabled: boolean;
  leavesTheMachine: boolean;
  reachable: boolean;
  availableModels: string[];
  error: string | null;
}

export interface Suggestion {
  transactionId: number;
  description: string;
  categoryId: number;
  categoryName: string;
  confidence: number;
}

export interface AppInfo {
  databasePath: string;
  databaseSizeBytes: number;
  schemaVersion: number;
  accounts: number;
  transactions: number;
}

/** Error tipado que devuelven los comandos del núcleo. */
export interface CommandError {
  code: string;
  message: string;
}
