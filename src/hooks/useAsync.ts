import { useCallback, useEffect, useRef, useState } from "react";

import { errorMessage } from "../lib/ipc";

interface AsyncState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
}

/**
 * Ejecuta una llamada al núcleo y expone su estado.
 *
 * Descarta las respuestas de peticiones que ya no son la última: al cambiar el
 * filtro rápido, una consulta lenta anterior no debe pisar el resultado nuevo.
 */
export function useAsync<T>(
  operation: () => Promise<T>,
  dependencies: unknown[],
): AsyncState<T> & { reload: () => void } {
  const [state, setState] = useState<AsyncState<T>>({
    data: null,
    loading: true,
    error: null,
  });
  const requestId = useRef(0);
  const [reloadToken, setReloadToken] = useState(0);

  const reload = useCallback(() => setReloadToken((token) => token + 1), []);

  useEffect(() => {
    const currentRequest = ++requestId.current;
    let active = true;

    setState((previous) => ({ ...previous, loading: true, error: null }));

    operation()
      .then((data) => {
        if (!active || currentRequest !== requestId.current) return;
        setState({ data, loading: false, error: null });
      })
      .catch((error: unknown) => {
        if (!active || currentRequest !== requestId.current) return;
        setState({ data: null, loading: false, error: errorMessage(error) });
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...dependencies, reloadToken]);

  return { ...state, reload };
}
