# Week Meals

PWA mobile-first pour un foyer : gérer ses recettes, planifier les repas de la
semaine (midi / soir) et générer une liste de courses intelligente.

**La spécificité :** la génération de la liste de courses convertit les grammages
en unités achetables — `600 g de courgettes` devient `3 courgettes`, grâce à un
référentiel versionné de poids moyens ([data/ingredients.yaml](data/ingredients.yaml)).

> 🚧 **Statut : en construction.** L'API (auth, recettes) et la coquille PWA
> tournent en local ; le parcours est ouvert en **mode public** le temps de
> câbler les écrans (voir « Mode public » plus bas). Plan, décisions et schémas
> dans [docs/](docs/).

## Fonctionnalités cibles

- 📖 **Recettes** — CRUD complet : photo, titre, temps de préparation/cuisson,
  ingrédients (quantité + unité : g, kg, mL, L ou pièces) et étapes de préparation
- 📅 **Semaine** — calendrier 7 jours × 2 créneaux (midi/soir), on y place les recettes
- 🛒 **Liste de courses** — générée depuis une plage de jours du calendrier,
  éditable, cochable en magasin (UX inspirée de Google Keep), **fonctionne hors-ligne**
- 👥 **Foyer** — auth par **passkeys** (Face ID / empreinte), **aucun email ni
  mot de passe**, enrôlement des appareils par code d'appairage (open-source
  friendly : zéro donnée perso à configurer)

## Stack

| Composant | Choix |
|---|---|
| Backend | Rust — Axum + SQLx (clean architecture, crates par couche) |
| Frontend | React + Vite + TypeScript, PWA (offline via IndexedDB) |
| BDD | SQLite — un simple fichier, créé et migré au démarrage |
| Photos | Cloudflare R2 (S3-compatible) |
| Hébergement | Cloudflare Pages (front) + Scaleway Serverless Containers (API) |

Le détail des choix et leurs alternatives : [docs/adr/](docs/adr/).

## Dev local / self-host

La base est un **fichier SQLite** (cf. [ADR-0008](docs/adr/0008-sqlite-volume-fly.md)) :
rien à provisionner, le serveur le crée et le migre au démarrage. Le
`docker compose` ne sert plus qu'à MinIO, qui tient lieu de R2 pour les photos.

### Prérequis

- [Docker](https://docs.docker.com/get-docker/) (avec `docker compose`), pour
  les photos uniquement — l'app démarre sans.

### Démarrage

```sh
# 1. Configuration — copier l'exemple et ajuster si besoin
cp .env.example .env

# 2. Stockage des photos (facultatif : sans lui, la présignature répond 503)
docker compose up -d

# 3. API (Axum) — lit .env, crée et migre ./weekmeals.db, écoute sur :8080
cargo run --manifest-path api/Cargo.toml -p server

# 4. Front (Vite) — dans un autre terminal, sur :5173
cd web && cp .env.example .env.local && npm install && npm run dev
```

Repartir de zéro tient en une commande — supprimer le fichier suffit :

```sh
rm -f weekmeals.db*
```

### Tests

Les tests d'intégration ouvrent chacun une base SQLite temporaire : ils tournent
sans service à lancer, et sans `--ignored`.

```sh
cargo test --manifest-path api/Cargo.toml --workspace
```

### CLI — recettes en YAML (`weekmeals`)

Le binaire `cli` (`weekmeals`) importe / exporte / seede les recettes au format
YAML (contrat des seeds, cf. [`data/recipes/`](data/recipes/)). Il lit `.env`
(`DATABASE_URL`) et cible le **foyer de démonstration** par défaut (`--household`
pour un autre foyer).

```sh
alias weekmeals='cargo run --manifest-path api/Cargo.toml -p cli --'

weekmeals seed                       # importe data/recipes/*.yaml (upsert idempotent)
weekmeals import chemin/recette.yaml # importe un ou plusieurs fichiers
weekmeals export --out ./mes-recettes  # un fichier .yaml par recette
weekmeals export                     # ...ou sur stdout (documents séparés par ---)

weekmeals seed-ingredients           # référentiel des poids moyens (global)
```

Le **référentiel d'ingrédients** ([data/ingredients.yaml](data/ingredients.yaml))
est global (pas par foyer) et alimente la conversion grammes → unités de la
liste de courses. `seed-ingredients` fait un upsert par nom : le rejouer après
avoir édité le fichier met simplement la base à jour.

L'import est **idempotent** : il fait un upsert par titre (dans le foyer), donc
rejouer un seed ne crée pas de doublon.

#### Récupérer une recette depuis le web

```sh
weekmeals scrape <url> --out recette.yaml   # ...ou sur stdout
# on relit / corrige le YAML, puis :
weekmeals import recette.yaml
```

`scrape` lit le **JSON-LD schema.org** que publient la plupart des sites de
cuisine — pas de sélecteur HTML propre à chaque site. Les quantités des sites
étant du texte libre (« 2 c. à soupe d'huile »), leur découpage en
`quantity`/`unit` est **heuristique** : le YAML produit est un **brouillon à
relire** avant import. Les cuillères sont converties (soupe = 15 mL, café =
5 mL), de même que cL/dL ; sans unité reconnue, la ligne devient une pièce.

Le même import est disponible dans l'app : le formulaire de création de recette
a un champ **« Importer depuis une URL »** qui prérempli les champs (à corriger
avant d'enregistrer). Exposé en API, c'est le serveur qui va chercher l'URL :
`POST /recipes/scrape` est donc gardé contre le **SSRF** (https uniquement, IP
publiques vérifiées et épinglées, redirections coupées, taille bornée).

### Authentification par passkeys (cf. [ADR-0006](docs/adr/0006-auth-passkeys-appareils-enroles.md))

L'accès se fait par **passkeys WebAuthn** : « Continuer avec Face ID », sans
mot de passe ni identifiant à saisir. Un appareil s'enrôle pendant une fenêtre
ouverte au CLI, protégée par un code d'appairage à usage unique :

```sh
weekmeals device open-window --minutes 15   # imprime le code d'appairage
weekmeals device list                        # appareils enrôlés
weekmeals device revoke <id>                 # révoque un appareil
weekmeals device close-window                # ferme la fenêtre
```

Le mode est piloté par `AUTH_MODE` :

- `locked` (défaut, fail-closed) : seuls les appareils enrôlés passent.
- `disabled` : l'API n'exige aucune session et scope tout au foyer de
  démonstration (migration `seed_demo_household`) ; le front n'affiche pas
  d'écran de connexion. Pratique en dev/preview. **Ne jamais utiliser en
  production.** (L'ancien `AUTH_DISABLED=1` reste accepté et équivaut à
  `disabled`.)

En mode `locked`, front et API doivent partager le même domaine parent
(`WEBAUTHN_RP_ID` / `WEBAUTHN_RP_ORIGIN`). Avant tout déploiement, remplacer la
valeur `change-me` de `SESSION_SECRET` dans `.env`.

### Workflow de migration

Les migrations vivent dans [`api/migrations/`](api/migrations/), une par fichier
`AAAAMMJJHHMMSS_description.sql`, appliquées dans l'ordre et suivies par SQLx
(table `_sqlx_migrations`).

Elles sont écrites en **SQLite** : pas de `uuid` ni de `timestamptz` (les
correspondances de types sont fixées par l'[ADR-0008](docs/adr/0008-sqlite-volume-fly.md)),
et `alter table` n'accepte qu'une colonne à la fois. Ajouter une migration :
créer le fichier à la main, ou avec [`sqlx-cli`](https://crates.io/crates/sqlx-cli)
si on l'a installé (`cargo install sqlx-cli --no-default-features --features rustls,sqlite`) :

```sh
sqlx migrate add <description> --source api/migrations
```

> Les migrations sont **append-only** : ne jamais éditer une migration déjà
> livrée, en ajouter une nouvelle.

Inspecter la base de dev, au besoin :

```sh
sqlite3 weekmeals.db '.tables'
# Les UUID sont des blobs : les lire avec hex()
sqlite3 weekmeals.db 'select hex(id), title from recipes'
```

## Documentation

- [Plan & architecture](docs/plan.md) — modèle métier, structure du code, roadmap
- [ADR](docs/adr/) — décisions d'architecture
- [Brief design](docs/design/brief.md) — direction UX/UI

## Langue

Projet personnel francophone, à vocation open source : **docs en français,
code / schémas / routes en anglais**.

## Licence

[MIT](LICENSE)
