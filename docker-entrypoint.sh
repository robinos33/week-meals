#!/bin/sh
# Démarrage du conteneur Week Meals (cf. ADR-0008).
#
# La base est un fichier SQLite sur le volume Fly monté en /data. Ce script
# prépare ce volume, restaure la base depuis R2 si elle manque, puis lance le
# serveur — sous Litestream quand la réplication est configurée.
#
# Il démarre en root le temps de donner le volume à l'utilisateur applicatif
# (un volume Fly est monté root à sa création) et rend la main aussitôt :
# `setpriv` plutôt qu'un `USER` dans l'image, parce que ce chown doit avoir lieu
# après le montage, donc au démarrage. Le serveur et Litestream tournent en
# `weekmeals`, jamais en root.
set -e

DB_PATH="${DB_PATH:-/data/weekmeals.db}"

mkdir -p "$(dirname "$DB_PATH")"
chown -R weekmeals:weekmeals "$(dirname "$DB_PATH")"

DROP_PRIVS="setpriv --reuid=weekmeals --regid=weekmeals --clear-groups"

# Sans bucket configuré (dev local, `docker run` de dépannage), on se passe de
# réplication : la base vit et meurt alors avec le volume.
if [ -z "$LITESTREAM_BUCKET" ]; then
    echo "Litestream désactivé (LITESTREAM_BUCKET absent) — aucune réplication." >&2
    exec $DROP_PRIVS server
fi

# Machine neuve, volume vierge : on récupère la dernière réplique. Les deux
# gardes rendent l'appel sûr dans tous les cas — base déjà présente (redémarrage
# ordinaire) ou réplique inexistante (tout premier déploiement).
$DROP_PRIVS litestream restore -if-db-not-exists -if-replica-exists "$DB_PATH"

# `-exec` : Litestream surveille le WAL et supervise le serveur, dont il relaie
# le code de sortie. Un seul processus à superviser côté Fly.
exec $DROP_PRIVS litestream replicate -exec server
