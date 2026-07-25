/**
 * Base locale du navigateur (IndexedDB) via Dexie.
 *
 * Sert de mémoire hors-ligne de l'app : on y range le cache de React Query
 * (liste de courses + mutations en attente) pour que la liste reste
 * consultable et modifiable sans réseau, en magasin (cf. ADR-0004, #21).
 *
 * Dexie n'est qu'un emballage confortable autour d'`IndexedDB`, la petite base
 * intégrée à tous les navigateurs — aucune dépendance serveur.
 */

import Dexie, { type Table } from "dexie";

/** Une entrée clé → valeur : ici un unique blob « le cache React Query ». */
export interface CacheEntry {
  key: string;
  value: unknown;
}

/** La base « week-meals » et sa table clé-valeur. */
class OfflineDb extends Dexie {
  cache!: Table<CacheEntry, string>;

  constructor() {
    super("week-meals");
    // `key` est la clé primaire ; la valeur est un blob opaque (le snapshot
    // sérialisé du cache), on n'indexe donc rien d'autre.
    this.version(1).stores({ cache: "key" });
  }
}

/** Instance partagée (une seule connexion IndexedDB pour tout l'onglet). */
export const offlineDb = new OfflineDb();
