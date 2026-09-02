import { useCallback, useState } from "react";

import { AccountDialog } from "../components/AccountDialog";
import { useAsync } from "../hooks/useAsync";
import { api, errorMessage } from "../lib/ipc";
import { formatDate, formatMoney } from "../lib/money";
import type {
  Account,
  AppInfo,
  AssistantStatus,
  ImportRecord,
  TransferSettings,
} from "../types/ipc";
import { DEFAULT_OLLAMA_ENDPOINT, DEFAULT_OLLAMA_MODEL } from "../lib/constants";

/** Tamaño legible: la base pasa de KB a MB en cuanto se importan unos meses. */
function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

interface SettingsViewProps {
  /** Cambia cuando los datos se modifican fuera de esta vista (una importación). */
  dataVersion: number;
  onAccountsChanged: () => void;
  /** Avisa al resto de la app de que esta vista ha cambiado los datos. */
  onDataChanged: () => void;
}

/** Cuentas, asistente y dónde viven los datos. */
export function SettingsView({
  dataVersion,
  onAccountsChanged,
  onDataChanged,
}: SettingsViewProps) {
  // Esta es la pantalla de administración de cuentas, así que aquí sí entran
  // las archivadas: en cualquier otra lista estorban, pero si no aparecen aquí
  // no hay forma de recuperarlas.
  const allAccounts = useAsync<Account[]>(() => api.listAccounts(true), [dataVersion]);
  const info = useAsync<AppInfo>(() => api.appInfo(), [dataVersion]);
  const imports = useAsync<ImportRecord[]>(() => api.listImports(10), [dataVersion]);
  const assistant = useAsync<AssistantStatus>(() => api.assistantStatus(), []);
  const transfers = useAsync<TransferSettings>(() => api.transferSettings(), [dataVersion]);

  const [creatingAccount, setCreatingAccount] = useState(false);
  // Deshacer una importación borra sus movimientos y no hay vuelta atrás, así
  // que el botón pide una segunda pulsación en lugar de disparar al primer clic.
  const [confirmingRevert, setConfirmingRevert] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  // `null` hasta que llega la configuración guardada: si los campos arrancaran
  // con los valores por defecto, activar el asistente pisaría el modelo que el
  // usuario hubiera elegido antes.
  const [endpointDraft, setEndpointDraft] = useState<string | null>(null);
  const [modelDraft, setModelDraft] = useState<string | null>(null);

  const status = assistant.data;
  const stored = status?.provider;
  const storedEndpoint = stored?.kind === "ollama" ? stored.endpoint : DEFAULT_OLLAMA_ENDPOINT;
  const storedModel = stored?.kind === "ollama" ? stored.model : DEFAULT_OLLAMA_MODEL;
  const endpoint = endpointDraft ?? storedEndpoint;
  const model = modelDraft ?? storedModel;

  const toggleAssistant = useCallback(
    async (enabled: boolean) => {
      setError(null);
      try {
        await api.setAssistantSettings(
          enabled ? { kind: "ollama", endpoint, model } : { kind: "disabled" },
        );
        setEndpointDraft(null);
        setModelDraft(null);
        assistant.reload();
        // La vista de reglas decide con esto si puede pedir sugerencias.
        onDataChanged();
      } catch (toggleError) {
        setError(errorMessage(toggleError));
      }
    },
    [assistant, endpoint, model, onDataChanged],
  );

  // El resultado de la última detección: cuántos pares nuevos han salido. Sin
  // este aviso, pulsar «Buscar traspasos» y no encontrar nada parecería que el
  // botón no ha hecho nada.
  const [transferNotice, setTransferNotice] = useState<string | null>(null);

  const toggleTransferDetection = useCallback(
    async (enabled: boolean) => {
      setError(null);
      setTransferNotice(null);
      try {
        const detection = await api.setTransferDetection(enabled);
        if (enabled) {
          setTransferNotice(
            detection.linked > 0
              ? `${detection.linked} traspaso(s) nuevos encontrados.`
              : "Ningún traspaso nuevo entre tus cuentas.",
          );
        }
        transfers.reload();
        // Los widgets del dashboard suman de otra forma a partir de ahora.
        onDataChanged();
      } catch (toggleError) {
        setError(errorMessage(toggleError));
      }
    },
    [transfers, onDataChanged],
  );

  const detectTransfers = useCallback(async () => {
    setError(null);
    setTransferNotice(null);
    try {
      const detection = await api.detectTransfers();
      setTransferNotice(
        detection.linked > 0
          ? `${detection.linked} traspaso(s) nuevos encontrados.`
          : "Ningún traspaso nuevo entre tus cuentas.",
      );
      transfers.reload();
      onDataChanged();
    } catch (detectError) {
      setError(errorMessage(detectError));
    }
  }, [transfers, onDataChanged]);

  const dismissTransfer = useCallback(
    async (linkId: number, dismissed: boolean) => {
      setError(null);
      setTransferNotice(null);
      try {
        await api.setTransferDismissed(linkId, dismissed);
        transfers.reload();
        onDataChanged();
      } catch (dismissError) {
        setError(errorMessage(dismissError));
      }
    },
    [transfers, onDataChanged],
  );

  const archiveAccount = useCallback(
    async (account: Account) => {
      setError(null);
      try {
        await api.setAccountArchived(account.id, !account.archived);
        allAccounts.reload();
        onAccountsChanged();
      } catch (archiveError) {
        setError(errorMessage(archiveError));
      }
    },
    [allAccounts, onAccountsChanged],
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
                <th className="table__amount">Movimientos</th>
                <th style={{ width: 120 }} />
              </tr>
            </thead>
            <tbody>
              {(allAccounts.data ?? []).map((account) => (
                <tr key={account.id}>
                  <td>{account.bank}</td>
                  <td>
                    {account.name}
                    {account.archived ? <span className="badge"> archivada</span> : null}
                  </td>
                  <td className="table__amount tabular">{account.transactions}</td>
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
          {(allAccounts.data?.length ?? 0) === 0 ? (
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
                onChange={(event) => setEndpointDraft(event.target.value)}
              />
            </label>
            <label className="field" style={{ width: 240 }}>
              Modelo
              {(status?.availableModels.length ?? 0) > 0 ? (
                <select
                  className="select"
                  value={model}
                  onChange={(event) => setModelDraft(event.target.value)}
                >
                  {/* El modelo guardado puede ya no estar descargado: se deja
                      elegible para no perderlo en silencio al abrir Ajustes. */}
                  {!status?.availableModels.includes(model) ? (
                    <option value={model}>{model} (no descargado)</option>
                  ) : null}
                  {status?.availableModels.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  className="input"
                  value={model}
                  onChange={(event) => setModelDraft(event.target.value)}
                />
              )}
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
          <h3 className="card__title">Traspasos entre cuentas</h3>
          {transfers.data ? (
            <span className="badge">
              <span
                className="badge__dot"
                style={{
                  background: transfers.data.enabled ? "var(--income)" : "var(--text-faint)",
                }}
              />
              {transfers.data.enabled
                ? `${transfers.data.active} reconocidos`
                : "desactivado"}
            </span>
          ) : null}
        </div>
        <div className="card__body stack">
          <p className="small muted" style={{ margin: 0 }}>
            Mover 300 € de una cuenta tuya a otra no es gastar 300 € y ganar otros 300: es el
            mismo dinero cambiado de sitio. Con esto activado, los pares que la app reconoce
            (mismo importe, signo contrario, cuentas distintas y como mucho{" "}
            {transfers.data?.windowDays ?? 2} días de diferencia) dejan de contar en los widgets.
            Siguen apareciendo en la lista de movimientos, marcados como traspaso.
          </p>

          {transferNotice ? <div className="banner">{transferNotice}</div> : null}

          <div className="row">
            <button
              type="button"
              className="button"
              onClick={() => void toggleTransferDetection(!(transfers.data?.enabled ?? false))}
            >
              {transfers.data?.enabled ? "Desactivar" : "Activar"}
            </button>
            <button
              type="button"
              className="button button--ghost"
              onClick={() => void detectTransfers()}
              disabled={!transfers.data?.enabled}
            >
              Buscar traspasos ahora
            </button>
          </div>

          {(transfers.data?.links.length ?? 0) > 0 ? (
            <>
              <p className="small muted" style={{ margin: 0 }}>
                Revisa los pares: dos importes iguales de signo contrario pueden ser una
                coincidencia. Lo que descartes aquí vuelve a contar como gasto e ingreso y no
                se vuelve a proponer.
              </p>
              <table className="table">
                <thead>
                  <tr>
                    <th style={{ width: 110 }}>Fecha</th>
                    <th>De → a</th>
                    <th className="table__amount" style={{ width: 120 }}>
                      Importe
                    </th>
                    <th style={{ width: 150 }} />
                  </tr>
                </thead>
                <tbody>
                  {(transfers.data?.links ?? []).map((link) => (
                    <tr key={link.id} style={{ opacity: link.dismissed ? 0.55 : 1 }}>
                      <td className="tabular small">
                        {formatDate(link.bookedOn)}
                        {link.dayGap > 0 ? (
                          <div className="small muted">+{link.dayGap} d</div>
                        ) : null}
                      </td>
                      <td>
                        <div className="small">
                          {link.fromAccount} → {link.toAccount}
                        </div>
                        <div className="small muted">
                          {link.outgoingDescription} · {link.incomingDescription}
                        </div>
                      </td>
                      <td className="table__amount tabular">{formatMoney(link.amount)}</td>
                      <td>
                        <button
                          type="button"
                          className="button button--ghost"
                          onClick={() => void dismissTransfer(link.id, !link.dismissed)}
                        >
                          {link.dismissed ? "Sí es un traspaso" : "No es un traspaso"}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          ) : null}
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
            allAccounts.reload();
            onAccountsChanged();
            info.reload();
          }}
        />
      ) : null}
    </div>
  );
}
