import type { NewWidget, Widget } from "../types/ipc";

/**
 * Cola de siembra del dashboard.
 *
 * Cargar y sembrar los widgets no puede solaparse consigo mismo: si dos
 * montajes de la vista comprueban a la vez que no hay ninguno —React ejecuta
 * los efectos dos veces en desarrollo— los dos siembran y el dashboard aparece
 * con cada widget repetido. Encadenando las llamadas, la segunda vuelve a mirar
 * la base ya sembrada y se limita a devolver lo que hay.
 */
let queue: Promise<unknown> = Promise.resolve();

export interface WidgetStore {
  list: () => Promise<Widget[]>;
  create: (widget: NewWidget) => Promise<Widget>;
}

/**
 * Devuelve los widgets guardados y, solo si no hay ninguno, crea los del
 * layout por defecto para no dejar al usuario delante de un lienzo vacío.
 */
export function loadOrSeedWidgets(store: WidgetStore, defaults: NewWidget[]): Promise<Widget[]> {
  const next = queue.catch(() => undefined).then(() => run(store, defaults));
  // La cola nunca arrastra un rechazo: un fallo puntual no puede dejar
  // bloqueadas todas las cargas posteriores.
  queue = next.catch(() => undefined);
  return next;
}

async function run(store: WidgetStore, defaults: NewWidget[]): Promise<Widget[]> {
  const stored = await store.list();
  if (stored.length > 0) {
    return stored;
  }

  const created: Widget[] = [];
  for (const widget of defaults) {
    created.push(await store.create(widget));
  }
  return created;
}
