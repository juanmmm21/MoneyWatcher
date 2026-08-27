import { formatBps, formatMoney } from "../lib/money";
import type { FlowTotals } from "../types/ipc";

import { WidgetFrame } from "./WidgetFrame";

interface TotalsWidgetProps {
  title: string;
  totals: FlowTotals;
  currency: string;
}

/** Los cuatro números que resumen el periodo: entra, sale, queda y qué parte se ahorra. */
export function TotalsWidget({ title, totals, currency }: TotalsWidgetProps) {
  const netIsPositive = !totals.net.startsWith("-");

  return (
    <WidgetFrame title={title}>
      <div className="grid-two" style={{ gap: 18 }}>
        <div className="stat">
          <span className="stat__label">Ingresos</span>
          <span className="stat__value tabular amount--income">
            {formatMoney(totals.income, { currency })}
          </span>
        </div>
        <div className="stat">
          <span className="stat__label">Gastos</span>
          <span className="stat__value tabular amount--expense">
            {formatMoney(totals.expense, { currency })}
          </span>
        </div>
        <div className="stat">
          <span className="stat__label">Balance del periodo</span>
          <span
            className={`stat__value tabular ${netIsPositive ? "amount--income" : "amount--expense"}`}
          >
            {formatMoney(totals.net, { currency })}
          </span>
        </div>
        <div className="stat">
          <span className="stat__label">Tasa de ahorro</span>
          <span className="stat__value tabular">{formatBps(totals.savingsRateBps)}</span>
          <span className="stat__hint">de lo ingresado en el periodo</span>
        </div>
      </div>
    </WidgetFrame>
  );
}
