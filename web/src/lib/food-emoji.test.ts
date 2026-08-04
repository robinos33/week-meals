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
    expect(foodEmoji("pâte brisée")).toBe("🥧");
    expect(foodEmoji("pâte feuilletée")).toBe("🥧");
    expect(foodEmoji("pâtes")).toBe("🍝");
    // « pâte » seul ne dit pas de quoi il s'agit : pas d'emoji au hasard.
    expect(foodEmoji("pâte à tartiner")).toBeNull();
  });

  it("donne un repère aux fromages et au frais", () => {
    expect(foodEmoji("feta")).toBe("🧀");
    expect(foodEmoji("mozzarella")).toBe("🧀");
    expect(foodEmoji("fromage blanc")).toBe("🧀");
    expect(foodEmoji("crème fraîche")).toBe("🥛");
    // La crème glacée reste une glace, pas de la crème.
    expect(foodEmoji("crème glacée")).toBe("🍦");
  });

  it("reconnaît les produits vus en liste réelle", () => {
    expect(foodEmoji("galette bretonne")).toBe("🥞");
    expect(foodEmoji("houmous")).toBe("🫘");
    expect(foodEmoji("coppa")).toBe("🍖");
    expect(foodEmoji("limande")).toBe("🐟");
    expect(foodEmoji("pâte à pizza rectangulaire")).toBe("🍕");
  });

  it("gère un nom avec quantité et complément", () => {
    expect(foodEmoji("500 g de fromage râpé")).toBe("🧀");
  });

  it("donne aussi un repère aux produits hors alimentaires", () => {
    expect(foodEmoji("papier toilette")).toBe("🧻");
    expect(foodEmoji("piles")).toBe("🔋");
    expect(foodEmoji("croquettes pour chat")).toBe("🐾");
  });

  it("reconnaît les produits d'une liste réelle tombés en Autres", () => {
    expect(foodEmoji("burrata")).toBe("🧀");
    expect(foodEmoji("fajitas")).toBe("🌯");
    expect(foodEmoji("biscottes")).toBe("🍞");
    expect(foodEmoji("Cassegrain")).toBe("🥫");
  });

  it("renvoie null pour un aliment inconnu", () => {
    expect(foodEmoji("quinoa")).toBeNull();
    expect(foodEmoji("")).toBeNull();
  });
});
