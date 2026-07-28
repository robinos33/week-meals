import { describe, expect, it } from "vitest";
import { aisleEmoji, groupByAisle, OTHER_AISLE_LABEL } from "./aisles";

const LABELS = {
  legumes: "Légumes",
  cremerie: "Crèmerie",
  surgeles: "Surgelés",
};

const ORDER = ["surgeles", "legumes", "cremerie"];

function item(name: string, category: string | null) {
  return { name, category };
}

describe("groupByAisle", () => {
  it("suit l'ordre de visite du magasin, pas celui de la liste", () => {
    const groups = groupByAisle(
      [item("courgette", "legumes"), item("glace", "surgeles"), item("lait", "cremerie")],
      ORDER,
      LABELS,
    );
    expect(groups.map((group) => group.label)).toEqual(["Surgelés", "Légumes", "Crèmerie"]);
    expect(groups[0].items.map((i) => i.name)).toEqual(["glace"]);
  });

  it("ne crée pas de section pour un rayon sans article", () => {
    const groups = groupByAisle([item("courgette", "legumes")], ORDER, LABELS);
    expect(groups).toHaveLength(1);
    expect(groups[0].slug).toBe("legumes");
  });

  it("garde l'ordre d'origine à l'intérieur d'un rayon", () => {
    const groups = groupByAisle(
      [item("courgette", "legumes"), item("tomate", "legumes")],
      ORDER,
      LABELS,
    );
    expect(groups[0].items.map((i) => i.name)).toEqual(["courgette", "tomate"]);
  });

  it("range en « Autres », en fin de parcours, ce qui n'a pas de rayon", () => {
    const groups = groupByAisle(
      [item("piles", null), item("courgette", "legumes")],
      ORDER,
      LABELS,
    );
    expect(groups.map((group) => group.label)).toEqual(["Légumes", OTHER_AISLE_LABEL]);
    expect(groups[1].items.map((i) => i.name)).toEqual(["piles"]);
  });

  it("n'égare pas un article rangé dans un rayon absent du magasin", () => {
    // Un ancien rayon (« epicerie », scindé depuis) ou un rayon plus récent que
    // le magasin : l'article reste visible plutôt que de disparaître.
    const groups = groupByAisle([item("farine", "epicerie")], ORDER, LABELS);
    expect(groups).toHaveLength(1);
    expect(groups[0].label).toBe(OTHER_AISLE_LABEL);
  });

  it("affiche le slug d'un rayon dont le libellé manque", () => {
    const groups = groupByAisle([item("croquettes", "animaux")], ["animaux"], LABELS);
    expect(groups[0].label).toBe("animaux");
  });
});

describe("aisleEmoji", () => {
  it("donne un repère au rayon connu, une puce sinon", () => {
    expect(aisleEmoji("legumes")).toBe("🥕");
    expect(aisleEmoji(null)).toBe("•");
    expect(aisleEmoji("inconnu")).toBe("•");
  });
});
