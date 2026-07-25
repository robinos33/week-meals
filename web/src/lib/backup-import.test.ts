import { describe, expect, it } from "vitest";
import type { RecipeInput } from "../api/recipes";
import {
  BackupImportError,
  importSummary,
  normalizeTitle,
  parseBackup,
  planImport,
} from "./backup-import";

const validRecipe: RecipeInput = {
  title: "Ratatouille",
  prep_time_min: 25,
  cook_time_min: 45,
  photo: "https://cdn.example.test/rata.jpg",
  ingredients: [{ name: "courgette", amount: 600, unit: "g" }],
  steps: ["Émincer.", "Laisser mijoter."],
};

function backupJson(recipes: unknown[], extra: Record<string, unknown> = {}): string {
  return JSON.stringify({
    format: "weekmeals-recipes",
    version: 1,
    exported_at: "2026-07-25T10:30:00.000Z",
    recipe_count: recipes.length,
    recipes,
    ...extra,
  });
}

describe("parseBackup — enveloppe", () => {
  it("rejette un JSON invalide", () => {
    expect(() => parseBackup("{pas du json")).toThrow(BackupImportError);
  });

  it("rejette un fichier d'un autre format", () => {
    expect(() => parseBackup(JSON.stringify({ format: "autre", recipes: [] }))).toThrow(
      /pas une sauvegarde Week Meals/,
    );
  });

  it("rejette une version plus récente", () => {
    expect(() => parseBackup(backupJson([], { version: 2 }))).toThrow(/version plus récente/);
  });

  it("rejette une sauvegarde sans tableau de recettes", () => {
    expect(() => parseBackup(JSON.stringify({ format: "weekmeals-recipes" }))).toThrow(
      /corrompue/,
    );
  });
});

describe("parseBackup — recettes", () => {
  it("nettoie et retient une recette valide", () => {
    const { recipes, invalidCount } = parseBackup(backupJson([validRecipe]));
    expect(invalidCount).toBe(0);
    expect(recipes).toEqual([validRecipe]);
  });

  it("écarte les recettes sans titre et les compte comme invalides", () => {
    const { recipes, invalidCount } = parseBackup(
      backupJson([validRecipe, { title: "  " }, { steps: [] }]),
    );
    expect(recipes).toHaveLength(1);
    expect(invalidCount).toBe(2);
  });

  it("filtre les ingrédients à nom vide ou unité inconnue", () => {
    const { recipes } = parseBackup(
      backupJson([
        {
          title: "Test",
          ingredients: [
            { name: "sel", amount: 5, unit: "g" },
            { name: "", amount: 1, unit: "g" },
            { name: "x", amount: 1, unit: "cuillère" },
          ],
          steps: [],
        },
      ]),
    );
    expect(recipes[0].ingredients).toEqual([{ name: "sel", amount: 5, unit: "g" }]);
  });

  it("écarte les photos sans lien web et coerce les temps négatifs en null", () => {
    const { recipes } = parseBackup(
      backupJson([
        { title: "T", photo: "/api/recipes/photos/a.png", prep_time_min: -3, steps: [] },
      ]),
    );
    expect(recipes[0].photo).toBeNull();
    expect(recipes[0].prep_time_min).toBeNull();
  });
});

describe("planImport — dédoublonnage", () => {
  const r = (title: string): RecipeInput => ({ ...validRecipe, title });

  it("écarte les recettes déjà présentes dans le foyer (titre normalisé)", () => {
    const plan = planImport([r("Ratatouille"), r("Tarte")], ["  ratatouille "]);
    expect(plan.toImport.map((x) => x.title)).toEqual(["Tarte"]);
    expect(plan.duplicateCount).toBe(1);
  });

  it("écarte les doublons internes au fichier", () => {
    const plan = planImport([r("Tarte"), r("tarte")], []);
    expect(plan.toImport).toHaveLength(1);
    expect(plan.duplicateCount).toBe(1);
  });
});

describe("normalizeTitle", () => {
  it("minuscules et espaces compactés", () => {
    expect(normalizeTitle("  Tarte   aux  Pommes ")).toBe("tarte aux pommes");
  });
});

describe("importSummary", () => {
  it("cas nominal avec doublons", () => {
    expect(
      importSummary({ imported: 3, duplicateCount: 2, invalidCount: 0, failed: 0 }),
    ).toBe("3 recettes importées, 2 déjà présentes.");
  });

  it("aucune nouveauté, tout en double", () => {
    expect(
      importSummary({ imported: 0, duplicateCount: 4, invalidCount: 0, failed: 0 }),
    ).toBe("Aucune nouvelle recette importée, 4 déjà présentes.");
  });

  it("signale les invalides et les échecs", () => {
    expect(
      importSummary({ imported: 1, duplicateCount: 0, invalidCount: 1, failed: 2 }),
    ).toBe("1 recette importée, 1 ignorée (illisible), 2 en échec.");
  });
});
