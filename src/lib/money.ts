import type { MoneyString } from "../types/ipc";

/**
 * Formateo de importes.
 *
 * Los cálculos ya vienen hechos del núcleo; aquí solo se presenta. Aun así se
 * trabaja sobre la cadena decimal en lugar de convertirla a `number`, para que
 * un importe grande no pierda céntimos por el camino.
 */

interface Parts {
  negative: boolean;
  units: string;
  cents: string;
}

function split(amount: MoneyString): Parts {
  const negative = amount.trimStart().startsWith("-");
  const [units = "0", cents = "00"] = amount.replace("-", "").split(".");
  return { negative, units, cents: cents.padEnd(2, "0").slice(0, 2) };
}

function groupThousands(units: string, separator: string): string {
  return units.replace(/\B(?=(\d{3})+(?!\d))/g, separator);
}

export interface FormatOptions {
  currency?: string;
  /** Oculta los céntimos (útil en ejes de gráficos). */
  compact?: boolean;
  /** Antepone el signo también a los positivos. */
  showSign?: boolean;
}

export function formatMoney(amount: MoneyString, options: FormatOptions = {}): string {
  const { currency = "EUR", compact = false, showSign = false } = options;
  const { negative, units, cents } = split(amount);

  const grouped = groupThousands(units, ".");
  const body = compact ? grouped : `${grouped},${cents}`;
  const symbol = currencySymbol(currency);
  const sign = negative ? "−" : showSign ? "+" : "";

  return `${sign}${body} ${symbol}`.trim();
}

export function currencySymbol(currency: string): string {
  switch (currency.toUpperCase()) {
    case "EUR":
      return "€";
    case "USD":
      return "$";
    case "GBP":
      return "£";
    default:
      return currency.toUpperCase();
  }
}

/** Signo del importe sin convertirlo a número. */
export function isNegative(amount: MoneyString): boolean {
  return amount.trimStart().startsWith("-");
}

/**
 * Convierte a `number` solo para dibujar gráficos, donde la precisión exacta no
 * es observable. Nunca se usa este valor para mostrar cifras ni para calcular.
 */
export function toChartValue(amount: MoneyString): number {
  return Number.parseFloat(amount);
}

/** Puntos básicos a porcentaje legible: 9621 -> "96,2 %". */
export function formatBps(bps: number): string {
  return `${(bps / 100).toFixed(1).replace(".", ",")} %`;
}

/** "2026-03" -> "mar 2026" */
export function formatMonth(month: string): string {
  const [year, monthNumber] = month.split("-");
  const names = [
    "ene", "feb", "mar", "abr", "may", "jun",
    "jul", "ago", "sep", "oct", "nov", "dic",
  ];
  const index = Number.parseInt(monthNumber ?? "1", 10) - 1;
  return `${names[index] ?? monthNumber} ${year}`;
}

/** "2026-03-14" -> "14 mar 2026" */
export function formatDate(date: string): string {
  const [year, month, day] = date.split("-");
  return `${day} ${formatMonth(`${year}-${month}`)}`;
}
