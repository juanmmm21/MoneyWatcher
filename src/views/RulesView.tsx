import { useCallback, useState } from "react";

import { useAsync } from "../hooks/useAsync";
import { api, errorMessage } from "../lib/ipc";
import type { Category, Rule, RuleMatcher, Suggestion } from "../types/ipc";

interface RulesViewProps {
  categories: Category[];
  assistantEnabled: boolean;
}

const MATCHERS: { value: RuleMatcher; label: string }[] = [
  { value: "contains", label: "contiene" },
  { value: "starts_with", label: "empieza por" },
  { value: "ends_with", label: "termina en" },
  { value: "equals", label: "es exactamente" },
];

const ORIGIN_LABEL: Record<Rule["origin"], string> = {
  user: "manual",
  learned: "aprendida",
  assistant: "asistente",
};

/**
 * Reglas de categorización: lo que hace que la app ordene sola. El asistente
 * de IA vive aquí también, pero solo propone; nada se aplica sin aceptar.
 */
export function RulesView({ categories, assistantEnabled }: RulesViewProps) {
  const rules = useAsync<Rule[]>(() => api.listRules(), []);
  const [pattern, setPattern] = useState("");
  const [matcher, setMatcher] = useState<RuleMatcher>("contains");
  const [categoryId, setCategoryId] = useState<number | null>(categories[0]?.id ?? null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<Suggestion[] | null>(null);
  const [busy, setBusy] = useState(false);

  const createRule = useCallback(async () => {
    if (pattern.trim() === "" || categoryId === null) return;
    setError(null);
    try {
      await api.createRule({
        matcher,
        pattern: pattern.trim(),
        accountId: null,
        direction: null,
        minAmount: null,
        maxAmount: null,
        categoryId,
        priority: 100,
        origin: "user",
      });
      setPattern("");
      rules.reload();
      const summary = await api.runRules();
      setNotice(`Regla creada. ${summary.categorized} movimiento(s) categorizados.`);
    } catch (createError) {
      setError(errorMessage(createError));
    }
  }, [matcher, pattern, categoryId, rules]);

  const removeRule = useCallback(
    async (ruleId: number) => {
      setError(null);
      try {
        await api.deleteRule(ruleId);
        rules.reload();
      } catch (deleteError) {
        setError(errorMessage(deleteError));
      }
    },
    [rules],
  );

  const runRules = useCallback(async () => {
    setError(null);
    setBusy(true);
    try {
      const summary = await api.runRules();
      setNotice(
        `${summary.categorized} movimiento(s) categorizados, ${summary.pending} pendientes.`,
      );
    } catch (runError) {
      setError(errorMessage(runError));
    } finally {
      setBusy(false);
    }
  }, []);

  const askAssistant = useCallback(async () => {
    setError(null);
    setBusy(true);
    setSuggestions(null);
    try {
      setSuggestions(await api.suggestCategories());
    } catch (assistantError) {
      setError(errorMessage(assistantError));
    } finally {
      setBusy(false);
    }
  }, []);

  const acceptSuggestion = useCallback(async (suggestion: Suggestion) => {
    setError(null);
    try {
      await api.correctTransactionCategory(suggestion.transactionId, suggestion.categoryId, true);
      setSuggestions((current) =>
        current?.filter((item) => item.transactionId !== suggestion.transactionId) ?? null,
      );
    } catch (acceptError) {
      setError(errorMessage(acceptError));
    }
  }, []);

  const categoryName = (id: number) =>
    categories.find((category) => category.id === id)?.name ?? "—";

  return (
    <div className="stack">
      {error ? <div className="banner banner--error">{error}</div> : null}
      {notice ? <div className="banner">{notice}</div> : null}

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Nueva regla</h3>
          <button
            type="button"
            className="button"
            onClick={() => void runRules()}
            disabled={busy}
          >
            Aplicar reglas ahora
          </button>
        </div>
        <div className="card__body row row--wrap">
          <span className="small muted">Si el concepto</span>
          <select
            className="select"
            value={matcher}
            onChange={(event) => setMatcher(event.target.value as RuleMatcher)}
          >
            {MATCHERS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <input
            className="input"
            style={{ minWidth: 200 }}
            placeholder="mercadona"
            value={pattern}
            onChange={(event) => setPattern(event.target.value)}
          />
          <span className="small muted">entonces la categoría es</span>
          <select
            className="select"
            value={categoryId ?? ""}
            onChange={(event) => setCategoryId(Number(event.target.value))}
          >
            {categories.map((category) => (
              <option key={category.id} value={category.id}>
                {category.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="button button--primary"
            onClick={() => void createRule()}
            disabled={pattern.trim() === ""}
          >
            Crear
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Reglas ({rules.data?.length ?? 0})</h3>
        </div>
        <div className="card__body" style={{ padding: 0 }}>
          <table className="table">
            <thead>
              <tr>
                <th>Patrón</th>
                <th style={{ width: 170 }}>Categoría</th>
                <th style={{ width: 110 }}>Origen</th>
                <th style={{ width: 90 }} className="table__amount">
                  Aciertos
                </th>
                <th style={{ width: 60 }} />
              </tr>
            </thead>
            <tbody>
              {(rules.data ?? []).map((rule) => (
                <tr key={rule.id}>
                  <td>
                    <span className="small muted">
                      {MATCHERS.find((option) => option.value === rule.matcher)?.label}{" "}
                    </span>
                    <strong>{rule.pattern}</strong>
                  </td>
                  <td>{categoryName(rule.categoryId)}</td>
                  <td>
                    <span className="badge">{ORIGIN_LABEL[rule.origin]}</span>
                  </td>
                  <td className="table__amount tabular">{rule.hits}</td>
                  <td>
                    <button
                      type="button"
                      className="button button--ghost button--danger"
                      onClick={() => void removeRule(rule.id)}
                    >
                      Borrar
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {(rules.data?.length ?? 0) === 0 ? (
            <div className="empty">
              <span className="empty__title">Todavía no hay reglas</span>
              <span className="small">
                Corrige la categoría de un movimiento y la app aprenderá la primera sola.
              </span>
            </div>
          ) : null}
        </div>
      </div>

      <div className="card">
        <div className="card__header">
          <h3 className="card__title">Asistente</h3>
          <button
            type="button"
            className="button"
            onClick={() => void askAssistant()}
            disabled={!assistantEnabled || busy}
          >
            Proponer categorías
          </button>
        </div>
        <div className="card__body stack">
          {!assistantEnabled ? (
            <span className="small muted">
              El asistente está desactivado. Se activa en Ajustes, apuntando a un modelo local.
            </span>
          ) : null}

          {suggestions?.length === 0 ? (
            <span className="small muted">
              El modelo no ha propuesto nada nuevo: no queda nada pendiente o no ha sabido
              clasificarlo.
            </span>
          ) : null}

          {(suggestions ?? []).map((suggestion) => (
            <div key={suggestion.transactionId} className="row" style={{ gap: 12 }}>
              <span style={{ flex: 1, minWidth: 0 }}>{suggestion.description}</span>
              <span className="badge">{suggestion.categoryName}</span>
              <span className="small muted tabular">{suggestion.confidence} %</span>
              <button
                type="button"
                className="button"
                onClick={() => void acceptSuggestion(suggestion)}
              >
                Aceptar
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
