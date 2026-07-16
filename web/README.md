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

| Script            | Rôle                                              |
|-------------------|---------------------------------------------------|
| `npm run dev`     | serveur de dev Vite                               |
| `npm run build`   | typecheck (`tsc -b`) + build de production        |
| `npm run preview` | sert le build de `dist/`                           |
| `npm run typecheck` | vérification TypeScript seule                   |
| `npm run gen:icons` | régénère les icônes PWA depuis `scripts/`       |

## Architecture

- **Design tokens** « Cantine » : `src/theme/tokens.css` (clair + sombre).
- **Thème** : `src/theme/ThemeProvider.tsx` — bascule clair/système/sombre,
  respecte `prefers-color-scheme`, persistée.
- **Coquille** : `src/components/AppShell.tsx` — barre d'onglets basse
  (`TabBar`), zones sûres (safe-area), invite d'installation (`InstallPrompt`)
  et bandeau de mise à jour du service worker (`ReloadPrompt`).
- **Routing** : TanStack Router (`src/router.tsx`). **Données** : TanStack Query
  (`src/query.ts`) via le client `src/api/client.ts` (cookies de session).
- **PWA** : `vite-plugin-pwa` (manifest, icônes, service worker) — cf.
  `vite.config.ts`.

## Déploiement

Cloudflare Pages — voir [`docs/deploiement-front.md`](../docs/deploiement-front.md).
