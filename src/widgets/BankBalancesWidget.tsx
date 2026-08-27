import { formatMoney } from "../lib/money";
import type { BankSummary } from "../types/ipc";

import { WidgetEmpty, WidgetFrame } from "./WidgetFrame";

interface BankBalancesWidgetProps {
  title: string;
  banks: BankSummary[];
  currency: string;
}

/**
 * Saldo y flujo por entidad. Es la vista que refleja cómo está organizado el
 * dinero de verdad: una lista de ingresos y otra de gastos por banco.
 */
export function BankBalancesWidget({ title, banks, currency }: BankBalancesWidgetProps) {
  if (banks.length === 0) {
    return (
      <WidgetFrame title={title}>
        <WidgetEmpty message="Añade una cuenta para ver su saldo aquí." />
      </WidgetFrame>
    );
  }

  return (
    <WidgetFrame title={title}>
      <table className="table">
        <thead>
          <tr>
            <th>Banco</th>
            <th className="table__amount">Ingresos</th>
            <th className="table__amount">Gastos</th>
            <th className="table__amount">Saldo</th>
          </tr>
        </thead>
        <tbody>
          {banks.map((bank) => (
            <tr key={bank.bank}>
              <td>
                <div>{bank.bank}</div>
                <div className="small muted">
                  {bank.accounts} {bank.accounts === 1 ? "cuenta" : "cuentas"}
                </div>
              </td>
              <td className="table__amount tabular amount--income">
                {formatMoney(bank.income, { currency })}
              </td>
              <td className="table__amount tabular amount--expense">
                {formatMoney(bank.expense, { currency })}
              </td>
              <td className="table__amount tabular">{formatMoney(bank.balance, { currency })}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </WidgetFrame>
  );
}
