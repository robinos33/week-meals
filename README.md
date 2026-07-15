# Week Meals

PWA mobile-first pour un foyer : gérer ses recettes, planifier les repas de la
semaine (midi / soir) et générer une liste de courses intelligente.

**La spécificité :** la génération de la liste de courses convertit les grammages
en unités achetables — `600 g de courgettes` devient `3 courgettes`, grâce à un
référentiel versionné de poids moyens ([data/ingredients.yaml](data/ingredients.yaml)).

> 🚧 **Statut : phase de conception.** Pas encore de code applicatif — le plan,
> les décisions d'architecture et les schémas de données sont dans [docs/](docs/).

## Fonctionnalités cibles

- 📖 **Recettes** — CRUD complet : photo, titre, temps de préparation/cuisson,
  ingrédients (quantité + unité : g, kg, mL, L ou pièces) et étapes de préparation
- 📅 **Semaine** — calendrier 7 jours × 2 créneaux (midi/soir), on y place les recettes
- 🛒 **Liste de courses** — générée depuis une plage de jours du calendrier,
  éditable, cochable en magasin (UX inspirée de Google Keep), **fonctionne hors-ligne**
- 👥 **Foyer** — auth pseudo + mot de passe, **aucun email requis**, accès sur
  invitation uniquement (open-source friendly : zéro donnée perso à configurer)

## Stack

| Composant | Choix |
|---|---|
| Backend | Rust — Axum + SQLx (clean architecture, crates par couche) |
| Frontend | React + Vite + TypeScript, PWA (offline via IndexedDB) |
| BDD | PostgreSQL (Neon en prod, Docker en local) |
| Photos | Cloudflare R2 (S3-compatible) |
| Hébergement | Cloudflare Pages (front) + Scaleway Serverless Containers (API) |

Le détail des choix et leurs alternatives : [docs/adr/](docs/adr/).

## Dev local / self-host

Environnement reproductible : PostgreSQL en conteneur + migrations SQLx.

### Prérequis

- [Docker](https://docs.docker.com/get-docker/) (avec `docker compose`)
- [`sqlx-cli`](https://crates.io/crates/sqlx-cli) pour jouer les migrations :
  ```sh
  cargo install sqlx-cli --no-default-features --features rustls,postgres
  ```

### Démarrage

```sh
# 1. Configuration — copier l'exemple et ajuster si besoin
cp .env.example .env

# 2. Base de données
docker compose up -d          # Postgres sur localhost:5432 (POSTGRES_PORT pour changer)

# 3. Migrations
sqlx migrate run --source api/migrations
```

Avant tout déploiement, remplacer les valeurs `change-me` de `.env`
(`SESSION_SECRET`, `BOOTSTRAP_INVITE_CODE`) — voir [ADR-0002](docs/adr/0002-auth-sans-email.md).

### Workflow de migration

Les migrations vivent dans [`api/migrations/`](api/migrations/), une par fichier
`AAAAMMJJHHMMSS_description.sql`, appliquées dans l'ordre et suivies par SQLx
(table `_sqlx_migrations`).

```sh
sqlx migrate add <description>          # nouvelle migration (préciser --source api/migrations)
sqlx migrate run    --source api/migrations   # appliquer les migrations en attente
sqlx migrate info   --source api/migrations   # état appliqué / en attente
```

> Les migrations sont **append-only** : ne jamais éditer une migration déjà
> livrée, en ajouter une nouvelle.

## Documentation

- [Plan & architecture](docs/plan.md) — modèle métier, structure du code, roadmap
- [ADR](docs/adr/) — décisions d'architecture
- [Brief design](docs/design/brief.md) — direction UX/UI

## Langue

Projet personnel francophone, à vocation open source : **docs en français,
code / schémas / routes en anglais**.

## Licence

[MIT](LICENSE)
