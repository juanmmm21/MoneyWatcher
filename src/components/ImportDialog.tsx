import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import { formatDate, formatMoney, isNegative } from "../lib/money";
import type { Account, ImportResult, StatementPreview } from "../types/ipc";

interface ImportDialogProps {
  accounts: Account[];
  onClose: () => void;
  onImported: (result: ImportResult) => void;
}

const PREVIEW_ROWS = 12;

/**
 * Importación en dos pasos: primero se enseña lo que la app ha entendido del
 * fichero y solo después se guarda. Un extracto mal interpretado que entra
 * directo en la base de datos es mucho más caro de arreglar que una revisión.
 */
export function ImportDialog({ accounts, onClose, onImported }: ImportDialogProps) {
  const [accountId, setAccountId] = useState<number | null>(accounts[0]?.id ?? null);
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<StatementPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const chooseFile = useCallback(async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Extractos", extensions: ["csv", "txt", "tsv"] }],
      });
      if (typeof selected !== "string") return;

      setBusy(true);
      setPath(selected);
      setPreview(await api.previewStatement(selected));
    } catch (importError) {
      setPreview(null);
      setError(errorMessage(importError));
    } finally {
      setBusy(false);
    }
  }, []);

  const confirmImport = useCallback(async () => {
    if (accountId === null || path === null) return;
    setBusy(true);
    setError(null);
    try {
      onImported(await api.importStatement(accountId, path));
    } catch (importError) {
      setError(errorMessage(importError));
    } finally {
      setBusy(false);
    }
  }, [accountId, path, onImported]);

  const currency = accounts.find((account) => account.id === accountId)?.currency ?? "EUR";

  return (
    <div className="dialog-backdrop" role="dialog" aria-modal="true">
      <div className="dialog">
        <div className="dialog__header">
          <h2 className="dialog__title">Importar extracto</h2>
          <span className="small muted">Los datos no salen de este equipo.</span>
        </div>

        <div className="dialog__body stack">
          {accounts.length === 0 ? (
            <div className="banner banner--warning">
              Crea primero una cuenta en Ajustes para poder importar movimientos.
            </div>
          ) : null}

          <div className="row row--wrap">
            <label className="field">
              Cuenta de destino
              <select
                className="select"
                value={accountId ?? ""}
                onChange={(event) => setAccountId(Number(event.target.value))}
              >
                {accounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.bank} · {account.name}
                  </option>
                ))}
              </select>
            </label>

            <button
              type="button"
              className="button"
              onClick={() => void chooseFile()}
              disabled={busy || accounts.length === 0}
            >
              {path ? "Elegir otro fichero" : "Elegir fichero CSV…"}
            </button>
          </div>

          {path ? <div className="small muted">{path}</div> : null}
          {error ? <div className="banner banner--error">{error}</div> : null}

          {preview ? (
            <>
              <div className="row row--wrap small muted">
                <span className="badge">Separador «{preview.delimiter}»</span>
                <span className="badge">{preview.rows.length} movimientos</span>
                {preview.skipped.length > 0 ? (
                  <span className="badge">{preview.skipped.length} líneas descartadas</span>
                ) : null}
                <span className="badge">Columnas: {preview.headers.join(" · ")}</span>
              </div>

              <table className="table">
                <thead>
                  <tr>
                    <th>Fecha</th>
                    <th>Concepto</th>
                    <th className="table__amount">Importe</th>
                  </tr>
                </thead>
                <tbody>
                  {preview.rows.slice(0, PREVIEW_ROWS).map((row) => (
                    <tr key={row.line}>
                      <td className="tabular small">{formatDate(row.bookedOn)}</td>
                      <td>{row.description}</td>
                      <td
                        className={`table__amount tabular ${
                          isNegative(row.amount) ? "amount--expense" : "amount--income"
                        }`}
                      >
                        {formatMoney(row.amount, { currency })}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              {preview.rows.length > PREVIEW_ROWS ? (
                <div className="small muted">
                  … y {preview.rows.length - PREVIEW_ROWS} movimientos más.
                </div>
              ) : null}

              {preview.skipped.length > 0 ? (
                <details className="small muted">
                  <summary>Líneas que no se han podido leer</summary>
                  <ul>
                    {preview.skipped.slice(0, 10).map((skipped) => (
                      <li key={skipped.line}>
                        Línea {skipped.line}: {skipped.reason}
                      </li>
                    ))}
                  </ul>
                </details>
              ) : null}
            </>
          ) : null}
        </div>

        <div className="dialog__footer">
          <button type="button" className="button button--ghost" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="button"
            className="button button--primary"
            onClick={() => void confirmImport()}
            disabled={busy || !preview || accountId === null}
          >
            {busy ? "Importando…" : "Importar movimientos"}
          </button>
        </div>
      </div>
    </div>
  );
}
