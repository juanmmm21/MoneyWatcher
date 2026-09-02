import { describe, expect, it } from "vitest";

import { loadOrSeedWidgets, type WidgetStore } from "./dashboard";
import type { NewWidget, Widget } from "../types/ipc";

const DEFAULTS: NewWidget[] = [
  { kind: "totals", title: "Resumen", config: {}, placement: { x: 0, y: 0, w: 6, h: 6 } },
  { kind: "bank_balances", title: "Bancos", config: {}, placement: { x: 6, y: 0, w: 6, h: 6 } },
];

/** Almacén en memoria que imita a los comandos de widgets del núcleo. */
function fakeStore(): WidgetStore & { created: number } {
  const widgets: Widget[] = [];
  const store = {
    created: 0,
    list: async () => [...widgets],
    create: async (widget: NewWidget) => {
      const created: Widget = { id: widgets.length + 1, ...widget };
      widgets.push(created);
      store.created += 1;
      return created;
    },
  };
  return store;
}

describe("loadOrSeedWidgets", () => {
  it("siembra el layout por defecto cuando no hay widgets guardados", async () => {
    const store = fakeStore();
    const widgets = await loadOrSeedWidgets(store, DEFAULTS);

    expect(widgets).toHaveLength(DEFAULTS.length);
    expect(store.created).toBe(DEFAULTS.length);
  });

  it("devuelve los guardados sin crear ninguno más", async () => {
    const store = fakeStore();
    await loadOrSeedWidgets(store, DEFAULTS);
    const again = await loadOrSeedWidgets(store, DEFAULTS);

    expect(again).toHaveLength(DEFAULTS.length);
    expect(store.created).toBe(DEFAULTS.length);
  });

  it("dos cargas simultáneas no duplican el dashboard", async () => {
    const store = fakeStore();
    const [first, second] = await Promise.all([
      loadOrSeedWidgets(store, DEFAULTS),
      loadOrSeedWidgets(store, DEFAULTS),
    ]);

    expect(store.created).toBe(DEFAULTS.length);
    expect(first).toHaveLength(DEFAULTS.length);
    expect(second).toHaveLength(DEFAULTS.length);
  });

  it("un fallo no bloquea las cargas siguientes", async () => {
    const broken: WidgetStore = {
      list: async () => {
        throw new Error("sin conexión con el núcleo");
      },
      create: async () => {
        throw new Error("sin conexión con el núcleo");
      },
    };

    await expect(loadOrSeedWidgets(broken, DEFAULTS)).rejects.toThrow("sin conexión con el núcleo");

    const store = fakeStore();
    await expect(loadOrSeedWidgets(store, DEFAULTS)).resolves.toHaveLength(DEFAULTS.length);
  });
});
