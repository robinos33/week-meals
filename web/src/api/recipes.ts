/**
 * Types et hooks partagés du domaine Recettes (liste, détail, formulaire).
 *
 * Le contrat suit `RecipeView` / `RecipeBody` de l'API (cf.
 * `api/recipes/src/presentation.rs`). Centralisé ici pour que les trois écrans
 * parlent des mêmes types, et que les invalidations de cache restent cohérentes.
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";

/** Unités reconnues par l'API (`kernel::Unit`, sérialisées en minuscules). */
export type Unit = "g" | "kg" | "ml" | "l" | "piece";

/** Unités proposées dans les formulaires, avec leur libellé lisible. */
export const UNITS: ReadonlyArray<{ value: Unit; label: string }> = [
  { value: "g", label: "g" },
  { value: "kg", label: "kg" },
  { value: "ml", label: "mL" },
  { value: "l", label: "L" },
  { value: "piece", label: "pièce(s)" },
];

/** Ingrédient d'une recette (quantité + unité). */
export interface Ingredient {
  name: string;
  amount: number;
  unit: Unit;
}

/** Recette complète telle qu'exposée par l'API. */
export interface RecipeView {
  id: string;
  household_id: string;
  title: string;
  photo: string | null;
  prep_time_min: number | null;
  cook_time_min: number | null;
  ingredients: Ingredient[];
  steps: string[];
}

/** Corps de création / mise à jour (remplacement complet). */
export interface RecipeInput {
  title: string;
  prep_time_min: number | null;
  cook_time_min: number | null;
  photo: string | null;
  ingredients: Ingredient[];
  steps: string[];
}

/** Temps total (préparation + cuisson) formaté, ou `null` si non renseigné. */
export function totalTime(recipe: {
  prep_time_min: number | null;
  cook_time_min: number | null;
}): string | null {
  const total = (recipe.prep_time_min ?? 0) + (recipe.cook_time_min ?? 0);
  return total > 0 ? `${total} min` : null;
}

const listKey = (search: string) => ["recipes", search] as const;
const detailKey = (id: string) => ["recipe", id] as const;

/** Liste (ou recherche) des recettes du foyer. */
export function useRecipes(search: string) {
  return useQuery({
    queryKey: listKey(search),
    queryFn: () =>
      api.get<RecipeView[]>(
        `/recipes${search.trim() ? `?search=${encodeURIComponent(search.trim())}` : ""}`,
      ),
  });
}

/** Détail d'une recette. */
export function useRecipe(id: string) {
  return useQuery({
    queryKey: detailKey(id),
    queryFn: () => api.get<RecipeView>(`/recipes/${id}`),
  });
}

/** Invalide toutes les listes de recettes (recherche incluse). */
function invalidateLists(queryClient: ReturnType<typeof useQueryClient>) {
  return queryClient.invalidateQueries({ queryKey: ["recipes"] });
}

/** Création d'une recette. */
export function useCreateRecipe() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: RecipeInput) => api.post<RecipeView>("/recipes", input),
    onSuccess: () => invalidateLists(queryClient),
  });
}

/** Mise à jour (remplacement complet) d'une recette. */
export function useUpdateRecipe(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: RecipeInput) => api.put<RecipeView>(`/recipes/${id}`, input),
    onSuccess: (recipe) => {
      queryClient.setQueryData(detailKey(id), recipe);
      return invalidateLists(queryClient);
    },
  });
}

/** Suppression d'une recette. */
export function useDeleteRecipe(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.delete<void>(`/recipes/${id}`),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: detailKey(id) });
      return invalidateLists(queryClient);
    },
  });
}
