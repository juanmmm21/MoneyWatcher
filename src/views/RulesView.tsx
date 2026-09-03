import { useCallback, useState } from "react";

import { useAsync } from "../hooks/useAsync";
import { api, errorMessage } from "../lib/ipc";
import { formatMoney, isNegative } from "../lib/money";
import type { Category, Rule, RuleMatcher, Suggestion, SuggestionBatch } from "../types/ipc";

/** Lo que queda por revisar; lo que hace falta para saber si el asistente ha mirado el histórico entero. */
type Backlog = Pick<
  SuggestionBatch,
  "pendingTransactions" | "pendingGroups" | "remainingGroups" | "brandsUsed" | "brandLookupsFailed"
>;

interface RulesViewProps {
  categories: Category[];
  assistantEnabled: boolean;
  /** Cambia cuando los datos se modifican fuera de esta vista (una importación). */
  dataVersion: number;
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
export function RulesView({ categories, assistantEnabled, dataVersion }: RulesViewProps) {
  const rules = useAsync<Rule[]>(() => api.listRules(), [dataVersion]);
  const [pattern, setPattern] = useState("");
  const [matcher, setMatcher] = useState<RuleMatcher>("contains");
  const [categoryId, setCategoryId] = useState<number | null>(categories[0]?.id ?? null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<Suggestion[] | null>(null);
  // Comercios ya preguntados en esta sesión: se devuelven al núcleo para que la
  // siguiente tanda siga por donde iba en vez de repetir la primera.
  const [askedPatterns, setAskedPatterns] = useState<string[]>([]);
  const [backlog, setBacklog] = useState<Backlog | null>(null);
  const [busy, setBusy] = useState(false);

  const confidentSuggestions = (suggestions ?? []).filter((item) => !item.needsReview);
  const doubtfulSuggestions = (suggestions ?? []).filter((item) => item.needsReview);

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

  /** `continuing` sigue por el comercio donde se quedó; si no, empieza de cero. */
  const askAssistant = useCallback(
    async (continuing: boolean) => {
      setError(null);
      setNotice(null);
      setBusy(true);
      const skip = continuing ? askedPatterns : [];
      if (!continuing) setSuggestions(null);

      try {
        const batch = await api.suggestCategories(skip);
        setSuggestions((current) =>
          continuing ? [...(current ?? []), ...batch.suggestions] : batch.suggestions,
        );
        setAskedPatterns([...skip, ...batch.askedPatterns]);
        setBacklog({
          pendingTransactions: batch.pendingTransactions,
          pendingGroups: batch.pendingGroups,
          remainingGroups: batch.remainingGroups,
          brandsUsed: batch.brandsUsed,
          brandLookupsFailed: batch.brandLookupsFailed,
        });
      } catch (assistantError) {
        setError(errorMessage(assistantError));
      } finally {
        setBusy(false);
      }
    },
    [askedPatterns],
  );

  const acceptSuggestion = useCallback(async (suggestion: Suggestion) => {
    setError(null);
    try {
      // De una propuesta dudosa no se aprende ninguna regla aunque el usuario
      // la acepte: si el modelo no reconoció el comercio, generalizarla
      // arrastraría el mismo error a todos los movimientos parecidos.
      const result = await api.correctTransactionCategory(
        suggestion.transactionId,
        suggestion.categoryId,
        !suggestion.needsReview,
      );
      // El corregido más los que arrastró la regla aprendida: es la cifra que
      // dice si aceptar ha servido de algo.
      setNotice(`${1 + result.applied.categorized} movimiento(s) categorizados.`);
      setSuggestions((current) =>
        current?.filter((item) => item.transactionId !== suggestion.transactionId) ?? null,
      );
      setBacklog((current) =>
        current
          ? {
              ...current,
              pendingTransactions: Math.max(
                0,
                current.pendingTransactions - 1 - result.applied.categorized,
              ),
            }
          : current,
      );
    } catch (acceptError) {
      setError(errorMessage(acceptError));
    }
  }, []);

  /// Aceptar en bloque solo lo que el modelo tenía claro; lo dudoso se queda.
  const acceptConfident = useCallback(async () => {
    const confident = (suggestions ?? []).filter((item) => !item.needsReview);
    if (confident.length === 0) return;

    setError(null);
    setBusy(true);
    const accepted: number[] = [];
    let categorized = 0;
    try {
      for (const suggestion of confident) {
        const result = await api.correctTransactionCategory(
          suggestion.transactionId,
          suggestion.categoryId,
          true,
        );
        categorized += 1 + result.applied.categorized;
        accepted.push(suggestion.transactionId);
      }
      setNotice(
        `${accepted.length} propuesta(s) aceptadas: ${categorized} movimiento(s) categorizados.`,
      );
      setBacklog((current) =>
        current
          ? {
              ...current,
              pendingTransactions: Math.max(0, current.pendingTransactions - categorized),
            }
          : current,
      );
    } catch (acceptError) {
      setError(errorMessage(acceptError));
    } finally {
      // Se quitan las que sí entraron, aunque el lote se cortara a medias.
      setSuggestions((current) =>
        current?.filter((item) => !accepted.includes(item.transactionId)) ?? null,
      );
      setBusy(false);
    }
  }, [suggestions]);

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
          <div className="row">
            {backlog && backlog.remainingGroups > 0 ? (
              <button
                type="button"
                className="button"
                onClick={() => void askAssistant(true)}
                disabled={!assistantEnabled || busy}
              >
                Seguir con los {Math.min(backlog.remainingGroups, 25)} siguientes
              </button>
            ) : null}
            <button
              type="button"
              className="button"
              onClick={() => void askAssistant(false)}
              disabled={!assistantEnabled || busy}
            >
              {suggestions === null ? "Proponer categorías" : "Empezar de nuevo"}
            </button>
          </div>
        </div>
        <div className="card__body stack">
          {!assistantEnabled ? (
            <span className="small muted">
              El asistente está desactivado. Se activa en Ajustes, apuntando a un modelo local.
            </span>
          ) : null}

          {backlog ? (
            <span className="small muted">
              {backlog.pendingTransactions} movimiento(s) sin categoría en{" "}
              {backlog.pendingGroups} comercio(s) distinto(s). Se pregunta por los comercios
              que más movimientos arrastran, y aceptar una propuesta ordena el grupo entero.{" "}
              {backlog.remainingGroups > 0
                ? `Quedan ${backlog.remainingGroups} comercio(s) por preguntar.`
                : "No queda ningún comercio por preguntar."}
            </span>
          ) : null}

          {backlog && backlog.brandsUsed > 0 ? (
            <span className="small muted">
              {backlog.brandsUsed} marca(s) identificadas en internet antes de preguntar al
              modelo.
            </span>
          ) : null}

          {backlog && backlog.brandLookupsFailed > 0 ? (
            <div className="banner banner--warning">
              {backlog.brandLookupsFailed} consulta(s) de marca no respondieron. Las propuestas
              salen igual, pero de esos comercios el modelo ha ido a ciegas.
            </div>
          ) : null}

          {suggestions?.length === 0 ? (
            <span className="small muted">
              El modelo no ha propuesto nada en esta tanda: o no queda nada pendiente, o no ha
              sabido clasificar lo que se le ha enseñado.
            </span>
          ) : null}

          {confidentSuggestions.length > 0 ? (
            <>
              <div className="row">
                <span className="small muted" style={{ flex: 1 }}>
                  El modelo reconoció el comercio en {confidentSuggestions.length} de ellas, que
                  cubren {confidentSuggestions.reduce((total, item) => total + item.transactionCount, 0)}{" "}
                  movimiento(s). Aceptar una enseña su regla y ordena el grupo entero.
                </span>
                <button
                  type="button"
                  className="button"
                  onClick={() => void acceptConfident()}
                  disabled={busy}
                >
                  Aceptar las {confidentSuggestions.length} seguras
                </button>
              </div>

              {confidentSuggestions.map((suggestion) => (
                <div key={suggestion.transactionId} className="row" style={{ gap: 12 }}>
                  <span style={{ flex: 1, minWidth: 0 }}>{suggestion.description}</span>
                  {suggestion.transactionCount > 1 ? (
                    <span className="badge">{suggestion.transactionCount} movimientos</span>
                  ) : null}
                  <span
                    className="tabular"
                    style={{ color: isNegative(suggestion.amount) ? "var(--expense)" : "var(--income)" }}
                  >
                    {formatMoney(suggestion.amount)}
                  </span>
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
            </>
          ) : null}

          {doubtfulSuggestions.length > 0 ? (
            <>
              <div className="banner banner--warning">
                El modelo no reconoció {doubtfulSuggestions.length === 1 ? "este comercio" : "estos comercios"}
                {" "}y ha respondido por salir del paso. Revísalo: aceptar aquí cambia solo ese
                movimiento y no crea ninguna regla.
              </div>

              {doubtfulSuggestions.map((suggestion) => (
                <div key={suggestion.transactionId} className="row" style={{ gap: 12 }}>
                  <span style={{ flex: 1, minWidth: 0 }}>{suggestion.description}</span>
                  {suggestion.transactionCount > 1 ? (
                    <span className="badge">{suggestion.transactionCount} movimientos</span>
                  ) : null}
                  <span
                    className="tabular"
                    style={{ color: isNegative(suggestion.amount) ? "var(--expense)" : "var(--income)" }}
                  >
                    {formatMoney(suggestion.amount)}
                  </span>
                  <span className="badge">{suggestion.categoryName}</span>
                  <span className="small muted tabular">{suggestion.confidence} %</span>
                  <button
                    type="button"
                    className="button button--ghost"
                    onClick={() => void acceptSuggestion(suggestion)}
                  >
                    Aceptar de todas formas
                  </button>
                </div>
              ))}
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
