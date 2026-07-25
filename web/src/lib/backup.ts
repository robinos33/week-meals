/**
 * Sauvegarde des recettes du foyer côté appareil (#63).
 *
 * Le bouton « Sauvegarder » des Paramètres récupère toutes les recettes et les
 * télécharge en JSON, pour ne rien perdre en cas de souci serveur ou migration.
 * Un import avec dédoublonnage suivra (2e ticket) : le format porte donc une
 * `version` et un `format` explicites pour rester relisible.
 *
 * Les photos ne sont conservées que sous forme de **lien web absolu**
 * (http/https, p. ex. R2). Une photo stockée sur le volume du serveur n'a qu'un
 * chemin relatif (`/api/recipes/photos/…`) : ce lien ne survivrait pas à la
 * perte du serveur — précisément ce contre quoi la sauvegarde protège —, on le
 * laisse donc de côté (`null`).
 */

import type { Ingredient, RecipeView } from "../api/recipes";

export const BACKUP_FORMAT = "weekmeals-recipes";
export const BACKUP_VERSION = 1;

/** Une recette telle qu'inscrite dans la sauvegarde (sans identifiants ni méta). */
export interface BackupRecipe {
  title: string;
  prep_time_min: number | null;
  cook_time_min: number | null;
  /** Lien web absolu de la photo, ou `null` (voir en-tête du module). */
  photo: string | null;
  ingredients: Ingredient[];
  steps: string[];
}

/** Fichier de sauvegarde complet. */
export interface RecipeBackup {
  format: typeof BACKUP_FORMAT;
  version: number;
  exported_at: string;
  recipe_count: number;
  recipes: BackupRecipe[];
}

/**
 * Ne garde la photo que si c'est un lien web absolu (http/https), sinon `null`.
 * Partagé avec l'import (#73) pour appliquer la même règle des deux côtés.
 */
export function webPhoto(photo: string | null): string | null {
  return photo && /^https?:\/\//i.test(photo) ? photo : null;
}

/** Construit l'objet de sauvegarde à partir des recettes du foyer. */
export function buildBackup(recipes: RecipeView[], now: Date = new Date()): RecipeBackup {
  return {
    format: BACKUP_FORMAT,
    version: BACKUP_VERSION,
    exported_at: now.toISOString(),
    recipe_count: recipes.length,
    recipes: recipes.map((r) => ({
      title: r.title,
      prep_time_min: r.prep_time_min,
      cook_time_min: r.cook_time_min,
      photo: webPhoto(r.photo),
      ingredients: r.ingredients,
      steps: r.steps,
    })),
  };
}

/** Nom de fichier daté, p. ex. `week-meals-recettes-2026-07-25.json`. */
export function backupFilename(now: Date = new Date()): string {
  return `week-meals-recettes-${now.toISOString().slice(0, 10)}.json`;
}

/** Déclenche le téléchargement du JSON sur l'appareil (via un `<a download>`). */
export function downloadBackup(backup: RecipeBackup, filename: string): void {
  const blob = new Blob([JSON.stringify(backup, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
