# ADR-0003 — Recettes : DB source de vérité + fichiers seed versionnés

- **Statut :** acceptée (2026-07-11)

## Contexte

Deux besoins en tension : éditer les recettes **depuis l'UI mobile** (fluide),
et avoir les recettes **en fichiers versionnés** (portabilité, scraping assisté,
historique git).

## Options considérées

1. **DB source de vérité + seed versionné** ✅
2. **Fichiers git = source de vérité** — l'UI édite via l'API GitHub (chaque
   modif = un commit). Portable mais édition mobile lente (latence
   commit/déploiement) et nettement plus complexe.
3. **Fichiers en lecture seule** — pas d'ajout de recette depuis le téléphone.
   Rejeté : c'est un usage central.

## Décision

Option 1 :

- La **DB est la source de vérité** — CRUD complet depuis l'UI.
- `data/recipes/*.yaml` sert de **seed initial** et de cible pour le scraping.
- Deux commandes CLI font le pont :
  - `import` — upsert des YAML vers la DB (idempotent) ;
  - `export` — dump des recettes de la DB vers YAML (sauvegarde, partage).
- Le référentiel `data/ingredients.yaml` (poids moyens) suit la même logique :
  versionné, seedé en DB au déploiement.

## Conséquences

- Le format YAML des recettes est un **contrat public** du projet (documenté
  par l'exemple dans `data/recipes/`) — le faire évoluer avec précaution.
- Les fichiers ne reflètent pas automatiquement l'état de la DB : l'export est
  une action volontaire (ou un job périodique, plus tard).
