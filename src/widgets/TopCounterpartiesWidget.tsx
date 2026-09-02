import { formatMoney, toChartValue } from "../lib/money";
import type { CounterpartyTotal } from "../types/ipc";

import { WidgetEmpty, WidgetFrame } from "./WidgetFrame";

interface TopCounterpartiesWidgetProps {
  title: string;
  counterparties: CounterpartyTotal[];
}

/**
 * Dónde se va el dinero, por comercio. Se dibuja con barras proporcionales en
 * CSS en vez de con un gráfico: en una lista corta se lee mejor y ocupa menos.
 */
export function TopCounterpartiesWidget({
  title,
  counterparties,
}: TopCounterpartiesWidgetProps) {
  if (counterparties.length === 0) {
    return (
      <WidgetFrame title={title}>
        <WidgetEmpty message="Sin gastos registrados en este periodo." />
      </WidgetFrame>
    );
  }

  const largest = Math.max(...counterparties.map((entry) => toChartValue(entry.total)), 1);

  return (
    <WidgetFrame title={title}>
      <ul style={{ margin: 0, padding: 0, listStyle: "none", display: "grid", gap: 10 }}>
        {counterparties.map((entry) => (
          <li key={entry.label}>
            <div className="row" style={{ justifyContent: "space-between", gap: 12 }}>
              <span
                style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                title={entry.label}
              >
                {entry.label}
              </span>
              <span className="tabular small">{formatMoney(entry.total)}</span>
            </div>
            <div
              style={{
                height: 4,
                marginTop: 4,
                borderRadius: 999,
                background: "var(--bg-hover)",
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  width: `${(toChartValue(entry.total) / largest) * 100}%`,
                  height: "100%",
                  background: "var(--expense)",
                }}
              />
            </div>
          </li>
        ))}
      </ul>
    </WidgetFrame>
  );
}
