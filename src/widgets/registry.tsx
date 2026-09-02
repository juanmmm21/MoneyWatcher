import type { ReactElement } from "react";

import type { DashboardOverview, NewWidget, Widget } from "../types/ipc";

import { BankBalancesWidget } from "./BankBalancesWidget";
import { BreakdownWidget } from "./BreakdownWidget";
import { MonthlyFlowWidget } from "./MonthlyFlowWidget";
import { TopCounterpartiesWidget } from "./TopCounterpartiesWidget";
import { TotalsWidget } from "./TotalsWidget";
import { WidgetEmpty, WidgetFrame } from "./WidgetFrame";

export type WidgetKind =
  | "totals"
  | "monthly_flow"
  | "expense_breakdown"
  | "income_breakdown"
  | "bank_balances"
  | "top_counterparties";

interface WidgetDefinition {
  kind: WidgetKind;
  label: string;
  description: string;
  defaultTitle: string;
  defaultSize: { w: number; h: number };
}

/**
 * Catálogo de widgets disponibles. Añadir uno nuevo es añadir una entrada aquí
 * y su caso en `renderWidget`: el resto del dashboard no necesita cambios.
 */
export const WIDGET_CATALOG: WidgetDefinition[] = [
  {
    kind: "totals",
    label: "Resumen del periodo",
    description: "Ingresos, gastos, balance y tasa de ahorro.",
    defaultTitle: "Resumen",
    defaultSize: { w: 6, h: 4 },
  },
  {
    kind: "monthly_flow",
    label: "Flujo mensual",
    description: "Barras de ingresos y gastos por mes.",
    defaultTitle: "Ingresos y gastos por mes",
    defaultSize: { w: 6, h: 6 },
  },
  {
    kind: "expense_breakdown",
    label: "Gastos por categoría",
    description: "Reparto del gasto del periodo.",
    defaultTitle: "Gastos por categoría",
    defaultSize: { w: 6, h: 6 },
  },
  {
    kind: "income_breakdown",
    label: "Ingresos por categoría",
    description: "De dónde viene el dinero que entra.",
    defaultTitle: "Ingresos por categoría",
    defaultSize: { w: 6, h: 6 },
  },
  {
    kind: "bank_balances",
    label: "Bancos",
    description: "Saldo y flujo de cada entidad.",
    defaultTitle: "Bancos",
    defaultSize: { w: 6, h: 6 },
  },
  {
    kind: "top_counterparties",
    label: "Dónde se va el dinero",
    description: "Comercios y recibos con más gasto acumulado.",
    defaultTitle: "Dónde se va el dinero",
    defaultSize: { w: 6, h: 6 },
  },
];

/** Rejilla inicial: lo que ve alguien que abre la app por primera vez. */
export const DEFAULT_LAYOUT: NewWidget[] = [
  { kind: "totals", title: "Resumen", config: {}, placement: { x: 0, y: 0, w: 6, h: 4 } },
  {
    kind: "bank_balances",
    title: "Bancos",
    config: {},
    placement: { x: 6, y: 0, w: 6, h: 4 },
  },
  {
    kind: "monthly_flow",
    title: "Ingresos y gastos por mes",
    config: {},
    placement: { x: 0, y: 4, w: 12, h: 6 },
  },
  {
    kind: "expense_breakdown",
    title: "Gastos por categoría",
    config: {},
    placement: { x: 0, y: 10, w: 6, h: 6 },
  },
  {
    kind: "top_counterparties",
    title: "Dónde se va el dinero",
    config: {},
    placement: { x: 6, y: 10, w: 6, h: 6 },
  },
];

export function widgetDefinition(kind: string): WidgetDefinition | undefined {
  return WIDGET_CATALOG.find((definition) => definition.kind === kind);
}

export function renderWidget(
  widget: Widget,
  overview: DashboardOverview,
  currency: string,
): ReactElement {
  switch (widget.kind as WidgetKind) {
    case "totals":
      return <TotalsWidget title={widget.title} totals={overview.totals} currency={currency} />;
    case "monthly_flow":
      return (
        <MonthlyFlowWidget
          title={widget.title}
          months={overview.monthly}
          currency={currency}
          showNet={widget.config.showNet !== false}
        />
      );
    case "expense_breakdown":
      return (
        <BreakdownWidget
          title={widget.title}
          slices={overview.expensesByCategory}
          currency={currency}
        />
      );
    case "income_breakdown":
      return (
        <BreakdownWidget
          title={widget.title}
          slices={overview.incomeByCategory}
          currency={currency}
        />
      );
    case "bank_balances":
      return <BankBalancesWidget title={widget.title} banks={overview.banks} />;
    case "top_counterparties":
      return (
        <TopCounterpartiesWidget
          title={widget.title}
          counterparties={overview.topCounterparties}
          currency={currency}
        />
      );
    default:
      // Un widget guardado por una versión más nueva de la app: se avisa en vez
      // de romper todo el dashboard.
      return (
        <WidgetFrame title={widget.title}>
          <WidgetEmpty message={`Tipo de widget desconocido: ${widget.kind}`} />
        </WidgetFrame>
      );
  }
}
