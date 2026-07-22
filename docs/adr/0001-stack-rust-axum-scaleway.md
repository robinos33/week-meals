# ADR-0001 — Stack : Rust/Axum + React, hébergement 0 €

- **Statut :** acceptée (2026-07-11), **partiellement remplacée** (2026-07-22)
  par [ADR-0007](0007-hebergement-fly-mono-app.md) — l'hébergement passe de
  Scaleway + Cloudflare Pages à une app Fly.io unique — et par
  [ADR-0008](0008-sqlite-volume-fly.md) — la base passe de Neon à SQLite sur
  volume Fly. Le reste (Rust/Axum, React/Vite, R2) reste en vigueur.

## Contexte

Trafic très faible (un foyer), budget cible : **0 €/mois**. Objectif secondaire
assumé : **apprendre Rust** sur un vrai projet. Le projet sera open source —
la qualité du code (DDD / clean archi, tests) compte.

## Options considérées

1. **Rust « réel » — Axum + SQLx, conteneur Scaleway** ✅
   Rust idiomatique complet (Tokio, traits, error handling), écosystème mature,
   se prête très bien au découpage en couches. Free tier Serverless Containers
   (~400k Go-s/mois) très au-dessus du besoin ; scale-to-zero → cold start
   ~1-2 s, acceptable pour un usage familial.
2. **Rust WASM sur Cloudflare Workers**
   Gratuité la plus garantie (Workers + D1 + R2), mais dialecte de Rust
   contraint : pas de SQLx/Tokio, bindings JS, limite 10 ms CPU qui complique
   jusqu'au hash de mot de passe. Mauvais support de l'objectif d'apprentissage.
3. **TypeScript partout (Hono/Remix sur Cloudflare)**
   Le plus rapide à livrer, mais n'apprend pas Rust.

## Décision

Option 1 :

- **API** : Rust — Axum + SQLx + Tokio, sur **Scaleway Serverless Containers**
  (scale-to-zero).
- **Front** : React + Vite + TS en PWA, sur **Cloudflare Pages**.
- **BDD** : PostgreSQL managé **Neon** (free tier 0,5 Go), Docker en local.
- **Photos** : **Cloudflare R2** (10 Go gratuits, zéro egress), upload par URL
  présignée.

## Conséquences

- Cold start de ~1-2 s après inactivité — assumé.
- Seul poste de coût à surveiller : le stockage du Container Registry Scaleway
  au-delà du quota gratuit → purge des vieilles images en CI.
- Free tiers Neon/Scaleway à re-vérifier au moment du déploiement (jalon 6) —
  les offres évoluent.
