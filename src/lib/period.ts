import type { TransactionFilter } from "../types/ipc";

/** Periodos predefinidos del selector de la barra superior. */
export type PeriodId = "this-month" | "last-3-months" | "this-year" | "last-12-months" | "all";

export interface Period {
  id: PeriodId;
  label: string;
  from: string | null;
  to: string | null;
}

function isoDate(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function startOfMonth(date: Date, monthsBack = 0): Date {
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() - monthsBack, 1));
}

/**
 * Construye el periodo a partir del reloj del sistema. La fecha se maneja en
 * UTC para que el rango no baile según la zona horaria del usuario.
 */
export function buildPeriod(id: PeriodId, today = new Date()): Period {
  const end = isoDate(today);

  switch (id) {
    case "this-month":
      return { id, label: "Este mes", from: isoDate(startOfMonth(today)), to: end };
    case "last-3-months":
      return { id, label: "3 meses", from: isoDate(startOfMonth(today, 2)), to: end };
    case "this-year":
      return {
        id,
        label: "Este año",
        from: isoDate(new Date(Date.UTC(today.getUTCFullYear(), 0, 1))),
        to: end,
      };
    case "last-12-months":
      return { id, label: "12 meses", from: isoDate(startOfMonth(today, 11)), to: end };
    case "all":
      return { id, label: "Todo", from: null, to: null };
  }
}

export const PERIODS: PeriodId[] = [
  "this-month",
  "last-3-months",
  "last-12-months",
  "this-year",
  "all",
];

export function periodFilter(period: Period, accountIds: number[]): TransactionFilter {
  return {
    from: period.from,
    to: period.to,
    accountIds: accountIds.length > 0 ? accountIds : [],
  };
}
