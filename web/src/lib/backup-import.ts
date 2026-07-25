/**
 * Import d'une sauvegarde de recettes (#73), pendant de la sauvegarde (#63).
 *
 * On relit un fichier `week-meals-recettes-*.json`, on valide l'enveloppe et
 * chaque recette, puis on **dédoublonne par titre normalisé** (contre les
 * recettes déjà présentes du foyer et à l'intérieur du fichier). Les recettes
 * retenues sont créées via l'endpoint existant `POST /recipes` — pas de nouvel
 * endpoint ni de migration.
 */

import type { Ingredient, RecipeInput, Unit } from "../api/recipes";
import { UNITS } from "../api/recipes";
import { BACKUP_FORMAT, BACKUP_VERSION, webPhoto } from "./backup";

/** Erreur d'import portant un message affichable tel quel à l'utilisateur. */
export class BackupImportError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackupImportError";
  }
}

/** Résultat du parsing : recettes valides et nettoyées + entrées écartées. */
export interface ParsedBackup {
  recipes: RecipeInput[];
  /** Entrées ignorées car mal formées (titre manquant, etc.). */
  invalidCount: number;
}

/** Plan d'import après dédoublonnage. */
export interface ImportPlan {
  toImport: RecipeInput[];
  duplicateCount: number;
}

/** Bilan d'un import mené à son terme, pour le message de compte-rendu. */
export interface ImportOutcome {
  imported: number;
  duplicateCount: number;
  invalidCount: number;
  failed: number;
}

const VALID_UNITS = new Set<Unit>(UNITS.map((u) => u.value));

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Titre de comparaison : minuscules, sans espaces superflus. */
export function normalizeTitle(title: string): string {
  return title.trim().toLowerCase().replace(/\s+/g, " ");
}

/** Coerce en entier de minutes ≥ 0 (contrainte `u32` de l'API), sinon `null`. */
function minutesOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0
    ? Math.floor(value)
    : null;
}

/** Nettoie un ingrédient ; `null` si inexploitable (nom vide, unité inconnue). */
function sanitizeIngredient(raw: unknown): Ingredient | null {
  if (!isRecord(raw)) return null;
  const name = typeof raw.name === "string" ? raw.name.trim() : "";
  if (!name) return null; // l'API rejette les noms vides (EmptyIngredientName)
  if (typeof raw.unit !== "string" || !VALID_UNITS.has(raw.unit as Unit)) return null;
  const amount = typeof raw.amount === "number" && Number.isFinite(raw.amount) ? raw.amount : 0;
  return { name, amount, unit: raw.unit as Unit };
}

/** Nettoie une recette du fichier ; `null` si le titre est absent/vide. */
function sanitizeRecipe(raw: unknown): RecipeInput | null {
  if (!isRecord(raw)) return null;
  const title = typeof raw.title === "string" ? raw.title.trim() : "";
  if (!title) return null;
  const ingredients = Array.isArray(raw.ingredients)
    ? raw.ingredients
        .map(sanitizeIngredient)
        .filter((i): i is Ingredient => i !== null)
    : [];
  const steps = Array.isArray(raw.steps)
    ? raw.steps.filter((s): s is string => typeof s === "string")
    : [];
  return {
    title,
    prep_time_min: minutesOrNull(raw.prep_time_min),
    cook_time_min: minutesOrNull(raw.cook_time_min),
    photo: webPhoto(typeof raw.photo === "string" ? raw.photo : null),
    ingredients,
    steps,
  };
}

/**
 * Valide le texte d'un fichier de sauvegarde et en extrait les recettes.
 * @throws {BackupImportError} si l'enveloppe est invalide (JSON, format, version).
 */
export function parseBackup(text: string): ParsedBackup {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    throw new BackupImportError("Fichier illisible : ce n'est pas du JSON valide.");
  }
  if (!isRecord(data) || data.format !== BACKUP_FORMAT) {
    throw new BackupImportError("Ce fichier n'est pas une sauvegarde Week Meals.");
  }
  if (typeof data.version === "number" && data.version > BACKUP_VERSION) {
    throw new BackupImportError(
      "Cette sauvegarde vient d'une version plus récente de l'application.",
    );
  }
  if (!Array.isArray(data.recipes)) {
    throw new BackupImportError("Sauvegarde corrompue : aucune recette trouvée.");
  }

  const recipes: RecipeInput[] = [];
  let invalidCount = 0;
  for (const raw of data.recipes) {
    const recipe = sanitizeRecipe(raw);
    if (recipe) recipes.push(recipe);
    else invalidCount++;
  }
  return { recipes, invalidCount };
}

/**
 * Écarte les recettes déjà présentes (par titre normalisé) et les doublons
 * internes au fichier. `existingTitles` = titres du foyer.
 */
export function planImport(
  recipes: RecipeInput[],
  existingTitles: Iterable<string>,
): ImportPlan {
  const seen = new Set<string>();
  for (const title of existingTitles) seen.add(normalizeTitle(title));

  const toImport: RecipeInput[] = [];
  let duplicateCount = 0;
  for (const recipe of recipes) {
    const key = normalizeTitle(recipe.title);
    if (seen.has(key)) {
      duplicateCount++;
      continue;
    }
    seen.add(key);
    toImport.push(recipe);
  }
  return { toImport, duplicateCount };
}

/** Message de compte-rendu en français à partir du bilan d'import. */
export function importSummary(outcome: ImportOutcome): string {
  const { imported, duplicateCount, invalidCount, failed } = outcome;
  const parts: string[] = [];
  parts.push(
    imported === 0
      ? "Aucune nouvelle recette importée"
      : `${imported} recette${imported > 1 ? "s" : ""} importée${imported > 1 ? "s" : ""}`,
  );
  if (duplicateCount > 0) {
    parts.push(`${duplicateCount} déjà présente${duplicateCount > 1 ? "s" : ""}`);
  }
  if (invalidCount > 0) {
    parts.push(`${invalidCount} ignorée${invalidCount > 1 ? "s" : ""} (illisible${invalidCount > 1 ? "s" : ""})`);
  }
  if (failed > 0) {
    parts.push(`${failed} en échec`);
  }
  return `${parts.join(", ")}.`;
}
