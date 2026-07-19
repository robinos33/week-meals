/**
 * Types et hooks de la liste de courses (onglet Courses).
 *
 * Suit le contrat de l'API `shopping-list` : une liste par foyer, dont les
 * lignes sont soit **générées** depuis le calendrier (remplacées à chaque
 * génération), soit **ajoutées à la main** (conservées).
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";

/** Unités acceptées par l'API (mêmes que les recettes). */
export const UNITS = ["g", "kg", "ml", "l", "piece"] as const;
export type Unit = (typeof UNITS)[number];

/** Libellé affiché d'une unité. */
export const UNIT_LABELS: Record<Unit, string> = {
  g: "g",
  kg: "kg",
  ml: "mL",
  l: "L",
  piece: "pièce(s)",
};

/** Une ligne de la liste. */
export interface ShoppingItem {
  id: string;
  name: string;
  amount: number;
  unit: Unit;
  category: string | null;
  checked: boolean;
  generated: boolean;
}

const LIST_KEY = ["shopping-list"] as const;

/** La liste courante du foyer. */
export function useShoppingList() {
  return useQuery({
    queryKey: LIST_KEY,
    queryFn: () => api.get<ShoppingItem[]>("/shopping-list"),
  });
}

/** Invalide la liste après une écriture. */
function useListInvalidation() {
  const queryClient = useQueryClient();
  return () => queryClient.invalidateQueries({ queryKey: LIST_KEY });
}

/** Ajoute une ligne à la main. */
export function useAddItem() {
  const invalidate = useListInvalidation();
  return useMutation({
    mutationFn: (item: { name: string; amount: number; unit: Unit }) =>
      api.post<ShoppingItem>("/shopping-list/items", item),
    onSuccess: invalidate,
  });
}

/** Coche ou édite une ligne (champs omis = inchangés). */
export function useUpdateItem() {
  const invalidate = useListInvalidation();
  return useMutation({
    mutationFn: ({
      id,
      ...patch
    }: {
      id: string;
      checked?: boolean;
      name?: string;
      amount?: number;
      unit?: Unit;
    }) => api.patch<ShoppingItem>(`/shopping-list/items/${id}`, patch),
    onSuccess: invalidate,
  });
}

/** Supprime une ligne. */
export function useDeleteItem() {
  const invalidate = useListInvalidation();
  return useMutation({
    mutationFn: (id: string) => api.delete<void>(`/shopping-list/items/${id}`),
    onSuccess: invalidate,
  });
}

/** Vide toutes les lignes cochées. */
export function useClearChecked() {
  const invalidate = useListInvalidation();
  return useMutation({
    mutationFn: () => api.delete<void>("/shopping-list/checked"),
    onSuccess: invalidate,
  });
}

/** Fixe l'ordre d'affichage (glisser-déposer) : `ids` dans l'ordre voulu. */
export function useReorderItems() {
  const invalidate = useListInvalidation();
  return useMutation({
    mutationFn: (ids: string[]) => api.post<void>("/shopping-list/reorder", { ids }),
    onSuccess: invalidate,
  });
}

/** (Re)génère la liste depuis le calendrier, sur une plage de jours. */
export function useGenerateList() {
  const invalidate = useListInvalidation();
  return useMutation({
    mutationFn: (range: { from: string; to: string }) =>
      api.post<ShoppingItem[]>("/shopping-list/generate", range),
    onSuccess: invalidate,
  });
}

/** Nom normalisé pour comparer deux articles (casse et espaces ignorés). */
function normalizeName(name: string): string {
  return name.trim().toLowerCase();
}

/**
 * Deux articles partagent-ils le même combo **produit / quantité / unité** ?
 * Sert à garantir l'unicité côté saisie (pas de doublon exact dans la liste).
 */
export function sameCombo(
  item: Pick<ShoppingItem, "name" | "amount" | "unit">,
  candidate: { name: string; amount: number; unit: Unit },
): boolean {
  return (
    normalizeName(item.name) === normalizeName(candidate.name) &&
    Math.abs(item.amount - candidate.amount) < 1e-9 &&
    item.unit === candidate.unit
  );
}

/** Pas d'incrément adapté à l'unité pour le sélecteur de quantité. */
export function quantityStep(unit: Unit): number {
  if (unit === "piece") return 1;
  if (unit === "kg" || unit === "l") return 0.5;
  return 50; // g, mL
}

/**
 * Ajuste une quantité d'un cran (±1 pas), en restant strictement positive et
 * sans bavure de flottant (`0.1 + 0.5`…).
 */
export function adjustQuantity(amount: number, unit: Unit, direction: 1 | -1): number {
  const step = quantityStep(unit);
  const next = amount + direction * step;
  return Math.max(step, Math.round(next * 100) / 100);
}

/** Quantité formatée pour l'affichage (`3 pièce(s)`, `250 g`). */
export function formatQuantity(item: ShoppingItem): string {
  // Les quantités sont des flottants côté API : on évite « 250.0 g ».
  const amount = Number.isInteger(item.amount) ? item.amount : Number(item.amount.toFixed(2));
  return `${amount} ${UNIT_LABELS[item.unit]}`;
}
