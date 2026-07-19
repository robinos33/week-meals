import { describe, expect, it } from "vitest";
import { foodEmoji } from "./food-emoji";

describe("foodEmoji", () => {
  it("reconnaît un aliment simple", () => {
    expect(foodEmoji("pomme")).toBe("🍎");
    expect(foodEmoji("banane")).toBe("🍌");
  });

  it("ignore la casse et les accents", () => {
    expect(foodEmoji("PÊCHE")).toBe("🍑");
    expect(foodEmoji("Œuf")).toBe("🥚");
  });

  it("tolère le pluriel régulier", () => {
    expect(foodEmoji("pommes")).toBe("🍎");
    expect(foodEmoji("3 carottes")).toBe("🥕");
  });

  it("matche par mots entiers, pas par sous-chaîne", () => {
    // « poireau » contient « poire » mais ne doit pas devenir une poire.
    expect(foodEmoji("poireau")).toBe("🥬");
  });

  it("préfère le libellé composé au mot simple", () => {
    expect(foodEmoji("pomme de terre")).toBe("🥔");
    expect(foodEmoji("pommes de terre")).toBe("🥔");
    expect(foodEmoji("lait de coco")).toBe("🥥");
    expect(foodEmoji("patate douce")).toBe("🍠");
  });

  it("distingue le pois chiche du petit pois", () => {
    expect(foodEmoji("pois chiche")).toBe("🫘");
    expect(foodEmoji("petit pois")).toBe("🫛");
    expect(foodEmoji("gingembre")).toBe("🫚");
  });

  it("ne confond pas la pâte à tarte avec les pâtes", () => {
    expect(foodEmoji("pâte brisée")).toBeNull();
    expect(foodEmoji("pâtes")).toBe("🍝");
  });

  it("gère un nom avec quantité et complément", () => {
    expect(foodEmoji("500 g de fromage râpé")).toBe("🧀");
  });

  it("renvoie null pour un aliment inconnu", () => {
    expect(foodEmoji("quinoa")).toBeNull();
    expect(foodEmoji("")).toBeNull();
  });
});
