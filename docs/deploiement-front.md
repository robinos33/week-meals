# Déploiement du front — Cloudflare Pages

Le front PWA (`web/`) est déployé sur **Cloudflare Pages** (hébergement statique
gratuit, CDN mondial). L'API vit ailleurs (Scaleway Serverless Containers,
cf. [ADR-0001](adr/0001-stack-rust-axum-scaleway.md)) : front et API sont donc
sur des **origines distinctes** en prod, ce qui a des conséquences sur les
cookies de session (voir plus bas).

## Build

| Réglage Pages          | Valeur          |
|------------------------|-----------------|
| Répertoire racine      | `web`           |
| Commande de build      | `npm run build` |
| Répertoire de sortie   | `dist`          |
| Version de Node        | 20              |

Le SPA est servi via [`web/public/_redirects`](../web/public/_redirects)
(`/* /index.html 200`), et le cache est réglé par
[`web/public/_headers`](../web/public/_headers) (SW/manifest en `no-cache`,
assets hashés immuables).

## URL de l'API par environnement

Le client lit `VITE_API_URL` **au build** (variable Vite). On la définit par
environnement, jamais en dur :

- **CI (GitHub Actions)** : variable de dépôt `VITE_API_URL`
  (Settings → Secrets and variables → Actions → *Variables*).
- **Build Pages (dashboard)** : variable d'environnement `VITE_API_URL`,
  distincte entre *Production* et *Preview* si besoin.

## Déploiement

Deux options, au choix :

1. **Intégration Git native** de Cloudflare Pages (recommandé) : connecter le
   dépôt, renseigner les réglages de build ci-dessus. Chaque push produit un
   déploiement (preview par branche, production sur `main`).
2. **GitHub Actions** : le workflow
   [`.github/workflows/deploy-web.yml`](../.github/workflows/deploy-web.yml)
   build puis `wrangler pages deploy`. Secrets requis : `CLOUDFLARE_API_TOKEN`
   (scope *Cloudflare Pages: Edit*) et `CLOUDFLARE_ACCOUNT_ID`.

En local : `cd web && npx wrangler pages deploy dist` (après `npm run build`).

## Cookies de session en cross-origin (important)

Front et API étant sur des domaines différents, le cookie de session doit être
`SameSite=None; Secure` pour être envoyé sur les requêtes `fetch` cross-site, et
l'API doit autoriser précisément l'origine du front. Côté **API**, régler donc :

```sh
WEB_ORIGIN=https://<votre-app>.pages.dev   # origine exacte du front (CORS)
SESSION_SAME_SITE=none                      # requis en cross-site
SESSION_SECURE=1                            # impose HTTPS (obligatoire avec None)
```

Ces clés sont décrites dans [`.env.example`](../.env.example) et lues par le
serveur (`server::Config`). Le front, lui, envoie déjà `credentials: "include"`.

## Vérifier l'install PWA + le service worker en prod

Après déploiement, sur l'URL Pages (HTTPS) :

1. **Lighthouse → PWA** (Chrome DevTools) : « Installable » au vert (manifest +
   icônes 192/512 + SW + HTTPS).
2. **Application → Manifest** : nom, thème, icônes (dont maskable) présents.
3. **Application → Service Workers** : `sw.js` *activated*. Recharger deux fois
   pour confirmer le fonctionnement hors-ligne (couper le réseau, l'app se
   charge depuis le cache).
4. **Mise à jour** : redéployer, rouvrir l'app → le bandeau « Une nouvelle
   version est disponible » doit apparaître (géré par `ReloadPrompt`).
5. **Install** : sur mobile Chromium, l'invite d'installation apparaît ;
   sur iOS, « Partager → Sur l'écran d'accueil ».
