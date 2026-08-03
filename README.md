# Week Meals

PWA mobile-first pour un foyer : gérer ses recettes, planifier les repas de la
semaine (midi / soir) et générer une liste de courses intelligente.

**La spécificité :** la génération de la liste de courses parle un **dictionnaire
d'ingrédients** versionné ([data/ingredients.yaml](data/ingredients.yaml)). Il
rapproche les formulations des recettes d'un nom canonique — « Courgettes »,
« grosses courgettes bio » et « courgettes » ne font qu'une ligne, tandis que
« poivron rouge » et « poivron jaune » gardent la leur — puis convertit les
grammages en unités achetables : `600 g de courgettes` devient `3 courgettes`.

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
- 🏬 **Tri par magasin** — la liste se découpe en rayons, dans l'ordre de visite
  du magasin où l'on va (paramétrable magasin par magasin)
- 👥 **Foyer** — auth par **passkeys** (Face ID / empreinte), **aucun email ni
  mot de passe**, enrôlement des appareils par code d'appairage (open-source
  friendly : zéro donnée perso à configurer)

## Stack

| Composant | Choix |
|---|---|
| Backend | Rust — Axum + SQLx (clean architecture, crates par couche) |
| Frontend | React + Vite + TypeScript, PWA (offline via IndexedDB) |
| BDD | SQLite — un fichier, sur un volume Fly en prod (Litestream → R2) |
| Photos | Volume Fly en prod, ou Cloudflare R2 si configuré (cf. ADR-0009) |
| Hébergement | Fly.io — une seule app : l'Axum sert l'API (`/api`) et le front |

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

Les routes de l'API sont servies sous le préfixe **`/api`**
(cf. [ADR-0007](docs/adr/0007-hebergement-fly-mono-app.md)) : `VITE_API_URL`
doit donc l'inclure (`http://localhost:8080/api` en dev).

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

weekmeals seed-ingredients           # dictionnaire d'ingrédients (global)
```

Le **dictionnaire d'ingrédients** ([data/ingredients.yaml](data/ingredients.yaml))
est global (pas par foyer). Il donne à chaque produit un nom canonique, ses
synonymes, son rayon, son unité d'achat et, le cas échéant, le poids moyen d'une
pièce. Il alimente quatre choses : le rapprochement des formulations, la
conversion grammes → unités, les suggestions de la barre de saisie et le **tri
par rayon** en magasin.

Il ne contient d'ailleurs pas que de l'alimentaire : ce qui va dans le même
caddie y a sa place (papier toilette, lessive, piles, croquettes…). Ces produits
ne viennent jamais d'une recette, mais y figurer leur donne l'auto-complétion,
la bonne unité et le bon rayon.

`seed-ingredients` fait un upsert par nom : le rejouer après avoir édité le
fichier met simplement la base à jour. **Le serveur le rejoue aussi à chaque
démarrage** (chemin surchargeable par `INGREDIENTS_FILE`), pour qu'un
déploiement n'oublie jamais le vocabulaire de sa version.

Le rapprochement d'un nom au dictionnaire est tolérant, du plus strict au plus
souple : nom exact, puis clé canonique (casse, accents, ponctuation, mots vides,
pluriels), puis clé produit (les qualificatifs qui ne changent pas l'achat —
« bio », « grosses », « bien mûres »… — sont retirés), puis proximité
orthographique (« échalotte » → « échalote »). Un ingrédient absent du
dictionnaire reste utilisable tel quel : il garde son nom et son unité, sans
rayon ni conversion.

Ce que le rapprochement ne fait **jamais**, c'est confondre deux produits
différents : une **couleur nomme une variété**, pas un état. « poivron rouge »,
« poivron jaune » et « poivron » sont trois courses distinctes, et chacune a son
entrée au dictionnaire — comme « oignon rouge », « chou rouge », « abricot sec »
ou « lait de coco ». Une déclinaison qui n'y figure pas garde simplement le nom
saisi, sans rayon.

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
`POST /api/recipes/scrape` est donc gardé contre le **SSRF** (https uniquement, IP
publiques vérifiées et épinglées, redirections coupées, taille bornée).

##### Instagram

Les liens de publication Instagram (`/p/…`, `/reel/…`, `/tv/…`, avec ou sans
paramètres de partage) sont acceptés eux aussi. Ces pages n'ont pas de JSON-LD :
la recette est dans la **légende**, qu'on lit sur la page `embed` publique du
post avant de la découper à l'heuristique — titre en première ligne, sections
« Ingrédients » / « Préparation », puces et numérotations retirées, hashtags et
« abonne-toi » jetés, « pour N personnes » et « prépa 10 min » repris dans les
champs correspondants. Sans intertitre, la plus longue suite de lignes ayant la
forme d'ingrédients (puce ou quantité en tête) fait office de liste.

Deux limites à connaître : une légende sans le moindre ingrédient est refusée
(« aucune recette n'a été trouvée sur cette page »), et une recette qui n'existe
que dans la vidéo ne peut pas être importée. Instagram peut aussi refuser de
servir la page à un serveur, l'import ressort alors en « page injoignable ».

##### La photo est rapatriée

Un brouillon d'import désigne sa photo par l'URL du site d'origine, qui ne tient
pas dans le temps : une image déplacée, et la fiche affiche un cadre vide ; côté
Instagram c'est une certitude, le CDN signe ses URLs avec une date d'expiration.
L'API la **télécharge donc à l'import** et la range dans le stockage photo
(volume ou R2, cf. ADR-0009), exactement comme un upload manuel — la recette
garde une URL à nous.

Le téléchargement passe par la **même garde SSRF** que le scraping (l'URL vient
d'une page que personne ne contrôle), il est plafonné à 8 Mo comme le dépôt
client, et le format est déduit des **octets** et non de l'en-tête distant : une
page d'erreur servie en `image/jpeg` n'a aucune chance de finir rangée comme
photo. Tout est *best-effort* : stockage non configuré, image injoignable, trop
lourde ou d'un format non pris en charge, et le brouillon repart avec l'URL
distante — l'import ne doit jamais échouer à cause d'une photo. En CLI, où il n'y
a pas de stockage, `weekmeals scrape` garde l'URL distante dans le YAML.

### Rayons et magasins (cf. [ADR-0010](docs/adr/0010-rayons-catalogue-ferme-magasins.md))

Le `category` d'un ingrédient est un **rayon**, pris dans un catalogue fermé
défini côté serveur (`shopping-list::domain::aisle`, exposé par `GET /api/aisles`) :
fruits, légumes, boulangerie, boucherie, charcuterie, poissonnerie, crèmerie,
surgelés, épicerie salée, épicerie sucrée, condiments, boissons, hygiène,
entretien, maison, bébé, animaux. Le découpage suit les **allées d'un magasin**,
pas les familles d'aliments : les herbes fraîches sont au rayon légumes quand le
thym séché est aux condiments, et la pâte feuilletée est au frais.

Un **magasin** (Paramètres → Magasins) n'est pas un catalogue de produits mais un
*trajet* : un nom, et l'ordre dans lequel on en traverse les rayons — chez l'un
les surgelés sont à l'entrée, chez l'autre juste avant les caisses. L'onglet
Courses propose alors de trier la liste dans cet ordre, section par section. Le
magasin choisi est gardé **sur l'appareil** : dans un foyer, chacun ne fait pas
ses courses au même endroit.

Le tri n'est qu'un affichage — il ne réordonne rien côté serveur, et l'ordre
manuel (glisser-déposer) reste disponible. Un article dont le rayon est inconnu
du magasin, ou qui n'en a pas, atterrit dans une section « Autres » en fin de
parcours : rien ne disparaît jamais d'une liste de courses.

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

En mode `locked`, front et API doivent partager le même domaine
(`WEBAUTHN_RP_ID` / `WEBAUTHN_RP_ORIGIN`) — ce que le déploiement mono-app
garantit d'office. Avant tout déploiement, remplacer la valeur `change-me` de
`SESSION_SECRET` (en prod : `fly secrets set`, jamais dans `fly.toml`).

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

## Déploiement (Fly.io)

Une **seule app** Fly sert l'API sous `/api` et le front en statique
(cf. [ADR-0007](docs/adr/0007-hebergement-fly-mono-app.md)) : même origine, donc
ni CORS ni cookie `SameSite=None`, et les passkeys fonctionnent directement sur
le domaine `.fly.dev`. Les migrations sont jouées **au démarrage** du serveur —
pas d'étape `sqlx-cli` à part.

La base est un fichier SQLite sur un **volume Fly** monté en `/data`
(cf. [ADR-0008](docs/adr/0008-sqlite-volume-fly.md)), répliqué en continu vers
R2 par Litestream.

### Première mise en ligne

```sh
fly auth login
fly launch --no-deploy --copy-config   # reprend fly.toml sans l'écraser
fly volumes create weekmeals_data --region cdg --size 1
```

> **Une seule machine.** Le volume appartient à une machine : en scaler une
> seconde lui donnerait sa propre base, et les deux divergeraient en silence.
> `max_machines_running = 1` le verrouille dans `fly.toml`.

Provisionner un bucket **R2** pour les sauvegardes de la base, puis injecter les
secrets — `fly.toml` ne contient que du non-sensible, et plus d'URL de base de
données du tout :

```sh
fly secrets set \
  SESSION_SECRET="$(openssl rand -base64 64)" \
  WEB_ORIGIN='https://week-meals.fly.dev' \
  WEBAUTHN_RP_ID='week-meals.fly.dev' \
  WEBAUTHN_RP_ORIGIN='https://week-meals.fly.dev' \
  LITESTREAM_ENDPOINT='https://<account>.r2.cloudflarestorage.com' \
  LITESTREAM_BUCKET='week-meals-backups' \
  LITESTREAM_ACCESS_KEY_ID='…' \
  LITESTREAM_SECRET_ACCESS_KEY='…'
```

> Sans `LITESTREAM_BUCKET`, l'app démarre quand même — **sans réplication**.
> C'est pratique pour un dépannage, jamais pour un déploiement durable : le
> volume seul n'est sauvegardé que par les snapshots quotidiens de Fly.

> **Photos.** Elles sont écrites sur le volume (`PHOTO_STORAGE_DIR=/data/photos`,
> fixé dans `fly.toml`) — aucun secret à poser (cf.
> [ADR-0009](docs/adr/0009-photos-volume-fly.md)). Elles ne sont **pas**
> répliquées par Litestream : perdre le volume, c'est perdre les photos, pas la
> base. Pour les mettre sur R2 à la place, poser les `R2_*` (endpoint, région,
> bucket, clés, `R2_PUBLIC_BASE_URL`) en secrets : R2 reprend alors la main.

```sh
fly deploy
```

> `AUTH_MODE=locked` est fixé dans `fly.toml` : l'app est **fermée** tant
> qu'aucun appareil n'est enrôlé. C'est voulu (fail-closed).

### Enrôler le premier appareil

L'app étant verrouillée, la fenêtre d'enrôlement s'ouvre depuis le conteneur —
la CLI `weekmeals` est présente dans l'image :

```sh
fly ssh console -C "weekmeals device open-window --minutes 15"
```

Saisir le code d'appairage affiché dans l'écran de connexion, depuis l'appareil
à enrôler. Ensuite : `weekmeals device list` / `revoke <id>` / `close-window`.

### Seed des recettes

Le dictionnaire d'ingrédients est chargé au démarrage du serveur : il n'y a rien
à lancer à la main. Les recettes de démonstration, si on les veut :

```sh
fly ssh console -C "weekmeals seed --dir /app/data/recipes"
```

### Déploiements suivants — automatiques

Une fois la première mise en ligne faite, **tout merge sur `main` déploie**
(job `deploy` de [`ci.yml`](.github/workflows/ci.yml)), à condition que fmt,
clippy et les tests soient au vert. Le build tourne sur les builders Fly
(`--remote-only`), pas sur le runner GitHub.

Deux choses à provisionner une fois pour que ça marche :

```sh
fly tokens create deploy -x 8760h   # jeton de déploiement, valable un an
```

puis le coller dans les secrets du dépôt sous le nom **`FLY_API_TOKEN`**
(Settings → Secrets and variables → Actions). Le job cible l'environnement
GitHub `production` : le créer permet d'y exiger une approbation manuelle avant
chaque déploiement, mais il fonctionne sans.

Les déploiements ne se chevauchent jamais (`concurrency: deploy-fly`) — l'app
n'a qu'une machine et un volume. Pour déployer à la main malgré tout :

```sh
fly deploy
```

### Vérifier en local avant de pousser

L'image se construit et se teste sans Fly :

```sh
docker build -t week-meals .
docker run --rm -p 8080:8080 \
  -v week-meals-data:/data \
  -e AUTH_MODE=disabled \
  week-meals
```

### Sauvegarde et restauration

Litestream réplique le WAL en continu ; l'état des sauvegardes se lit depuis le
conteneur :

```sh
fly ssh console -C "litestream snapshots /data/weekmeals.db"
```

Restaurer écrase la base : arrêter l'app d'abord, et restaurer à côté pour
vérifier avant de remplacer.

```sh
fly ssh console
litestream restore -o /data/verif.db /data/weekmeals.db          # dernier état
litestream restore -o /data/verif.db -timestamp 2026-07-20T10:00:00Z \
    /data/weekmeals.db                                            # à une date
sqlite3 /data/verif.db 'select count(*) from recipes'
```

> Une restauration jamais essayée n'est pas une sauvegarde. La commande
> ci-dessus, dans un fichier à côté, ne casse rien : l'exécuter une fois après
> la mise en ligne.

## Documentation

- [Plan & architecture](docs/plan.md) — modèle métier, structure du code, roadmap
- [ADR](docs/adr/) — décisions d'architecture
- [Brief design](docs/design/brief.md) — direction UX/UI

## Langue

Projet personnel francophone, à vocation open source : **docs en français,
code / schémas / routes en anglais**.

## Licence

[MIT](LICENSE)
