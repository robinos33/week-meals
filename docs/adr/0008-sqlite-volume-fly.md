# ADR-0008 — SQLite sur volume Fly plutôt que Postgres managé

- **Statut :** acceptée (2026-07-22)
- **Remplace** la partie « base de données » de
  [ADR-0001](0001-stack-rust-axum-scaleway.md) (PostgreSQL managé, Neon). Le
  reste — Rust/Axum, React/Vite, R2 pour les photos — est confirmé.
- **Suppose** l'hébergement sur Fly.io, décidé séparément par
  [ADR-0007](0007-hebergement-fly-mono-app.md) : le volume dont il est question
  ici est un volume Fly.

## Contexte

Le déploiement rassemble le front et l'API dans un seul conteneur Fly
([ADR-0007](0007-hebergement-fly-mono-app.md)). Il restait un fournisseur extérieur pour la base : **Neon**. À
l'échelle du projet — un foyer, quelques centaines de recettes, un écrivain à la
fois — ce Postgres managé coûte plus qu'il ne rapporte :

- **Un fournisseur et un secret de plus** à gérer, alors que le passage à Fly
  venait précisément de réduire la surface à un seul déploiement.
- **Deux réveils en série** au démarrage à froid : la machine Fly, puis
  l'instance Neon qui se suspend elle aussi après inactivité. Le
  `scale-to-zero` paie deux fois.
- **Les tests d'intégration exigent un Postgres réel** : les trois flux
  (`auth_flow`, `recipes_flow`, `meal_plan_flow`) sont marqués `#[ignore]` et ne
  tournent jamais sans `docker compose up`. Une partie du code SQL n'est donc
  vérifiée par personne au quotidien.
- **Rien dans le domaine n'a besoin de Postgres** : pas de recherche
  plein-texte, pas de type exotique, pas de concurrence en écriture.

## Décision

**SQLite, dans un fichier posé sur un volume Fly**, monté sur `/data` et attaché
à l'unique machine de l'app. `DATABASE_URL=sqlite:///data/weekmeals.db`.

La durabilité ne repose pas sur le volume seul : **Litestream** réplique le WAL
en continu vers le bucket **R2 déjà provisionné** pour les photos. Un volume
Fly vit sur un hôte précis ; sans réplication, perdre l'hôte c'est perdre la
base entre deux snapshots quotidiens.

### Conventions de stockage

SQLite n'a pas les types de Postgres ; les correspondances sont fixées ici une
fois pour toutes, parce qu'elles doivent être **cohérentes entre l'écriture et
la lecture** (SQLite ne vérifie rien pour nous).

| Postgres | SQLite | Côté Rust |
|---|---|---|
| `uuid` | `blob` (16 octets) | `Uuid` — encodage natif SQLx |
| `timestamptz` | `text` | `DateTime<Utc>` |
| `date` | `text` (`YYYY-MM-DD`) | `NaiveDate` |
| `jsonb` | `text` | `String` (la passkey était déjà manipulée en JSON brut) |
| `bytea` | `blob` | `Vec<u8>` |
| `boolean` | `integer` (0/1) | `bool` |
| `double precision` | `real` | `f64` |
| `smallint` | `integer` | `i16` |

Les UUID sont des **blobs** et non du texte : c'est l'encodage natif de SQLx,
donc zéro conversion à chaque `bind`. En contrepartie ils ne sont pas lisibles
tels quels depuis `sqlite3` — utiliser `hex(id)` (et `x'…'` pour les
littéraux, comme dans la migration du foyer de démo).

### Ce qui n'existe pas en SQLite et a dû être réécrit

- **`where … = any($1)`** (chargement groupé des ingrédients et des étapes) :
  remplacé par une liste de `?` construite à la volée.
- **`ilike` et `order by lower(title)`** : le `LIKE` de SQLite n'est
  insensible à la casse qu'en ASCII, et le tri se fait en ordre d'octets — « Éclair »
  passerait après « Zeste ». Les recettes portent donc une colonne
  `title_norm` (minuscules **et** accents dépliés, calculée en Rust) qui sert à
  la fois à la recherche et au tri. C'est plus juste qu'avant : la recherche
  trouve maintenant « crème » en tapant « creme ».
- **CTE modificatrice** (`with … as (update … returning …)`, compteur
  « cuisiné X fois ») : SQLite n'autorise pas d'`update` dans un `with`. Éclatée
  en deux ordres dans une transaction — incrémenter les compteurs d'abord,
  marquer `counted_at` ensuite, l'ordre porte la correction.
- **`delete … using`** : remplacé par un `delete … where user_id in (select …)`.
- **Nom de contrainte dans l'erreur** : `meal-plan` distinguait « recette hors
  foyer » (404) d'une panne (500) en lisant le nom de la FK violée, information
  que SQLite ne donne pas. Le repository vérifie donc explicitement
  l'appartenance de la recette avant d'écrire.
- **`alter table … add constraint` / `drop constraint`** : inexistants. Les
  contraintes d'unicité concernées passent par des `create unique index` /
  `drop index` nommés.

### Réglages de connexion

Non négociables, appliqués à l'ouverture du pool :

- `foreign_keys = ON` — **désactivé par défaut** dans SQLite. Sans lui, tous les
  `on delete cascade` du schéma sont décoratifs.
- `journal_mode = WAL` — lecteurs et écrivain ne se bloquent plus, et c'est ce
  que Litestream réplique.
- `busy_timeout = 5s` — au lieu d'un `SQLITE_BUSY` immédiat.
- `synchronous = NORMAL` — le compromis recommandé avec WAL.

## Conséquences

- **Un fournisseur en moins.** Plus de `DATABASE_URL` secret : le chemin du
  fichier est en clair dans `fly.toml`. R2 reste le seul service externe.
- **Les tests d'intégration ne sont plus `#[ignore]`.** Ils créent un fichier
  temporaire, jouent les migrations et tournent partout, y compris en CI. C'est
  le gain le plus concret de la bascule : le SQL est enfin couvert par défaut.
- **Une seule machine, définitivement.** Chaque machine Fly aurait son propre
  volume, donc sa propre base : passer à deux machines ferait diverger les
  données en silence. `min_machines_running = 0` reste compatible (la machine
  s'arrête et redémarre avec son volume), mais l'app ne doit jamais être scalée
  horizontalement. Le verrou consultatif du migrateur SQLx, qui couvrait deux
  démarrages simultanés, n'a plus d'objet.
- **Une seule région.** Le volume est dans `cdg` ; l'app y est clouée.
- **La restauration est un geste à connaître, pas un bouton.** `litestream
  restore` depuis R2, documenté au README. Un backup jamais testé n'est pas un
  backup.
- **L'historique des migrations est réécrit.** Les huit fichiers existants sont
  transcrits en SQLite plutôt qu'empilés d'une neuvième migration de
  conversion : aucune base de production n'existe encore, une histoire
  Postgres qu'on ne rejouera jamais n'a rien à documenter. Les bases de dev
  locales sont donc à recréer (`weekmeals export` avant, `weekmeals seed`
  après).
- **Le plafond est bas mais lointain.** SQLite tient sans peine plusieurs Go et
  des milliers d'écritures par seconde en WAL. Ce qui casserait la décision,
  c'est un besoin de **plusieurs écrivains concurrents** ou d'accéder à la base
  depuis un autre processus que ce conteneur — ni l'un ni l'autre n'est au
  programme.

## Alternatives écartées

- **Rester sur Neon.** Fonctionne, mais garde le fournisseur, le secret, le
  double cold start et les tests `#[ignore]`.
- **Postgres auto-hébergé dans le même conteneur.** Cumule les inconvénients :
  un serveur à faire vivre dans 512 Mo, la même contrainte de volume unique,
  et aucune des simplifications de SQLite.
- **SQLite sans Litestream, en comptant sur les snapshots Fly.** Snapshots
  quotidiens, rétention courte : jusqu'à 24 h de planning et de recettes
  perdues pour économiser une variable d'environnement.
- **LiteFS** (réplication SQLite distribuée de Fly) : conçu pour plusieurs
  régions en lecture. Ici il n'y a qu'un foyer et une région — c'est de la
  complexité sans contrepartie.
