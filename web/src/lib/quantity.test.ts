import { describe, expect, it } from "vitest";
import { formatAmount, parseAmount } from "./quantity";

describe("formatAmount", () => {
  it("keeps integers as-is", () => {
    expect(formatAmount(2)).toBe("2");
    expect(formatAmount(12)).toBe("12");
  });

  it("renders common fractions as glyphs", () => {
    expect(formatAmount(0.5)).toBe("½");
    expect(formatAmount(0.25)).toBe("¼");
    expect(formatAmount(0.75)).toBe("¾");
    expect(formatAmount(1 / 3)).toBe("⅓");
    expect(formatAmount(2 / 3)).toBe("⅔");
  });

  it("renders mixed numbers", () => {
    expect(formatAmount(1.5)).toBe("1½");
    expect(formatAmount(2.25)).toBe("2¼");
  });

  it("falls back to a short decimal for odd values", () => {
    expect(formatAmount(1.2)).toBe("1.2");
    expect(formatAmount(0.4)).toBe("0.4");
  });
});

describe("parseAmount", () => {
  it("reads plain and decimal numbers, with comma", () => {
    expect(parseAmount("2")).toBe(2);
    expect(parseAmount("0.75")).toBe(0.75);
    expect(parseAmount("0,5")).toBe(0.5);
  });

  it("reads fractions and mixed numbers", () => {
    expect(parseAmount("1/2")).toBe(0.5);
    expect(parseAmount("1 1/2")).toBe(1.5);
    expect(parseAmount("3 / 4")).toBe(0.75);
  });

  it("rejects non-positive or unparsable input", () => {
    expect(parseAmount("")).toBeNull();
    expect(parseAmount("0")).toBeNull();
    expect(parseAmount("-1")).toBeNull();
    expect(parseAmount("abc")).toBeNull();
    expect(parseAmount("1/0")).toBeNull();
  });
});
