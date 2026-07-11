# ADR-0004 — Offline ciblé sur la liste de courses

- **Statut :** acceptée (2026-07-11)

## Contexte

Le moment d'usage critique de l'app est **en magasin**, où le réseau est
souvent médiocre. Cocher un article doit marcher instantanément, réseau ou pas.
À l'inverse, un offline complet (local-first sur toute l'app) impose une
machinerie de résolution de conflits disproportionnée pour 2 utilisateurs.

## Options considérées

1. **Offline sur la liste de courses uniquement** ✅
2. **Online only** — simple, mais cocher un article peut échouer en magasin.
   Rejeté : c'est le cœur de l'usage.
3. **Local-first complet** (recettes, calendrier, liste) — confort maximal mais
   complexité disproportionnée. Rejeté à cette échelle.

## Décision

- La liste de courses est mise en cache localement (**IndexedDB**, via Dexie).
- Les mutations hors-ligne (check/uncheck, édition, ajout) vont dans une
  **file de mutations** rejouée au retour du réseau.
- Résolution de conflits : **last-write-wins** — suffisant à 2 utilisateurs.
- Le reste de l'app (recettes, calendrier) est online-only ; le service worker
  de la PWA cache uniquement le shell applicatif.
- Un indicateur discret d'état offline / sync est affiché sur l'écran liste.

## Conséquences

- L'API des items de liste doit être **idempotente** (IDs générés côté client,
  timestamps de mutation) pour que le rejeu soit sûr.
- Si le besoin d'offline s'étend un jour, reconsidérer une lib de sync
  (ex. Replicache-like) plutôt que d'étendre la file maison.
