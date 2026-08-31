import { useCallback, useState } from "react";

import { AccountDialog } from "../components/AccountDialog";
import { useAsync } from "../hooks/useAsync";
import { api, errorMessage } from "../lib/ipc";
import { formatMoney } from "../lib/money";
import type { Account, AppInfo, AssistantStatus, ImportRecord } from "../types/ipc";
import { DEFAULT_OLLAMA_ENDPOINT, DEFAULT_OLLAMA_MODEL } from "../lib/constants";

/** Tamaño legible: la base pasa de KB a MB en cuanto se importan unos meses. */
function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

interface SettingsViewProps {
  accounts: Account[];
  /** Cambia cuando los datos se modifican fuera de esta vista (una importación). */
  dataVersion: number;
  onAccountsChanged: () => void;
  /** Avisa al resto de la app de que esta vista ha cambiado los datos. */
  onDataChanged: () => void;
}

/** Cuentas, asistente y dónde viven los datos. */
export function SettingsView({
  accounts,
  dataVersion,
  onAccountsChanged,
  onDataChanged,
}: SettingsViewProps) {
  const info = useAsync<AppInfo>(() => api.appInfo(), [dataVersion]);
  const imports = useAsync<ImportRecord[]>(() => api.listImports(10), [dataVersion]);
  const assistant = useAsync<AssistantStatus>(() => api.assistantStatus(), []);

  const [creatingAccount, setCreatingAccount] = useState(false);
  // Deshacer una importación borra sus movimientos y no hay vuelta atrás, así
  // que el botón pide una segunda pulsación en lugar de disparar al primer clic.
  const [confirmingRevert, setConfirmingRevert] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [endpoint, setEndpoint] = useState(DEFAULT_OLLAMA_ENDPOINT);
  const [model, setModel] = useState(DEFAULT_OLLAMA_MODEL);

  const toggleAssistant = useCallback(
    async (enabled: boolean) => {
      setError(null);
      try {
        await api.setAssistantSettings(
          enabled ? { kind: "ollama", endpoint, model } : { kind: "disabled" },
        );
        assistant.reload();
      } catch (toggleError) {
        setError(errorMessage(toggleError));
      }
    },
    [assistant, endpoint, model],
  );

  const archiveAccount = useCallback(
    async (account: Account) => {
      setError(null);
      try {
        await api.setAccountArchived(account.id, !account.archived);
        onAccountsChanged();
      } catch (archiveError) {
        setError(errorMessage(archiveError));
      }
    },
    [onAccountsChanged],
  );

  const revertImport = useCallback(
    async (importId: number) => {
      setError(null);
      setConfirmingRevert(null);
      try {
        await api.revertImport(importId);
        imports.reload();
        info.reload();
        onAccountsChanged();
        onDataChanged();
      } catch (revertError) {
        setError(errorMessage(revertError));
      }
    },
    [imports, info, onAccountsChanged, onDataChanged],
  );

  const status = assistant.data;

  return (
    <div className="stack">
      {error ? <div className="banner banner--error">{error}</div> : null}

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Cuentas</h3>
          <button type="button" className="button" onClick={() => setCreatingAccount(true)}>
            + Nueva cuenta
          </button>
        </div>
        <div className="card__body" style={{ padding: 0 }}>
          <table className="table">
            <thead>
              <tr>
                <th>Banco</th>
                <th>Cuenta</th>
                <th className="table__amount">Saldo</th>
                <th style={{ width: 120 }} />
              </tr>
            </thead>
            <tbody>
              {accounts.map((account) => (
                <tr key={account.id}>
                  <td>{account.bank}</td>
                  <td>
                    {account.name}
                    {account.archived ? <span className="badge"> archivada</span> : null}
                  </td>
                  <td className="table__amount tabular">
                    {formatMoney(account.balance, { currency: account.currency })}
                  </td>
                  <td>
                    <button
                      type="button"
                      className="button button--ghost"
                      onClick={() => void archiveAccount(account)}
                    >
                      {account.archived ? "Recuperar" : "Archivar"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {accounts.length === 0 ? (
            <div className="empty">
              <span className="empty__title">Sin cuentas todavía</span>
              <span className="small">Crea una cuenta por cada banco con el que trabajes.</span>
            </div>
          ) : null}
        </div>
      </div>

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Asistente de categorización</h3>
          {status ? (
            <span className="badge">
              <span
                className="badge__dot"
                style={{
                  background: status.enabled
                    ? status.reachable
                      ? "var(--income)"
                      : "var(--warning)"
                    : "var(--text-faint)",
                }}
              />
              {status.enabled ? (status.reachable ? "conectado" : "sin respuesta") : "desactivado"}
            </span>
          ) : null}
        </div>
        <div className="card__body stack">
          <p className="small muted" style={{ margin: 0 }}>
            Opcional. MoneyWatcher categoriza con reglas sin necesitar ningún modelo; el asistente
            solo se ocupa de lo que las reglas no saben resolver. Apunta por defecto a Ollama en
            tu equipo, de forma que los movimientos no salen de aquí.
          </p>

          <div className="row row--wrap">
            <label className="field" style={{ flex: 1, minWidth: 220 }}>
              Endpoint
              <input
                className="input"
                value={endpoint}
                onChange={(event) => setEndpoint(event.target.value)}
              />
            </label>
            <label className="field" style={{ width: 200 }}>
              Modelo
              <input
                className="input"
                value={model}
                onChange={(event) => setModel(event.target.value)}
              />
            </label>
          </div>

          {status?.leavesTheMachine ? (
            <div className="banner banner--warning">
              Ese endpoint no es local: los conceptos de tus movimientos saldrían de este equipo.
            </div>
          ) : null}

          {status?.error ? <div className="small muted">{status.error}</div> : null}

          {status && status.availableModels.length > 0 ? (
            <div className="small muted">
              Modelos disponibles: {status.availableModels.join(", ")}
            </div>
          ) : null}

          <div className="row">
            <button
              type="button"
              className="button"
              onClick={() => void toggleAssistant(!(status?.enabled ?? false))}
            >
              {status?.enabled ? "Desactivar asistente" : "Activar asistente"}
            </button>
            <button type="button" className="button button--ghost" onClick={() => assistant.reload()}>
              Comprobar conexión
            </button>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Importaciones recientes</h3>
        </div>
        <div className="card__body" style={{ padding: 0 }}>
          <table className="table">
            <thead>
              <tr>
                <th>Fichero</th>
                <th className="table__amount">Importados</th>
                <th className="table__amount">Duplicados</th>
                <th style={{ width: 200 }} />
              </tr>
            </thead>
            <tbody>
              {(imports.data ?? []).map((record) => (
                <tr key={record.id}>
                  <td>
                    <div>{record.sourceName}</div>
                    <div className="small muted">
                      {new Date(record.importedAt).toLocaleString()}
                    </div>
                  </td>
                  <td className="table__amount tabular">{record.importedCount}</td>
                  <td className="table__amount tabular">{record.duplicateCount}</td>
                  <td>
                    {confirmingRevert === record.id ? (
                      <div className="row">
                        <button
                          type="button"
                          className="button button--ghost button--danger"
                          onClick={() => void revertImport(record.id)}
                        >
                          Borrar {record.importedCount}
                        </button>
                        <button
                          type="button"
                          className="button button--ghost"
                          onClick={() => setConfirmingRevert(null)}
                        >
                          Cancelar
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        className="button button--ghost button--danger"
                        onClick={() => setConfirmingRevert(record.id)}
                      >
                        Deshacer
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {(imports.data?.length ?? 0) === 0 ? (
            <div className="empty">
              <span className="small">Aún no has importado ningún extracto.</span>
            </div>
          ) : null}
        </div>
      </div>

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Tus datos</h3>
        </div>
        <div className="card__body stack">
          <p className="small muted" style={{ margin: 0 }}>
            Todo vive en un fichero SQLite de tu equipo. Para hacer una copia de seguridad,
            cierra MoneyWatcher antes de copiarlo: con la app abierta, los últimos movimientos
            todavía están en los ficheros <code>-wal</code> y <code>-shm</code> que lo acompañan.
            Borrarlo (los tres) deja la app como recién instalada.
          </p>
          {info.data ? (
            <ul className="small" style={{ margin: 0, paddingLeft: 18 }}>
              <li>
                Base de datos: <code>{info.data.databasePath}</code>
              </li>
              <li className="tabular">
                {formatSize(info.data.databaseSizeBytes)} · {info.data.transactions} movimientos ·{" "}
                {info.data.accounts} cuentas
              </li>
              <li>Versión del esquema: {info.data.schemaVersion}</li>
            </ul>
          ) : null}
        </div>
      </div>

      {creatingAccount ? (
        <AccountDialog
          onClose={() => setCreatingAccount(false)}
          onCreated={() => {
            setCreatingAccount(false);
            onAccountsChanged();
            info.reload();
          }}
        />
      ) : null}
    </div>
  );
}
