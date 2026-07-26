/**
 * Types et hooks de la liste de courses (onglet Courses).
 *
 * Suit le contrat de l'API `shopping-list` : une liste par foyer, dont les
 * lignes sont soit **générées** depuis le calendrier (remplacées à chaque
 * génération), soit **ajoutées à la main** (conservées).
 */

import { useEffect, useState } from "react";
import {
  onlineManager,
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { api, eventSource } from "./client";

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

/**
 * Abonne l'écran au flux temps réel de la liste (#21).
 *
 * À chaque signal `changed` poussé par le serveur — qu'il vienne de soi ou de
 * l'autre personne en train de faire les courses — on invalide le cache pour
 * relire la liste. C'est ce qui la rend « vivante » à deux, sans recharger
 * l'onglet. `EventSource` se reconnecte tout seul si le réseau saute.
 *
 * Renvoie `live` (flux connecté) pour un indicateur discret côté UI.
 */
export function useShoppingListStream(): { live: boolean } {
  const queryClient = useQueryClient();
  const [live, setLive] = useState(false);

  useEffect(() => {
    const source = eventSource("/shopping-list/stream");
    const invalidate = () => queryClient.invalidateQueries({ queryKey: LIST_KEY });

    source.addEventListener("changed", invalidate);
    source.onopen = () => setLive(true);
    // `EventSource` bascule seul en reconnexion : on reflète juste l'état.
    source.onerror = () => setLive(false);

    return () => {
      source.removeEventListener("changed", invalidate);
      source.close();
    };
  }, [queryClient]);

  return { live };
}

// --- Écritures : optimistes et rejouées hors-ligne (#21) ------------------

/**
 * Clés de mutation, toutes préfixées `shopping-list`.
 *
 * Deux rôles : l'indicateur de synchro les compte d'un coup
 * (`useIsMutating({ mutationKey: ["shopping-list"] })`), et les valeurs par
 * défaut enregistrées par [`configureShoppingListOffline`] permettent de
 * **rejouer** une mutation restée en file, même après un rechargement (React
 * Query ne persiste que la clé + les variables, il retrouve ici la fonction et
 * l'optimisme associés).
 */
const ADD_KEY = ["shopping-list", "add"] as const;
const UPDATE_KEY = ["shopping-list", "update"] as const;
const DELETE_KEY = ["shopping-list", "delete"] as const;
const CLEAR_KEY = ["shopping-list", "clear-checked"] as const;

/** Variables d'ajout : `id` généré côté client (ligne optimiste stable). */
export interface AddVars {
  id: string;
  name: string;
  amount: number;
  unit: Unit;
}

/** Variables d'édition : `id` cible + champs à changer (les autres inchangés). */
export interface UpdateVars {
  id: string;
  checked?: boolean;
  name?: string;
  amount?: number;
  unit?: Unit;
}

/** Contexte d'annulation : l'état de la liste avant la mutation optimiste. */
interface Rollback {
  previous?: ShoppingItem[];
}

// --- Application optimiste (fonctions pures, testées) ----------------------

/** Ajoute une ligne (idempotent : ne double pas si l'`id` est déjà présent). */
export function applyAdd(items: ShoppingItem[], vars: AddVars): ShoppingItem[] {
  if (items.some((item) => item.id === vars.id)) return items;
  return [
    ...items,
    {
      id: vars.id,
      name: vars.name,
      amount: vars.amount,
      unit: vars.unit,
      category: null,
      checked: false,
      generated: false,
    },
  ];
}

/** Applique une édition à la ligne ciblée (champs `undefined` = inchangés). */
export function applyUpdate(items: ShoppingItem[], vars: UpdateVars): ShoppingItem[] {
  return items.map((item) => {
    if (item.id !== vars.id) return item;
    const next = { ...item };
    if (vars.checked !== undefined) next.checked = vars.checked;
    if (vars.name !== undefined) next.name = vars.name;
    if (vars.amount !== undefined) next.amount = vars.amount;
    if (vars.unit !== undefined) next.unit = vars.unit;
    return next;
  });
}

/** Retire une ligne. */
export function applyDelete(items: ShoppingItem[], id: string): ShoppingItem[] {
  return items.filter((item) => item.id !== id);
}

/** Retire toutes les lignes cochées. */
export function applyClearChecked(items: ShoppingItem[]): ShoppingItem[] {
  return items.filter((item) => !item.checked);
}

/**
 * Options communes d'une mutation optimiste sur la liste :
 * - `onMutate` applique le changement au cache immédiatement (ressenti
 *   instantané — et **c'est ce cache persisté qui reste visible hors-ligne**) ;
 * - `onError` restaure l'état précédent si l'appel serveur échoue *en ligne* ;
 * - `onSettled` resynchronise depuis le serveur, mais seulement en ligne
 *   (hors-ligne, la mutation est en file, il n'y a rien à resynchroniser).
 *
 * En `networkMode` par défaut (`online`), `onMutate` s'exécute tout de suite
 * puis la mutation est **mise en pause** tant qu'il n'y a pas de réseau ; React
 * Query la relance seule au retour du réseau (last-write-wins, suffisant à
 * deux). `retry` encaisse les micro-coupures en rayon.
 */
function optimistic<V>(
  queryClient: QueryClient,
  apply: (items: ShoppingItem[], vars: V) => ShoppingItem[],
) {
  return {
    retry: 3,
    onMutate: async (vars: V): Promise<Rollback> => {
      await queryClient.cancelQueries({ queryKey: LIST_KEY });
      const previous = queryClient.getQueryData<ShoppingItem[]>(LIST_KEY);
      queryClient.setQueryData<ShoppingItem[]>(LIST_KEY, (old) => apply(old ?? [], vars));
      return { previous };
    },
    onError: (_error: unknown, _vars: V, context: Rollback | undefined) => {
      if (context?.previous) queryClient.setQueryData(LIST_KEY, context.previous);
    },
    onSettled: () => {
      if (onlineManager.isOnline()) {
        void queryClient.invalidateQueries({ queryKey: LIST_KEY });
      }
    },
  };
}

/**
 * Enregistre les valeurs par défaut des mutations de la liste sur le client.
 *
 * À appeler **une fois au démarrage**, avant la restauration du cache persisté :
 * c'est ce qui rend rejouable une mutation mise en file avant un rechargement.
 */
export function configureShoppingListOffline(queryClient: QueryClient): void {
  queryClient.setMutationDefaults(ADD_KEY, {
    mutationFn: (vars: AddVars) =>
      api.post<ShoppingItem>("/shopping-list/items", {
        name: vars.name,
        amount: vars.amount,
        unit: vars.unit,
      }),
    ...optimistic<AddVars>(queryClient, applyAdd),
  });

  queryClient.setMutationDefaults(UPDATE_KEY, {
    mutationFn: ({ id, ...patch }: UpdateVars) =>
      api.patch<ShoppingItem>(`/shopping-list/items/${id}`, patch),
    ...optimistic<UpdateVars>(queryClient, applyUpdate),
  });

  queryClient.setMutationDefaults(DELETE_KEY, {
    mutationFn: (id: string) => api.delete<void>(`/shopping-list/items/${id}`),
    ...optimistic<string>(queryClient, applyDelete),
  });

  queryClient.setMutationDefaults(CLEAR_KEY, {
    mutationFn: () => api.delete<void>("/shopping-list/checked"),
    ...optimistic<void>(queryClient, applyClearChecked),
  });
}

/** Ajoute une ligne à la main (optimiste, rejouée hors-ligne). */
export function useAddItem() {
  const mutation = useMutation<ShoppingItem, Error, AddVars>({ mutationKey: ADD_KEY });
  return {
    ...mutation,
    // L'appelant fournit le combo ; l'`id` client stabilise la ligne optimiste.
    mutate: (item: { name: string; amount: number; unit: Unit }) =>
      mutation.mutate({ id: crypto.randomUUID(), ...item }),
  };
}

/** Coche ou édite une ligne (optimiste, rejouée hors-ligne). */
export function useUpdateItem() {
  return useMutation<ShoppingItem, Error, UpdateVars>({ mutationKey: UPDATE_KEY });
}

/** Supprime une ligne (optimiste, rejouée hors-ligne). */
export function useDeleteItem() {
  return useMutation<void, Error, string>({ mutationKey: DELETE_KEY });
}

/** Vide toutes les lignes cochées (optimiste, rejouée hors-ligne). */
export function useClearChecked() {
  return useMutation<void, Error, void>({ mutationKey: CLEAR_KEY });
}

/**
 * Fixe l'ordre d'affichage (glisser-déposer). En ligne uniquement : l'ordre a
 * peu d'intérêt hors-ligne, et [`PendingList`] gère déjà l'optimisme local.
 */
export function useReorderItems() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => api.post<void>("/shopping-list/reorder", { ids }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: LIST_KEY }),
  });
}

/** (Re)génère la liste depuis le calendrier, sur une plage de jours. En ligne. */
export function useGenerateList() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (range: { from: string; to: string }) =>
      api.post<ShoppingItem[]>("/shopping-list/generate", range),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: LIST_KEY }),
  });
}

/** État de synchro affiché par l'indicateur discret (en-tête Courses). */
export type SyncState = "offline" | "syncing" | "live";

/**
 * Abonne l'écran au temps réel **et** calcule l'état de synchro à montrer :
 * `offline` (pas de réseau), `syncing` (des mutations en file/en cours à
 * rejouer), sinon `live` (en ligne et à jour).
 *
 * Regroupe le flux SSE (via [`useShoppingListStream`]) et le compteur de
 * mutations pour n'ouvrir qu'un seul `EventSource`.
 */
export function useShoppingSync(): { state: SyncState } {
  // Effet de bord : ouvre le flux temps réel et invalide sur `changed`.
  useShoppingListStream();

  const [online, setOnline] = useState(onlineManager.isOnline());
  useEffect(() => onlineManager.subscribe(() => setOnline(onlineManager.isOnline())), []);

  const pending = useIsMutating({ mutationKey: ["shopping-list"] });

  const state: SyncState = !online ? "offline" : pending > 0 ? "syncing" : "live";
  return { state };
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
