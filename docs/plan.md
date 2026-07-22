# Week Meals — Plan & architecture

## Vision

App de foyer (2 utilisateurs au départ) pour planifier les repas de la semaine
et faire les courses sans friction. Deux moments d'usage clés :

1. **Le dimanche, sur le canapé** — on feuillette les recettes et on remplit le
   calendrier de la semaine à deux.
2. **En magasin, une main sur le caddie** — on coche la liste de courses, souvent
   avec un réseau médiocre → l'app doit marcher hors-ligne sur cet écran.

## Modèle métier

- **Household** (foyer) — les utilisateurs appartiennent à un foyer ; toutes les
  données sont scopées au foyer. L'app est donc multi-foyers *by design*, sans
  travail supplémentaire pour les self-hosters.
- **User** — reconnu par **passkey** (WebAuthn) enrôlée sur son téléphone. Ni
  email, ni mot de passe. Voir [ADR-0006](adr/0006-auth-passkeys-appareils-enroles.md).
- **Device** — passkey enrôlée pendant une fenêtre d'onboarding ouverte au CLI :
  clé publique, libellé, dernière activité. Révocable.
- **Recipe** — titre, photo, temps prépa/cuisson, ingrédients (`quantity + unit`)
  et **étapes de préparation** (`steps` : liste ordonnée de texte).
  Unités : `g`, `kg`, `ml`, `l`, `piece`.
- **MealPlan** — calendrier : jour × créneau (`lunch` / `dinner`) → recette.
- **ShoppingList** — générée depuis une plage de dates du calendrier + ajouts
  manuels. Chaque entrée : nom, quantité, unité, `checked`. Les entrées cochées
  sont regroupées en bas (pas supprimées) ; action « vider les cochés ».
- **IngredientReference** — référentiel des poids moyens, seedé depuis
  [data/ingredients.yaml](../data/ingredients.yaml).

### Conversion grammes → unités (le cœur métier)

Service de domaine **pur** (zéro I/O, trivialement testable) :

1. Agréger les quantités d'un même ingrédient sur toutes les recettes planifiées
   (600 g + 300 g de courgettes → 900 g).
2. Convertir via le référentiel : `900 g ÷ 250 g/courgette → 4 courgettes`
   (arrondi **supérieur**).
3. Les ingrédients « comptables » (œufs…) restent en pièces ; les vracs
   (farine, lait…) restent en g / L — un flag `countable` dans le référentiel.

## Architecture du code

Monorepo, **découpage vertical par domaine** ([ADR-0005](adr/0005-decoupage-vertical-par-domaine.md)).
Côté Rust, chaque domaine métier est une **crate** qui porte ses propres couches
(`domain` / `application` / `infrastructure` / `presentation`) sous forme de
modules internes. Un `kernel` pur héberge les types transverses ; le binaire
`server` compose les routers de chaque domaine.

```
week-meals/
├── api/                     # Rust — workspace
│   ├── kernel/              # noyau pur, partagé (VO communs, IDs, erreurs)
│   ├── auth/                # passkeys (WebAuthn), foyer, appareils enrôlés
│   ├── recipes/             # recettes (CRUD, photos, import/export)
│   ├── meal-plan/           # calendrier midi/soir
│   ├── shopping-list/       # liste + conversion grammes→unités (cœur métier)
│   └── server/              # binaire HTTP (Axum), compose les domaines
├── web/                     # React + Vite + TS, PWA
├── data/
│   ├── ingredients.yaml     # référentiel poids moyens (versionné)
│   └── recipes/*.yaml       # seed de recettes (cible du scraping)
└── docs/
    ├── adr/
    └── design/
```

Chaque crate de domaine s'organise de la même façon (modules internes) :

```
recipes/src/
├── domain/          # entités, VO, traits de repository, services purs
├── application/     # use cases : commands (écritures) + queries (lectures)
├── infrastructure/  # implémentations (repos SQLx, adapters)
└── presentation/    # routes Axum, DTO
```

Conventions (pattern Action → Domain → Response) :

- Un controller/route = une action, qui appelle un **Handler**.
- Le Handler retourne toujours un **Response object**, jamais d'exception qui
  remonte à la présentation.
- Traits de repository déclarés dans `domain`, implémentés dans `infrastructure`.
- **Règle de couches (par convention)** : le module `domain` reste pur — il
  n'importe ni SQLx ni Axum. Non opposable par le compilateur au sein d'une
  crate ; garantie par la revue.

### Recettes : DB + seed versionné ([ADR-0003](adr/0003-recettes-db-plus-seed.md))

La **DB est la source de vérité** (édition fluide depuis le mobile). Les fichiers
YAML de `data/recipes/` servent de seed initial et de cible pour du scraping ;
une commande d'import/export fait le pont.

### Offline ([ADR-0004](adr/0004-offline-liste-courses.md))

Seule la **liste de courses** est offline-first : cache IndexedDB + file de
mutations rejouée au retour du réseau. Conflits en *last-write-wins* (suffisant
à 2 utilisateurs). Le reste de l'app est online-only.

## Stack & hébergement (0 €)

| Composant | Choix | Notes |
|---|---|---|
| Backend | Rust — Axum + SQLx + Tokio | `webauthn-rs`, sessions cookie (`tower-sessions`) |
| Frontend | React + Vite + TS | TanStack Query/Router, `vite-plugin-pwa`, Dexie |
| BDD | SQLite — un fichier sur volume Fly | Répliqué vers R2 (Litestream) — [ADR-0008](adr/0008-sqlite-volume-fly.md) |
| Photos | Cloudflare R2 (10 Go gratuits) | Upload via URL présignée |
| Front hosting | Cloudflare Pages | |
| API hosting | Scaleway Serverless Containers | Scale-to-zero, cold start ~1-2 s acceptable |
| CI/CD | GitHub Actions | fmt + clippy + tests ; déploiement Fly automatique sur `main` |

Détail et alternatives : [ADR-0001](adr/0001-stack-rust-axum-scaleway.md), amendé
depuis sur l'hébergement par [ADR-0007](adr/0007-hebergement-fly-mono-app.md)
(une seule app Fly, front et API) et sur la base par
[ADR-0008](adr/0008-sqlite-volume-fly.md).

## Qualité

- Tests unitaires sur le domaine — la conversion est le cœur à blinder.
- Tests d'intégration contre une vraie base — un fichier SQLite jetable par
  test, donc sans service à lancer ni test `#[ignore]`.
- Vitest côté front.
- `docker-compose` pour le dev local, `.env.example`, README self-host.

## Roadmap

Chaque jalon livre quelque chose d'utilisable.

1. **Socle** — workspace Rust, CI, docker-compose, auth (passkeys) + household + appareils
2. **Recettes** — CRUD + photos R2 + import/export `data/recipes/`
3. **Calendrier** — planification midi/soir
4. **Liste de courses** — génération + conversion + édition + check
5. **Offline** — cache + sync de la liste
6. **Prod & bonus** — déploiement, polish PWA, scraping de recettes vers les YAML
