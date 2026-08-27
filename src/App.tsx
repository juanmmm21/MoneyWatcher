import { useCallback, useMemo, useState } from "react";

import { ImportDialog } from "./components/ImportDialog";
import { useAsync } from "./hooks/useAsync";
import { api } from "./lib/ipc";
import { PERIODS, buildPeriod, periodFilter, type PeriodId } from "./lib/period";
import type { Account, Category, ImportResult } from "./types/ipc";
import { DashboardView } from "./views/DashboardView";
import { RulesView } from "./views/RulesView";
import { SettingsView } from "./views/SettingsView";
import { TransactionsView } from "./views/TransactionsView";

type Section = "dashboard" | "transactions" | "rules" | "settings";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "dashboard", label: "Dashboard" },
  { id: "transactions", label: "Movimientos" },
  { id: "rules", label: "Reglas" },
  { id: "settings", label: "Ajustes" },
];

export function App() {
  const [section, setSection] = useState<Section>("dashboard");
  const [periodId, setPeriodId] = useState<PeriodId>("last-12-months");
  const [importing, setImporting] = useState(false);
  const [lastImport, setLastImport] = useState<ImportResult | null>(null);
  const [onlyPending, setOnlyPending] = useState(false);

  const accounts = useAsync<Account[]>(() => api.listAccounts(false), []);
  const categories = useAsync<Category[]>(() => api.listCategories(), []);
  const assistant = useAsync(() => api.assistantStatus(), []);

  const period = useMemo(() => buildPeriod(periodId), [periodId]);
  const filter = useMemo(() => periodFilter(period, []), [period]);

  // Todas las cuentas suelen compartir divisa; se toma la de la primera como
  // divisa de presentación de los totales agregados.
  const currency = accounts.data?.[0]?.currency ?? "EUR";

  const openTransactions = useCallback((pending: boolean) => {
    setOnlyPending(pending);
    setSection("transactions");
  }, []);

  const handleImported = useCallback(
    (result: ImportResult) => {
      setImporting(false);
      setLastImport(result);
      accounts.reload();
    },
    [accounts],
  );

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="sidebar__brand">
          <span className="sidebar__mark" />
          MoneyWatcher
        </div>

        {SECTIONS.map((entry) => (
          <button
            key={entry.id}
            type="button"
            className="sidebar__nav-item"
            aria-current={section === entry.id ? "page" : undefined}
            onClick={() => {
              if (entry.id === "transactions") setOnlyPending(false);
              setSection(entry.id);
            }}
          >
            {entry.label}
          </button>
        ))}

        <div className="sidebar__footer">
          Todos los datos se quedan en este equipo.
        </div>
      </nav>

      <main className="main">
        <header className="topbar">
          <h1 className="topbar__title">
            {SECTIONS.find((entry) => entry.id === section)?.label}
          </h1>

          <span className="topbar__spacer" />

          {section === "dashboard" || section === "transactions" ? (
            <select
              className="select"
              value={periodId}
              onChange={(event) => setPeriodId(event.target.value as PeriodId)}
            >
              {PERIODS.map((id) => (
                <option key={id} value={id}>
                  {buildPeriod(id).label}
                </option>
              ))}
            </select>
          ) : null}

          <button
            type="button"
            className="button button--primary"
            onClick={() => setImporting(true)}
          >
            Importar extracto
          </button>
        </header>

        <div className="content stack">
          {lastImport ? (
            <div className="banner">
              <span>
                {lastImport.imported} movimientos importados de{" "}
                <strong>{lastImport.import.sourceName}</strong>
                {lastImport.duplicates > 0 ? `, ${lastImport.duplicates} duplicados omitidos` : ""}
                {lastImport.skipped > 0 ? `, ${lastImport.skipped} líneas ilegibles` : ""}.{" "}
                {lastImport.categorization.categorized > 0
                  ? `${lastImport.categorization.categorized} categorizados por reglas.`
                  : ""}
              </span>
              <span className="topbar__spacer" />
              <button
                type="button"
                className="button button--ghost"
                onClick={() => setLastImport(null)}
              >
                Cerrar
              </button>
            </div>
          ) : null}

          {accounts.error ? (
            <div className="banner banner--error">
              No se pudieron cargar las cuentas: {accounts.error}
            </div>
          ) : null}

          {section === "dashboard" ? (
            <DashboardView
              filter={filter}
              currency={currency}
              onReviewPending={() => openTransactions(true)}
            />
          ) : null}

          {section === "transactions" ? (
            <TransactionsView
              accounts={accounts.data ?? []}
              categories={categories.data ?? []}
              baseFilter={filter}
              initialUncategorized={onlyPending}
            />
          ) : null}

          {section === "rules" ? (
            <RulesView
              categories={categories.data ?? []}
              assistantEnabled={assistant.data?.enabled ?? false}
            />
          ) : null}

          {section === "settings" ? (
            <SettingsView
              accounts={accounts.data ?? []}
              onAccountsChanged={() => {
                accounts.reload();
                assistant.reload();
              }}
            />
          ) : null}
        </div>
      </main>

      {importing ? (
        <ImportDialog
          accounts={accounts.data ?? []}
          onClose={() => setImporting(false)}
          onImported={handleImported}
        />
      ) : null}
    </div>
  );
}
