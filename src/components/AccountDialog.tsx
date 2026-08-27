import { useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Account, AccountKind } from "../types/ipc";

interface AccountDialogProps {
  onClose: () => void;
  onCreated: (account: Account) => void;
}

const KINDS: { value: AccountKind; label: string }[] = [
  { value: "checking", label: "Cuenta corriente" },
  { value: "savings", label: "Ahorro" },
  { value: "credit", label: "Tarjeta de crédito" },
  { value: "cash", label: "Efectivo" },
  { value: "investment", label: "Inversión" },
];

/** Alta de cuenta: banco, nombre y saldo de partida. */
export function AccountDialog({ onClose, onCreated }: AccountDialogProps) {
  const [bank, setBank] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState<AccountKind>("checking");
  const [currency, setCurrency] = useState("EUR");
  const [openingBalance, setOpeningBalance] = useState("0.00");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const created = await api.createAccount({
        name: name.trim(),
        bank: bank.trim(),
        kind,
        currency,
        // El núcleo acepta la cadena tal cual y la valida al parsearla, así que
        // se envía sin tocar: nada de convertir a número por el camino.
        openingBalance: openingBalance.trim() === "" ? "0.00" : openingBalance.trim(),
      });
      onCreated(created);
    } catch (createError) {
      setError(errorMessage(createError));
    } finally {
      setBusy(false);
    }
  };

  const canSubmit = bank.trim() !== "" && name.trim() !== "" && !busy;

  return (
    <div className="dialog-backdrop" role="dialog" aria-modal="true">
      <div className="dialog" style={{ width: "min(480px, 100%)" }}>
        <div className="dialog__header">
          <h2 className="dialog__title">Nueva cuenta</h2>
        </div>

        <div className="dialog__body stack">
          <label className="field">
            Banco
            <input
              className="input"
              value={bank}
              placeholder="Santander"
              onChange={(event) => setBank(event.target.value)}
            />
          </label>

          <label className="field">
            Nombre de la cuenta
            <input
              className="input"
              value={name}
              placeholder="Nómina"
              onChange={(event) => setName(event.target.value)}
            />
          </label>

          <div className="row">
            <label className="field" style={{ flex: 1 }}>
              Tipo
              <select
                className="select"
                value={kind}
                onChange={(event) => setKind(event.target.value as AccountKind)}
              >
                {KINDS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field" style={{ width: 110 }}>
              Divisa
              <input
                className="input"
                value={currency}
                onChange={(event) => setCurrency(event.target.value.toUpperCase())}
                maxLength={3}
              />
            </label>

            <label className="field" style={{ width: 140 }}>
              Saldo inicial
              <input
                className="input tabular"
                value={openingBalance}
                onChange={(event) => setOpeningBalance(event.target.value)}
                placeholder="0,00"
              />
            </label>
          </div>

          {error ? <div className="banner banner--error">{error}</div> : null}
        </div>

        <div className="dialog__footer">
          <button type="button" className="button button--ghost" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="button"
            className="button button--primary"
            onClick={() => void submit()}
            disabled={!canSubmit}
          >
            Crear cuenta
          </button>
        </div>
      </div>
    </div>
  );
}
