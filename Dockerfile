# Image mono-conteneur Week Meals (cf. ADR-0007) : le binaire Axum sert l'API
# sous `/api` et le build du front en statique. Un seul déploiement, une seule
# origine — donc pas de CORS ni de cookie `SameSite=None` en prod.
#
# La base est un fichier SQLite sur un volume (cf. ADR-0008), répliqué vers R2
# par Litestream. Sans `LITESTREAM_BUCKET`, le conteneur démarre quand même,
# sans réplication.
#
#   docker build -t week-meals .
#   docker run -p 8080:8080 -v week-meals-data:/data \
#       -e DATABASE_URL=sqlite:///data/weekmeals.db week-meals

# ---------------------------------------------------------------- front (Vite)
FROM node:22-alpine AS web
WORKDIR /web

# `npm ci` avant de copier les sources : la couche est réutilisée tant que les
# dépendances ne bougent pas.
COPY web/package.json web/package-lock.json ./
RUN npm ci

COPY web/ ./
# Same-origin : une URL relative suffit et évite de rebuilder l'image par
# environnement (l'URL Fly n'a pas à être connue au build).
ENV VITE_API_URL=/api
RUN npm run build

# ------------------------------------------------------------------ API (Rust)
FROM rust:1.97-slim-bookworm AS api
WORKDIR /api

# OpenSSL est requis par `webauthn-rs` (via `webauthn-attestation-ca`), qui ne
# sait pas fonctionner en rustls pur — le reste de la stack (sqlx, reqwest,
# rust-s3) est bien en rustls.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Aucune macro sqlx compile-time dans le projet : la compilation n'a besoin
# d'aucune base de données (vérifié — pas de `query!`/`query_as!`). SQLite est
# compilé dans le binaire (feature `sqlite` de SQLx), d'où le gcc déjà présent
# dans l'image Rust et rien à installer au runtime.
COPY api/ ./
RUN cargo build --release --bin server --bin weekmeals

# ------------------------------------------------------------------ Litestream
# Version épinglée : c'est un binaire téléchargé, il ne doit pas bouger sous nos
# pieds d'un build à l'autre. `TARGETARCH` est fourni par BuildKit — l'image se
# construit donc aussi bien pour le builder amd64 de Fly que sur un Mac ARM.
FROM debian:bookworm-slim AS litestream
ARG LITESTREAM_VERSION=0.3.13
ARG TARGETARCH
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && curl -fsSL -o /tmp/litestream.tar.gz \
        "https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/litestream-v${LITESTREAM_VERSION}-linux-${TARGETARCH}.tar.gz" \
    && tar -xzf /tmp/litestream.tar.gz -C /usr/local/bin litestream \
    && rm -rf /var/lib/apt/lists/* /tmp/litestream.tar.gz

# -------------------------------------------------------------------- runtime
FROM debian:bookworm-slim AS runtime

# `ca-certificates` : TLS sortant vers R2 (photos et réplication) et les sites
# scrapés à l'import de recette. `libssl3` : lié dynamiquement par webauthn-rs.
# `util-linux` fournit `setpriv`, dont l'entrypoint se sert pour abandonner les
# privilèges root après avoir préparé le volume.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 weekmeals

WORKDIR /app

COPY --from=api /api/target/release/server /usr/local/bin/server
COPY --from=api /api/target/release/weekmeals /usr/local/bin/weekmeals
COPY --from=litestream /usr/local/bin/litestream /usr/local/bin/litestream
COPY --from=web /web/dist /app/web
# Référentiel d'ingrédients et recettes de seed : utilisés par la CLI
# (`weekmeals seed`, `weekmeals reference import`) via `fly ssh console`.
# Attention à ne pas confondre `/app/data` (ces graines, dans l'image) avec
# `/data` (le volume, où vit la base).
COPY data/ /app/data/
COPY litestream.yml /etc/litestream.yml
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# `DB_PATH` désigne le même fichier que `DATABASE_URL`, sous la forme d'un
# chemin nu : c'est ce qu'attendent Litestream (qui n'expanse pas les valeurs
# par défaut, cf. litestream.yml) et l'entrypoint.
ENV WEB_DIST=/app/web \
    PORT=8080 \
    DB_PATH=/data/weekmeals.db \
    DATABASE_URL=sqlite:///data/weekmeals.db

# Pas de `USER` : l'entrypoint démarre en root pour donner le volume monté à
# l'utilisateur applicatif, puis abandonne ses privilèges (cf. le script).
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
