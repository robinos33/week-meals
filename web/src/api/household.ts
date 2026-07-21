/**
 * Réglages du foyer (#57).
 *
 * Le planning est partagé : ces réglages portent sur le foyer, pas sur
 * l'utilisateur. Pour l'instant, le seul réglage est le **premier jour de la
 * semaine** (`week_start_day`), suivant la convention `Date.getDay()` :
 * 0 = dimanche … 6 = samedi. Il pilote le découpage de l'onglet Semaine et,
 * par ricochet, la fenêtre de génération de la liste de courses.
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";

/** Réglages du foyer exposés par l'API. */
export interface HouseholdSettings {
  /** Premier jour de la semaine (0 = dimanche … 6 = samedi). */
  week_start_day: number;
}

/** Défaut historique : lundi, tant que les réglages ne sont pas chargés. */
export const DEFAULT_WEEK_START_DAY = 1;

const settingsKey = ["household-settings"] as const;

/** Lit les réglages du foyer. */
export function useHouseholdSettings() {
  return useQuery({
    queryKey: settingsKey,
    queryFn: () => api.get<HouseholdSettings>("/household/settings"),
  });
}

/** Met à jour le premier jour de la semaine du foyer. */
export function useSetWeekStartDay() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (weekStartDay: number) =>
      api.put<HouseholdSettings>("/household/settings", { week_start_day: weekStartDay }),
    onSuccess: (settings) => {
      queryClient.setQueryData(settingsKey, settings);
      // Le découpage de la semaine change : les plannings affichés doivent se
      // recharger sur la nouvelle fenêtre.
      return queryClient.invalidateQueries({ queryKey: ["meal-plan"] });
    },
  });
}
