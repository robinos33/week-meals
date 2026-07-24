# ADR-0009 — Photos des recettes sur le volume Fly plutôt que R2

- **Statut :** acceptée (2026-07-24)
- **Complète** [ADR-0007](0007-hebergement-fly-mono-app.md) (mono-app Fly) et
  [ADR-0008](0008-sqlite-volume-fly.md) (base SQLite sur volume) : les photos
  vont sur le **même volume** que la base, à côté d'elle.
- **N'annule pas** le port `PhotoStorage` ni son implémentation S3 (R2/MinIO) :
  celle-ci reste, et **prime** quand elle est configurée.

## Contexte

L'upload de photos avait été pensé pour un **stockage S3-compatible** : l'API
présigne une URL, le client dépose le fichier directement dessus (PUT), puis
persiste l'URL publique dans `recipes.photo`. Aucun octet ne transite par l'API.
C'est la bonne architecture pour Cloudflare R2 (prod) ou MinIO (dev).

Sauf qu'**en prod, R2 n'a jamais été branché**. Le déploiement Fly ne définit
aucune des variables `R2_*`, donc `photo_storage_from_env()` renvoie `None`,
`POST /api/recipes/photos/presign` répond `503` et le front retombe sur « collez
une URL ». Résultat concret : **on ne peut pas uploader de photo**.

Deux façons de débloquer :

1. **Provisionner R2 pour les photos.** Un bucket, un domaine public, quatre
   secrets de plus, une politique CORS à tenir — pour *quelques images* d'un
   seul foyer. C'est la surface que [ADR-0007](0007-hebergement-fly-mono-app.md)
   et [ADR-0008](0008-sqlite-volume-fly.md) venaient justement de réduire.
2. **Écrire les images sur le volume Fly déjà monté** (`/data`), là où vit déjà
   la base. Zéro fournisseur, zéro secret, zéro CORS (même origine).

## Décision

**Les photos sont stockées sur le volume Fly**, sous `/data/photos`, via une
seconde implémentation du port `PhotoStorage` : `VolumePhotoStorage`.

Le port ne change pas. `presign_upload` reste le point d'entrée ; pour le volume
il génère une clé opaque `<uuid>.<ext>` et renvoie des URLs **servies par notre
propre API** :

- `upload_url` = `/api/recipes/photos/<clé>?token=<jeton>` — le client y dépose
  le fichier par `PUT`, exactement comme vers R2 ;
- `public_url` = `/api/recipes/photos/<clé>` — stockée dans `recipes.photo`.

Deux routes nouvelles portent ce backend :

- `PUT /recipes/photos/{filename}` écrit les octets sur le volume ;
- `GET /recipes/photos/{filename}` sert le fichier.

### Sélection du backend — R2 prioritaire, volume en repli

`photo_storage_from_env()` choisit, dans l'ordre :

1. **R2** si les `R2_*` sont présents (aucune régression pour qui le configure) ;
2. sinon **le volume** si `PHOTO_STORAGE_DIR` est défini (`/data/photos` en prod) ;
3. sinon **rien** (`503`, comportement actuel — dev sans config).

La cohabitation garde l'option object-storage ouverte sans la rendre obligatoire.

### Autorisation du dépôt — un jeton, pas un cookie

Avec R2, l'URL présignée *est* l'autorisation : signée, expirante, sans cookie.
Le front fait donc un `PUT` **sans `credentials`**. Pour ne rien changer côté
front, le volume reproduit cette sémantique : `presign_upload` (qui exige déjà
une session — route auth-gated) émet un **jeton opaque à usage unique**, valide
900 s, mémorisé en process et lié à *cette* clé. Le `PUT` n'est accepté que sur
présentation du jeton ; il est consommé au premier usage.

Le jeton vit **en mémoire du process**, pas en base :

- la fenêtre presign→PUT se compte en secondes, bien en deçà des 900 s ;
- l'app tourne sur **une seule machine** ([ADR-0008](0008-sqlite-volume-fly.md)),
  donc pas de jeton à partager entre instances ;
- un redémarrage entre presign et PUT invalide le jeton — l'utilisateur relance
  l'upload, sans dommage.

### Garde-fous

- **Nom de fichier strict.** `<uuid v4>.<jpg|png|webp>` uniquement ; tout le
  reste (slash, `..`, autre extension) est rejeté avant de toucher au disque —
  pas de traversée de chemin, la clé servie est toujours celle générée côté
  serveur.
- **Taille bornée.** Le `PUT` plafonne le corps (8 Mio) : le volume fait 1 Gio,
  on ne le laisse pas remplir par un fichier.
- **Cache.** Le `GET` sert en `Cache-Control: public, max-age=31536000,
  immutable` — l'URL est adressée par UUID, son contenu ne change jamais.

## Conséquences

### Ce qu'on accepte

- **Les octets transitent par l'API** au dépôt et au service, contrairement au
  presign R2. À l'échelle d'un foyer et de quelques images de quelques centaines
  de Kio, c'est négligeable ; la machine Fly n'est de toute façon réveillée que
  pendant l'usage ([ADR-0001](0001-stack-rust-axum-scaleway.md)).
- **Les photos ne sont pas répliquées.** Litestream réplique **la seule base
  SQLite** vers R2 ([ADR-0008](0008-sqlite-volume-fly.md)) ; les fichiers du
  volume, non. **Perdre le volume, c'est perdre les photos** — la base, elle,
  survit, et une recette sans photo reste une recette (le front affiche 🍽️).
  C'est le compromis assumé : la donnée qui compte (recettes, planning) est
  durable ; l'image est agréable mais reconstituable. Si un jour ça ne suffit
  plus, R2 est à un `fly secrets set` de distance — le port est déjà là.

### Ce qui reste ouvert

- **Nettoyage des orphelins.** Remplacer la photo d'une recette laisse l'ancien
  fichier sur le volume. Négligeable au volume attendu ; un balayage
  périodique pourra venir si besoin, hors périmètre ici.
