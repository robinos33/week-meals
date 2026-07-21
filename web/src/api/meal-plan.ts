/**
 * Types et hooks du calendrier (onglet Semaine).
 *
 * Suit le contrat de l'API `meal-plan` : entrées `(date, slot, recipe_id)` sur
 * une plage de dates. Les résumés de recettes viennent de `/recipes` (pour
 * afficher photo + titre dans les créneaux remplis).
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";

/** Créneau d'un repas. */
export type MealSlot = "lunch" | "dinner";

/** Une entrée du calendrier. */
export interface MealPlanEntry {
  date: string; // ISO YYYY-MM-DD
  slot: MealSlot;
  recipe_id: string;
}

/** Résumé de recette utile aux créneaux (sous-ensemble de `RecipeView`). */
export interface RecipeSummary {
  id: string;
  title: string;
  photo: string | null;
  /** Nombre de fois cuisinée (#58) — sert au podium de la grille Recettes. */
  cooked_count: number;
}

/** Calendrier du foyer sur une plage inclusive `from..=to`. */
export function useWeekPlan(from: string, to: string) {
  return useQuery({
    queryKey: ["meal-plan", from, to],
    queryFn: () => api.get<MealPlanEntry[]>(`/meal-plan?from=${from}&to=${to}`),
  });
}

/** Toutes les recettes du foyer (pour le picker et les mini-cartes). */
export function useRecipeSummaries() {
  return useQuery({
    queryKey: ["recipes", ""],
    queryFn: () => api.get<RecipeSummary[]>("/recipes"),
  });
}

/** Place (ou remplace) une recette dans un créneau. */
export function useSetEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (entry: { date: string; slot: MealSlot; recipe_id: string }) =>
      api.put<void>(`/meal-plan/${entry.date}/${entry.slot}`, { recipe_id: entry.recipe_id }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["meal-plan"] }),
  });
}

/** Vide un créneau. */
export function useClearEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (entry: { date: string; slot: MealSlot }) =>
      api.delete<void>(`/meal-plan/${entry.date}/${entry.slot}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["meal-plan"] }),
  });
}
