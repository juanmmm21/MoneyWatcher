import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  ComposedChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { formatMoney, formatMonth, toChartValue } from "../lib/money";
import type { MonthlyFlow } from "../types/ipc";

import { WidgetEmpty, WidgetFrame } from "./WidgetFrame";

interface MonthlyFlowWidgetProps {
  title: string;
  months: MonthlyFlow[];
  /** Añade la línea de balance neto sobre las barras. */
  showNet?: boolean;
}

interface ChartPoint {
  month: string;
  label: string;
  income: number;
  expense: number;
  net: number;
  raw: MonthlyFlow;
}

/** Ingresos y gastos mes a mes, la vista que responde a "¿en qué se me va?". */
export function MonthlyFlowWidget({
  title,
  months,
  showNet = true,
}: MonthlyFlowWidgetProps) {
  if (months.length === 0) {
    return (
      <WidgetFrame title={title}>
        <WidgetEmpty message="Todavía no hay movimientos en este periodo." />
      </WidgetFrame>
    );
  }

  const data: ChartPoint[] = months.map((month) => ({
    month: month.month,
    label: formatMonth(month.month),
    income: toChartValue(month.income),
    expense: toChartValue(month.expense),
    net: toChartValue(month.net),
    raw: month,
  }));

  const Chart = showNet ? ComposedChart : BarChart;

  return (
    <WidgetFrame title={title}>
      <ResponsiveContainer width="100%" height="100%" minHeight={180}>
        <Chart data={data} margin={{ top: 8, right: 8, bottom: 0, left: 8 }}>
          <CartesianGrid stroke="var(--border)" vertical={false} />
          <XAxis dataKey="label" tickLine={false} axisLine={false} />
          <YAxis
            tickLine={false}
            axisLine={false}
            width={64}
            tickFormatter={(value: number) =>
              formatMoney(String(Math.round(value)), { compact: true })
            }
          />
          <Tooltip
            cursor={{ fill: "var(--bg-hover)" }}
            content={({ active, payload }) => {
              if (!active || !payload?.length) return null;
              const point = payload[0]?.payload as ChartPoint;
              return (
                <div className="chart-tooltip">
                  <strong>{point.label}</strong>
                  <div className="tabular amount--income">
                    Ingresos {formatMoney(point.raw.income)}
                  </div>
                  <div className="tabular amount--expense">
                    Gastos {formatMoney(point.raw.expense)}
                  </div>
                  <div className="tabular muted">
                    Balance {formatMoney(point.raw.net)}
                  </div>
                </div>
              );
            }}
          />
          <Bar dataKey="income" fill="var(--income)" radius={[4, 4, 0, 0]} maxBarSize={28} />
          <Bar dataKey="expense" fill="var(--expense)" radius={[4, 4, 0, 0]} maxBarSize={28} />
          {showNet ? (
            <Line
              type="monotone"
              dataKey="net"
              stroke="var(--accent)"
              strokeWidth={2}
              dot={false}
            />
          ) : null}
        </Chart>
      </ResponsiveContainer>
    </WidgetFrame>
  );
}
