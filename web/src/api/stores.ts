/**
 * Rayons et magasins (paramétrage du tri de la liste de courses).
 *
 * Le **catalogue des rayons** est une constante applicative servie par l'API
 * (`GET /aisles`) : c'est le vocabulaire qui range les ingrédients du
 * dictionnaire, et le front n'en duplique donc ni la liste ni les libellés.
 *
 * Un **magasin** n'est pas un catalogue de produits mais un *trajet* : un nom
 * et l'ordre dans lequel on en traverse les rayons. C'est ce qui permet de
 * trier la liste pour ne faire qu'un aller simple en magasin. Les deux
 * ressources sont scopées au foyer et changent rarement : on les garde
 * longtemps en cache, et elles survivent hors-ligne avec le reste du cache
 * persisté — le tri doit tenir en rayon, réseau ou pas.
 */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";

/** Un rayon du catalogue : identifiant stable + libellé affiché. */
export interface Aisle {
  slug: string;
  label: string;
}

/** Un magasin du foyer. */
export interface Store {
  id: string;
  name: string;
  /** Slugs des rayons, dans l'ordre de visite. */
  aisles: string[];
}

const AISLES_KEY = ["aisles"] as const;
const STORES_KEY = ["stores"] as const;

/** Une journée : le catalogue ne bouge qu'aux déploiements. */
const A_DAY = 24 * 60 * 60 * 1000;

/** Le catalogue des rayons, dans l'ordre par défaut. */
export function useAisles() {
  return useQuery({
    queryKey: AISLES_KEY,
    queryFn: () => api.get<Aisle[]>("/aisles"),
    staleTime: A_DAY,
  });
}

/** Les magasins du foyer. */
export function useStores() {
  return useQuery({
    queryKey: STORES_KEY,
    queryFn: () => api.get<Store[]>("/stores"),
    staleTime: A_DAY,
  });
}

/** Crée un magasin (ordre de rayons par défaut, à rectifier ensuite). */
export function useCreateStore() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.post<Store>("/stores", { name }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: STORES_KEY }),
  });
}

/**
 * Renomme un magasin et/ou réordonne ses rayons.
 *
 * L'ordre envoyé peut être partiel : le serveur complète avec les rayons non
 * cités, dans l'ordre par défaut.
 */
export function useUpdateStore() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...patch }: { id: string; name?: string; aisles?: string[] }) =>
      api.patch<Store>(`/stores/${id}`, patch),
    onSuccess: (store) => {
      queryClient.setQueryData<Store[]>(STORES_KEY, (stores) =>
        stores?.map((current) => (current.id === store.id ? store : current)),
      );
    },
  });
}

/** Supprime un magasin. */
export function useDeleteStore() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.delete<void>(`/stores/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: STORES_KEY }),
  });
}
