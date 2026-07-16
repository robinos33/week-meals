# Week Meals — front PWA

React + Vite + TypeScript, PWA installable (mobile-first), direction visuelle
**« Cantine »** (vert potager, titres serif Fraunces, barre d'onglets basse).

## Développement

```sh
cd web
cp .env.example .env.local     # ajuster VITE_API_URL si besoin
npm install
npm run dev                    # http://localhost:5173
```

L'API doit tourner en parallèle (voir le README racine, section « Dev local »),
avec `WEB_ORIGIN=http://localhost:5173` pour autoriser le front via CORS.

## Scripts

| Script              | Rôle                                            |
|---------------------|-------------------------------------------------|
| `npm run dev`       | serveur de dev Vite                             |
| `npm run build`     | typecheck (`tsc -b`) + build de production      |
| `npm run preview`   | sert le build de `dist/`                         |
| `npm run typecheck` | vérification TypeScript seule                   |
| `npm run lint`      | ESLint                                          |
| `npm test`          | tests Vitest (`test:watch` en continu)          |
| `npm run gen:icons` | régénère les icônes PWA depuis `scripts/`       |

Les quatre premiers tournent en CI sur chaque PR touchant `web/`
(`.github/workflows/ci-web.yml`).

## Architecture

- **Design tokens** « Cantine » : `src/theme/tokens.css` (clair + sombre).
- **Coquille** : `src/components/AppShell.tsx` — barre d'onglets basse
  (`TabBar`), zones sûres (safe-area), invite d'installation (`InstallPrompt`)
  et bandeau de mise à jour du service worker (`ReloadPrompt`).
- **Routing** : TanStack Router (`src/router.tsx`). Les écrans vivent sous une
  route de mise en page sans chemin qui exige une session (`src/api/session.ts`,
  `GET /auth/me`) : un écran ajouté dessous est protégé par défaut, et un 401
  renvoie vers `/login` en mémorisant l'écran demandé.
- **Données** : TanStack Query (`src/query.ts`) via le client `src/api/client.ts`
  (cookies de session, `credentials: "include"`).
- **PWA** : `vite-plugin-pwa` (manifest, icônes, service worker) — cf.
  `vite.config.ts`.

## Déploiement

Cloudflare Pages (cf. [ADR-0001](../docs/adr/0001-stack-rust-axum-scaleway.md)) —
la configuration et la procédure arrivent avec #23.
