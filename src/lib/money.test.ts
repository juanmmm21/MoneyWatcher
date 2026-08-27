import { describe, expect, it } from "vitest";

import { formatBps, formatDate, formatMoney, formatMonth, isNegative } from "./money";

describe("formatMoney", () => {
  it("agrupa miles y mantiene los dos decimales exactos", () => {
    expect(formatMoney("1234567.89")).toBe("1.234.567,89 €");
    expect(formatMoney("0.05")).toBe("0,05 €");
  });

  it("usa el menos tipográfico para los importes negativos", () => {
    expect(formatMoney("-45.12")).toBe("−45,12 €");
    expect(formatMoney("45.12", { showSign: true })).toBe("+45,12 €");
  });

  it("respeta la divisa de la cuenta", () => {
    expect(formatMoney("10.00", { currency: "USD" })).toBe("10,00 $");
    expect(formatMoney("10.00", { currency: "CHF" })).toBe("10,00 CHF");
  });

  it("no pierde céntimos con importes que un float redondearía", () => {
    // 0.1 + 0.2 en coma flotante da 0.30000000000000004; aquí el núcleo ya ha
    // hecho la suma en enteros y el formateo respeta la cadena exacta.
    expect(formatMoney("0.30")).toBe("0,30 €");
    expect(formatMoney("9007199254740993.01")).toBe("9.007.199.254.740.993,01 €");
  });
});

describe("helpers de presentación", () => {
  it("detecta el signo sin convertir a número", () => {
    expect(isNegative("-0.01")).toBe(true);
    expect(isNegative("0.00")).toBe(false);
  });

  it("formatea puntos básicos y fechas", () => {
    expect(formatBps(9621)).toBe("96,2 %");
    expect(formatMonth("2026-03")).toBe("mar 2026");
    expect(formatDate("2026-03-14")).toBe("14 mar 2026");
  });
});
