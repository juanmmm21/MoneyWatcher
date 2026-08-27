import { Cell, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";

import { formatMoney, toChartValue } from "../lib/money";
import type { CategorySlice } from "../types/ipc";

import { WidgetEmpty, WidgetFrame } from "./WidgetFrame";

interface BreakdownWidgetProps {
  title: string;
  slices: CategorySlice[];
  currency: string;
  /** Cuántas categorías se listan bajo el gráfico. */
  limit?: number;
}

/** Reparto por categoría, con la leyenda como lista ordenada por peso. */
export function BreakdownWidget({ title, slices, currency, limit = 6 }: BreakdownWidgetProps) {
  if (slices.length === 0) {
    return (
      <WidgetFrame title={title}>
        <WidgetEmpty message="Sin datos para este periodo." />
      </WidgetFrame>
    );
  }

  const data = slices.map((slice) => ({ ...slice, value: toChartValue(slice.total) }));

  return (
    <WidgetFrame title={title}>
      <div style={{ display: "flex", gap: 16, height: "100%", minHeight: 180 }}>
        <div style={{ flex: "0 0 42%", minWidth: 120 }}>
          <ResponsiveContainer width="100%" height="100%" minHeight={140}>
            <PieChart>
              <Pie
                data={data}
                dataKey="value"
                nameKey="name"
                innerRadius="58%"
                outerRadius="88%"
                paddingAngle={2}
                stroke="none"
              >
                {data.map((slice) => (
                  <Cell key={slice.name} fill={slice.color} />
                ))}
              </Pie>
              <Tooltip
                content={({ active, payload }) => {
                  if (!active || !payload?.length) return null;
                  const slice = payload[0]?.payload as CategorySlice;
                  return (
                    <div className="chart-tooltip">
                      <strong>{slice.name}</strong>
                      <div className="tabular">{formatMoney(slice.total, { currency })}</div>
                      <div className="muted">{(slice.shareBps / 100).toFixed(1)} %</div>
                    </div>
                  );
                }}
              />
            </PieChart>
          </ResponsiveContainer>
        </div>

        <ul style={{ flex: 1, margin: 0, padding: 0, listStyle: "none", overflowY: "auto" }}>
          {slices.slice(0, limit).map((slice) => (
            <li
              key={slice.categoryId ?? slice.name}
              className="row"
              style={{ justifyContent: "space-between", padding: "3px 0" }}
            >
              <span className="row" style={{ minWidth: 0 }}>
                <span className="badge__dot" style={{ background: slice.color }} />
                <span
                  style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                >
                  {slice.name}
                </span>
              </span>
              <span className="tabular small muted">
                {formatMoney(slice.total, { currency })}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </WidgetFrame>
  );
}
