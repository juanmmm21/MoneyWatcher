import { useCallback, useEffect, useMemo, useState } from "react";
import GridLayout, { type Layout, WidthProvider } from "react-grid-layout";

import { useAsync } from "../hooks/useAsync";
import { api, errorMessage } from "../lib/ipc";
import type { DashboardOverview, TransactionFilter, Widget } from "../types/ipc";
import { DEFAULT_LAYOUT, WIDGET_CATALOG, renderWidget, widgetDefinition } from "../widgets/registry";

const ResponsiveGrid = WidthProvider(GridLayout);
const GRID_COLUMNS = 12;
const ROW_HEIGHT = 44;

interface DashboardViewProps {
  filter: TransactionFilter;
  currency: string;
  /** Cambia cuando los datos se modifican fuera de esta vista (una importación). */
  dataVersion: number;
  onReviewPending: () => void;
}

/**
 * Rejilla de widgets configurable. El layout se guarda en la base de datos al
 * soltar el ratón, no en cada píxel arrastrado, para no escribir en disco
 * durante toda la interacción.
 */
export function DashboardView({
  filter,
  currency,
  dataVersion,
  onReviewPending,
}: DashboardViewProps) {
  const [widgets, setWidgets] = useState<Widget[]>([]);
  const [widgetsError, setWidgetsError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const overview = useAsync<DashboardOverview>(
    () => api.dashboardOverview(filter),
    [JSON.stringify(filter), dataVersion],
  );

  const loadWidgets = useCallback(async () => {
    try {
      const stored = await api.listWidgets();
      if (stored.length > 0) {
        setWidgets(stored);
        return;
      }

      // Primer arranque: se siembra un dashboard con sentido en vez de dejar
      // al usuario delante de un lienzo vacío.
      const created: Widget[] = [];
      for (const widget of DEFAULT_LAYOUT) {
        created.push(await api.createWidget(widget));
      }
      setWidgets(created);
    } catch (error) {
      setWidgetsError(errorMessage(error));
    }
  }, []);

  useEffect(() => {
    void loadWidgets();
  }, [loadWidgets]);

  const layout: Layout[] = useMemo(
    () =>
      widgets.map((widget) => ({
        i: String(widget.id),
        x: widget.placement.x,
        y: widget.placement.y,
        w: widget.placement.w,
        h: widget.placement.h,
        minW: 3,
        minH: 3,
      })),
    [widgets],
  );

  const persistLayout = useCallback(async (next: Layout[]) => {
    try {
      await api.saveWidgetLayout(
        next.map((item) => ({ id: Number(item.i), x: item.x, y: item.y, w: item.w, h: item.h })),
      );
      setWidgets((current) =>
        current.map((widget) => {
          const placed = next.find((item) => Number(item.i) === widget.id);
          return placed
            ? { ...widget, placement: { x: placed.x, y: placed.y, w: placed.w, h: placed.h } }
            : widget;
        }),
      );
    } catch (error) {
      setWidgetsError(errorMessage(error));
    }
  }, []);

  const addWidget = useCallback(
    async (kind: string) => {
      const definition = widgetDefinition(kind);
      if (!definition) return;

      setAdding(false);
      try {
        const bottom = widgets.reduce(
          (lowest, widget) => Math.max(lowest, widget.placement.y + widget.placement.h),
          0,
        );
        const created = await api.createWidget({
          kind: definition.kind,
          title: definition.defaultTitle,
          config: {},
          placement: { x: 0, y: bottom, ...definition.defaultSize },
        });
        setWidgets((current) => [...current, created]);
      } catch (error) {
        setWidgetsError(errorMessage(error));
      }
    },
    [widgets],
  );

  const removeWidget = useCallback(async (widgetId: number) => {
    try {
      await api.deleteWidget(widgetId);
      setWidgets((current) => current.filter((widget) => widget.id !== widgetId));
    } catch (error) {
      setWidgetsError(errorMessage(error));
    }
  }, []);

  if (overview.error) {
    return <div className="banner banner--error">No se pudo cargar el dashboard: {overview.error}</div>;
  }

  if (!overview.data) {
    return <div className="muted">Cargando…</div>;
  }

  return (
    <div className="stack">
      {widgetsError ? <div className="banner banner--error">{widgetsError}</div> : null}

      {overview.data.uncategorized > 0 ? (
        <div className="banner banner--warning">
          <span>
            {overview.data.uncategorized}{" "}
            {overview.data.uncategorized === 1
              ? "movimiento sin categorizar"
              : "movimientos sin categorizar"}
            .
          </span>
          <button type="button" className="button" onClick={onReviewPending}>
            Revisar
          </button>
        </div>
      ) : null}

      <div className="row" style={{ justifyContent: "flex-end" }}>
        <div style={{ position: "relative" }}>
          <button type="button" className="button" onClick={() => setAdding((open) => !open)}>
            + Añadir widget
          </button>
          {adding ? (
            <div
              className="card"
              style={{ position: "absolute", right: 0, top: 38, zIndex: 10, width: 300 }}
            >
              <div className="card__body" style={{ display: "grid", gap: 4 }}>
                {WIDGET_CATALOG.map((definition) => (
                  <button
                    key={definition.kind}
                    type="button"
                    className="sidebar__nav-item"
                    onClick={() => void addWidget(definition.kind)}
                  >
                    <span>
                      <div style={{ color: "var(--text)" }}>{definition.label}</div>
                      <div className="small">{definition.description}</div>
                    </span>
                  </button>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      </div>

      <ResponsiveGrid
        className="layout"
        layout={layout}
        cols={GRID_COLUMNS}
        rowHeight={ROW_HEIGHT}
        margin={[14, 14]}
        containerPadding={[0, 0]}
        draggableHandle=".widget__drag-handle"
        onDragStop={(next) => void persistLayout(next)}
        onResizeStop={(next) => void persistLayout(next)}
      >
        {widgets.map((widget) => (
          <div key={String(widget.id)} style={{ position: "relative" }}>
            {renderWidget(widget, overview.data!, currency)}
            <button
              type="button"
              className="button button--ghost"
              title="Quitar widget"
              onClick={() => void removeWidget(widget.id)}
              style={{ position: "absolute", top: 6, right: 8, padding: "2px 8px" }}
            >
              ×
            </button>
          </div>
        ))}
      </ResponsiveGrid>
    </div>
  );
}
