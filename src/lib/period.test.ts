import { describe, expect, it } from "vitest";

import { buildPeriod, periodFilter } from "./period";

const REFERENCE = new Date("2026-03-14T10:00:00Z");

describe("buildPeriod", () => {
  it("empieza el mes en curso el día 1", () => {
    const period = buildPeriod("this-month", REFERENCE);
    expect(period.from).toBe("2026-03-01");
    expect(period.to).toBe("2026-03-14");
  });

  it("cuenta tres meses incluyendo el actual", () => {
    expect(buildPeriod("last-3-months", REFERENCE).from).toBe("2026-01-01");
  });

  it("retrocede doce meses cruzando el cambio de año", () => {
    expect(buildPeriod("last-12-months", REFERENCE).from).toBe("2025-04-01");
  });

  it("deja el periodo abierto cuando se piden todos los movimientos", () => {
    const period = buildPeriod("all", REFERENCE);
    expect(period.from).toBeNull();
    expect(period.to).toBeNull();
  });
});

describe("periodFilter", () => {
  it("solo incluye cuentas cuando hay alguna seleccionada", () => {
    const period = buildPeriod("this-year", REFERENCE);
    expect(periodFilter(period, []).accountIds).toEqual([]);
    expect(periodFilter(period, [3, 7]).accountIds).toEqual([3, 7]);
  });
});
