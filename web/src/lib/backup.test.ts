import { describe, expect, it } from "vitest";
import type { RecipeView } from "../api/recipes";
import { BACKUP_FORMAT, BACKUP_VERSION, backupFilename, buildBackup } from "./backup";

function recipe(overrides: Partial<RecipeView> = {}): RecipeView {
  return {
    id: "r1",
    household_id: "h1",
    title: "Ratatouille",
    photo: "https://cdn.example.test/rata.jpg",
    prep_time_min: 25,
    cook_time_min: 45,
    ingredients: [{ name: "courgette", amount: 600, unit: "g" }],
    steps: ["Émincer.", "Laisser mijoter."],
    servings: 4,
    cooked_count: 3,
    ...overrides,
  };
}

describe("buildBackup", () => {
  it("en-tête avec format, version, horodatage et compte", () => {
    const now = new Date("2026-07-25T10:30:00.000Z");
    const backup = buildBackup([recipe(), recipe({ id: "r2" })], now);

    expect(backup.format).toBe(BACKUP_FORMAT);
    expect(backup.version).toBe(BACKUP_VERSION);
    expect(backup.exported_at).toBe("2026-07-25T10:30:00.000Z");
    expect(backup.recipe_count).toBe(2);
    expect(backup.recipes).toHaveLength(2);
  });

  it("ne conserve que les champs métier (pas d'id ni de cooked_count)", () => {
    const backup = buildBackup([recipe()]);
    expect(backup.recipes[0]).toEqual({
      title: "Ratatouille",
      prep_time_min: 25,
      cook_time_min: 45,
      photo: "https://cdn.example.test/rata.jpg",
      ingredients: [{ name: "courgette", amount: 600, unit: "g" }],
      steps: ["Émincer.", "Laisser mijoter."],
      servings: 4,
    });
  });

  it("garde les photos à lien web absolu", () => {
    expect(buildBackup([recipe({ photo: "http://x.test/a.jpg" })]).recipes[0].photo).toBe(
      "http://x.test/a.jpg",
    );
  });

  it("écarte les photos sans lien web (chemin volume relatif) et l'absence de photo", () => {
    expect(
      buildBackup([recipe({ photo: "/api/recipes/photos/abc.png" })]).recipes[0].photo,
    ).toBeNull();
    expect(buildBackup([recipe({ photo: null })]).recipes[0].photo).toBeNull();
  });

  it("sauvegarde vide sur foyer sans recette", () => {
    const backup = buildBackup([]);
    expect(backup.recipe_count).toBe(0);
    expect(backup.recipes).toEqual([]);
  });
});

describe("backupFilename", () => {
  it("nom de fichier daté", () => {
    expect(backupFilename(new Date("2026-07-25T10:30:00.000Z"))).toBe(
      "week-meals-recettes-2026-07-25.json",
    );
  });
});
