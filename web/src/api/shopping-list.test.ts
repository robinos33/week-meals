import { describe, expect, it } from "vitest";
import {
  adjustQuantity,
  formatQuantity,
  quantityStep,
  sameCombo,
  type ShoppingItem,
} from "./shopping-list";

/** Article minimal pour les tests (les champs inutilisés sont neutres). */
function item(over: Partial<ShoppingItem>): ShoppingItem {
  return {
    id: "x",
    name: "Pomme",
    amount: 3,
    unit: "piece",
    category: null,
    checked: false,
    generated: false,
    ...over,
  };
}

describe("sameCombo", () => {
  it("matche un combo identique (nom / quantité / unité)", () => {
    expect(sameCombo(item({}), { name: "Pomme", amount: 3, unit: "piece" })).toBe(true);
  });

  it("ignore la casse et les espaces du nom", () => {
    expect(sameCombo(item({ name: "Pomme" }), { name: "  pomme ", amount: 3, unit: "piece" })).toBe(
      true,
    );
  });

  it("distingue une quantité différente", () => {
    expect(sameCombo(item({ amount: 3 }), { name: "Pomme", amount: 5, unit: "piece" })).toBe(false);
  });

  it("distingue une unité différente", () => {
    expect(sameCombo(item({ unit: "piece" }), { name: "Pomme", amount: 3, unit: "kg" })).toBe(false);
  });

  it("distingue un nom différent", () => {
    expect(sameCombo(item({ name: "Poire" }), { name: "Pomme", amount: 3, unit: "piece" })).toBe(
      false,
    );
  });
});

describe("quantityStep", () => {
  it("dépend de l'unité", () => {
    expect(quantityStep("piece")).toBe(1);
    expect(quantityStep("g")).toBe(50);
    expect(quantityStep("ml")).toBe(50);
    expect(quantityStep("kg")).toBe(0.5);
    expect(quantityStep("l")).toBe(0.5);
  });
});

describe("adjustQuantity", () => {
  it("incrémente et décrémente d'un pas", () => {
    expect(adjustQuantity(3, "piece", 1)).toBe(4);
    expect(adjustQuantity(200, "g", -1)).toBe(150);
  });

  it("reste strictement positif (plancher = un pas)", () => {
    expect(adjustQuantity(1, "piece", -1)).toBe(1);
    expect(adjustQuantity(50, "g", -1)).toBe(50);
  });

  it("évite les bavures de flottant", () => {
    expect(adjustQuantity(1, "kg", 1)).toBe(1.5);
    expect(adjustQuantity(0.5, "l", 1)).toBe(1);
  });
});

describe("formatQuantity", () => {
  it("n'affiche pas de décimale superflue", () => {
    expect(formatQuantity(item({ amount: 250, unit: "g" }))).toBe("250 g");
  });

  it("garde deux décimales utiles", () => {
    expect(formatQuantity(item({ amount: 1.5, unit: "kg" }))).toBe("1.5 kg");
  });
});
