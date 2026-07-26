/**
 * Persister React Query adossé à Dexie (IndexedDB).
 *
 * React Query garde son état en mémoire ; ce persister en écrit un instantané
 * dans IndexedDB à chaque changement, et le relit au démarrage. C'est ce qui
 * rend la liste de courses disponible hors-ligne et après un rechargement, et
 * qui conserve les mutations mises en file quand le réseau manque (#21).
 */

import type { PersistedClient, Persister } from "@tanstack/react-query-persist-client";
import { offlineDb } from "./offline-db";

/** Clé unique de l'instantané dans la table `cache`. */
const SNAPSHOT_KEY = "react-query";

/** Persister lisant/écrivant l'instantané du cache dans Dexie. */
export const dexiePersister: Persister = {
  persistClient: async (client) => {
    await offlineDb.cache.put({ key: SNAPSHOT_KEY, value: client });
  },
  restoreClient: async () => {
    const entry = await offlineDb.cache.get(SNAPSHOT_KEY);
    return entry?.value as PersistedClient | undefined;
  },
  removeClient: async () => {
    await offlineDb.cache.delete(SNAPSHOT_KEY);
  },
};
