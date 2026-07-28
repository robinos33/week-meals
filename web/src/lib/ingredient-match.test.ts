import { describe, expect, it } from "vitest";
import { canonicalKey, coreKey, matchScore, rankMatches, similarity } from "./ingredient-match";

describe("canonicalKey", () => {
  it("déplie casse, accents et ligatures", () => {
    expect(canonicalKey("  Œufs  ")).toBe("oeuf");
    expect(canonicalKey("Échalotes")).toBe("echalote");
  });

  it("retire ponctuation et mots vides", () => {
    expect(canonicalKey("gousse d'ail")).toBe("gousse ail");
    expect(canonicalKey("pommes de terre")).toBe("pomme terre");
  });

  it("rapproche singulier et pluriel", () => {
    expect(canonicalKey("tomates")).toBe(canonicalKey("tomate"));
    expect(canonicalKey("choux")).toBe(canonicalKey("chou"));
  });

  it("laisse les mots courts intacts", () => {
    expect(canonicalKey("riz")).toBe("riz");
    expect(canonicalKey("ail")).toBe("ail");
  });
});

describe("coreKey", () => {
  it("retire les qualificatifs de courses", () => {
    expect(coreKey("grosses courgettes")).toBe("courgette");
    expect(coreKey("Œufs bio")).toBe("oeuf");
    expect(coreKey("tomates bien mûres")).toBe("tomate");
  });

  it("garde les couleurs, qui nomment une variété", () => {
    expect(coreKey("poivron rouge")).toBe("poivron rouge");
    expect(coreKey("poivrons jaunes")).not.toBe(coreKey("poivron"));
    expect(coreKey("oignon rouge")).not.toBe(coreKey("oignon"));
  });

  it("garde les mots qui changent le produit", () => {
    expect(coreKey("lait de coco")).toBe("lait coco");
    expect(coreKey("abricots secs")).toBe("abricot sec");
    expect(coreKey("café moulu")).toBe("cafe moulu");
  });
});

describe("similarity", () => {
  it("vaut 1 pour deux clés identiques", () => {
    expect(similarity("courgette", "courgette")).toBe(1);
  });

  it("décroît avec les différences", () => {
    expect(similarity("echalotte", "echalote")).toBeGreaterThan(0.8);
    expect(similarity("poire", "poireau")).toBeLessThan(0.8);
  });
});

describe("matchScore", () => {
  it("classe le nom exact devant le préfixe", () => {
    const exact = matchScore("courgette", "courgette");
    const prefix = matchScore("cour", "courgette");
    expect(exact).toBe(1);
    expect(prefix).not.toBeNull();
    expect(exact!).toBeGreaterThan(prefix!);
  });

  it("suit la frappe en cours", () => {
    expect(matchScore("cou", "courgette")).not.toBeNull();
    expect(matchScore("oe", "œuf")).not.toBeNull();
  });

  it("reconnaît un synonyme sans changer le libellé", () => {
    expect(matchScore("oeuf", "œuf", ["oeuf"])).toBe(1);
  });

  it("ignore les qualificatifs qui ne changent pas le produit", () => {
    expect(matchScore("grosses courgettes bio", "courgette")).toBeGreaterThan(0.8);
  });

  it("ne rabat pas une couleur sur le produit générique", () => {
    // Saisie complète : « poivron rouge » n'est pas « poivron ».
    expect(matchScore("poivron rouge", "poivron")).toBeNull();
    expect(matchScore("poivron rouge", "poivron rouge")).toBe(1);
    // En cours de frappe, le générique reste proposé — on ne sait pas encore
    // ce qui suit.
    expect(matchScore("poivron r", "poivron")).not.toBeNull();
  });

  it("tolère une faute de frappe", () => {
    expect(matchScore("echalotte", "échalote")).not.toBeNull();
  });

  it("propose un composé à partir de son premier mot", () => {
    // « pomme » doit faire remonter « pomme de terre »…
    expect(matchScore("pomme", "pomme de terre")).not.toBeNull();
    // … mais « terre » aussi, tapé seul.
    expect(matchScore("terre", "pomme de terre")).not.toBeNull();
  });

  it("écarte ce qui n'a rien à voir", () => {
    expect(matchScore("courgette", "saumon")).toBeNull();
    expect(matchScore("   ", "courgette")).toBeNull();
  });
});

describe("rankMatches", () => {
  const candidates = [
    { name: "pomme de terre" },
    { name: "pomme" },
    { name: "compote de pommes" },
    { name: "saumon" },
  ];

  it("classe du plus pertinent au moins pertinent", () => {
    expect(rankMatches("pomme", candidates).map((c) => c.name)).toEqual([
      "pomme",
      "pomme de terre",
      "compote de pommes",
    ]);
  });

  it("fait remonter les candidats bonifiés", () => {
    // Ce qui est déjà dans la liste passe devant, à pertinence comparable.
    const ranked = rankMatches("pomme", candidates, (candidate) =>
      candidate.name === "pomme de terre" ? 0.5 : 0,
    );
    expect(ranked[0].name).toBe("pomme de terre");
  });

  it("ne renvoie rien sur une saisie vide", () => {
    expect(rankMatches("", candidates)).toEqual([]);
  });
});
