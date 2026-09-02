import { useCallback, useMemo, useState } from "react";

import { useAsync } from "../hooks/useAsync";
import { api, errorMessage } from "../lib/ipc";
import { formatDate, formatMoney, isNegative } from "../lib/money";
import type {
  Account,
  Category,
  Direction,
  TransactionFilter,
  TransactionPage,
} from "../types/ipc";

const PAGE_SIZE = 100;

interface TransactionsViewProps {
  accounts: Account[];
  categories: Category[];
  baseFilter: TransactionFilter;
  /** Cambia cuando los datos se modifican fuera de esta vista (una importación). */
  dataVersion: number;
  /** Abre la vista directamente con el filtro de pendientes activado. */
  initialUncategorized?: boolean;
}

/**
 * Tabla de movimientos con las acciones del día a día: buscar, filtrar y
 * corregir categorías. Cada corrección puede enseñarle una regla a la app, que
 * es como el sistema mejora sin depender de ningún modelo.
 */
export function TransactionsView({
  accounts,
  categories,
  baseFilter,
  dataVersion,
  initialUncategorized = false,
}: TransactionsViewProps) {
  const [search, setSearch] = useState("");
  const [accountId, setAccountId] = useState<number | null>(null);
  const [direction, setDirection] = useState<Direction | null>(null);
  const [uncategorizedOnly, setUncategorizedOnly] = useState(initialUncategorized);
  const [learnFromCorrections, setLearnFromCorrections] = useState(true);
  const [page, setPage] = useState(0);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const filter: TransactionFilter = useMemo(
    () => ({
      ...baseFilter,
      accountIds: accountId ? [accountId] : baseFilter.accountIds,
      search: search.trim() === "" ? null : search.trim(),
      direction,
      uncategorizedOnly,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }),
    [baseFilter, accountId, search, direction, uncategorizedOnly, page],
  );

  const result = useAsync<TransactionPage>(
    () => api.listTransactions(filter),
    [JSON.stringify(filter), dataVersion],
  );

  const accountName = useCallback(
    (id: number) => {
      const account = accounts.find((candidate) => candidate.id === id);
      return account ? `${account.bank} · ${account.name}` : "—";
    },
    [accounts],
  );

  const assignCategory = useCallback(
    async (transactionId: number, categoryId: number | null) => {
      setActionError(null);
      setNotice(null);
      try {
        if (categoryId === null) {
          await api.setTransactionCategory(transactionId, null);
        } else {
          const correction = await api.correctTransactionCategory(
            transactionId,
            categoryId,
            learnFromCorrections,
          );
          if (correction.learnedRule) {
            const extra =
              correction.applied.categorized > 0
                ? ` y ha categorizado ${correction.applied.categorized} movimiento(s) más`
                : "";
            setNotice(
              `Regla aprendida: «${correction.learnedRule.pattern}»${extra}.`,
            );
          }
        }
        result.reload();
      } catch (error) {
        setActionError(errorMessage(error));
      }
    },
    [learnFromCorrections, result],
  );

  const total = result.data?.total ?? 0;
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="stack">
      <div className="row row--wrap">
        <input
          className="input"
          style={{ minWidth: 240 }}
          placeholder="Buscar concepto o contraparte…"
          value={search}
          onChange={(event) => {
            setSearch(event.target.value);
            setPage(0);
          }}
        />

        <select
          className="select"
          value={accountId ?? ""}
          onChange={(event) => {
            setAccountId(event.target.value === "" ? null : Number(event.target.value));
            setPage(0);
          }}
        >
          <option value="">Todas las cuentas</option>
          {accounts.map((account) => (
            <option key={account.id} value={account.id}>
              {account.bank} · {account.name}
            </option>
          ))}
        </select>

        <select
          className="select"
          value={direction ?? ""}
          onChange={(event) => {
            const value = event.target.value;
            setDirection(value === "" ? null : (value as Direction));
            setPage(0);
          }}
        >
          <option value="">Ingresos y gastos</option>
          <option value="income">Solo ingresos</option>
          <option value="expense">Solo gastos</option>
        </select>

        <label className="row small">
          <input
            type="checkbox"
            checked={uncategorizedOnly}
            onChange={(event) => {
              setUncategorizedOnly(event.target.checked);
              setPage(0);
            }}
          />
          Solo sin categorizar
        </label>

        <label className="row small" title="Crear una regla al corregir una categoría">
          <input
            type="checkbox"
            checked={learnFromCorrections}
            onChange={(event) => setLearnFromCorrections(event.target.checked)}
          />
          Aprender de mis correcciones
        </label>

        <span className="topbar__spacer" />
        <span className="small muted tabular">{total} movimientos</span>
      </div>

      {actionError ? <div className="banner banner--error">{actionError}</div> : null}
      {notice ? <div className="banner">{notice}</div> : null}
      {result.error ? <div className="banner banner--error">{result.error}</div> : null}

      <div className="card">
        <div className="card__body" style={{ padding: 0 }}>
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 110 }}>Fecha</th>
                <th>Concepto</th>
                <th style={{ width: 190 }}>Cuenta</th>
                <th style={{ width: 190 }}>Categoría</th>
                <th className="table__amount" style={{ width: 130 }}>
                  Importe
                </th>
              </tr>
            </thead>
            <tbody>
              {(result.data?.transactions ?? []).map((transaction) => (
                <tr key={transaction.id}>
                  <td className="tabular small">{formatDate(transaction.bookedOn)}</td>
                  <td>
                    <div>{transaction.description}</div>
                    {transaction.counterparty ? (
                      <div className="small muted">{transaction.counterparty}</div>
                    ) : null}
                  </td>
                  <td className="small muted">{accountName(transaction.accountId)}</td>
                  <td>
                    <select
                      className="select"
                      style={{ width: "100%" }}
                      value={transaction.categoryId ?? ""}
                      onChange={(event) =>
                        void assignCategory(
                          transaction.id,
                          event.target.value === "" ? null : Number(event.target.value),
                        )
                      }
                    >
                      <option value="">Sin categoría</option>
                      {categories.map((category) => (
                        <option key={category.id} value={category.id}>
                          {category.name}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td
                    className={`table__amount tabular ${
                      isNegative(transaction.amount) ? "amount--expense" : "amount--income"
                    }`}
                  >
                    {formatMoney(transaction.amount)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {!result.loading && (result.data?.transactions.length ?? 0) === 0 ? (
            <div className="empty">
              <span className="empty__title">Nada por aquí</span>
              <span className="small">
                Importa un extracto o cambia los filtros para ver movimientos.
              </span>
            </div>
          ) : null}
        </div>
      </div>

      {pageCount > 1 ? (
        <div className="row" style={{ justifyContent: "center" }}>
          <button
            type="button"
            className="button"
            disabled={page === 0}
            onClick={() => setPage((current) => current - 1)}
          >
            Anterior
          </button>
          <span className="small muted">
            Página {page + 1} de {pageCount}
          </span>
          <button
            type="button"
            className="button"
            disabled={page + 1 >= pageCount}
            onClick={() => setPage((current) => current + 1)}
          >
            Siguiente
          </button>
        </div>
      ) : null}
    </div>
  );
}
