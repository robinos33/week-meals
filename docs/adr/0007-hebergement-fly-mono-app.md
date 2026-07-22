# ADR-0007 — Hébergement : une seule app Fly.io servant l'API et le front

- **Statut :** acceptée (2026-07-22), amendée par
  [ADR-0008](0008-sqlite-volume-fly.md) sur le choix de la base
- **Remplace** la partie « hébergement » de [ADR-0001](0001-stack-rust-axum-scaleway.md)
  (Scaleway Serverless Containers + Cloudflare Pages). Le reste de l'ADR-0001
  — Rust/Axum, React/Vite, R2 — reste valable.

## Contexte

L'ADR-0001 prévoyait de séparer le front (Cloudflare Pages) et l'API (Scaleway).
Au moment de déployer réellement, cette séparation coûte cher en complexité pour
un projet d'un seul foyer :

- **CORS** à maintenir, avec cookies de session : origine explicite, pas de `*`.
- **Cookie `SameSite=None`** obligatoire entre deux domaines distincts, ce qui
  impose `Secure` et expose aux blocages de cookies tiers des navigateurs.
- **WebAuthn** (ADR-0006) exige que le `rp_id` soit un domaine partagé par le
  front et l'API. Deux sous-domaines d'un domaine acheté fonctionnent
  (`app.exemple.fr` + `api.exemple.fr`), mais deux domaines fournisseur
  (`*.pages.dev` + `*.fly.dev`) **non** — il faudrait acheter un domaine avant
  même de pouvoir tester une passkey.
- **Deux pipelines** de déploiement, deux fournisseurs à surveiller.

## Décision

**Une seule application Fly.io.** Le binaire Axum sert :

- l'API sous le préfixe **`/api`** ;
- `/health` pour le health check de la plateforme ;
- **tout le reste** en statique depuis le build Vite (`WEB_DIST`), avec repli
  sur `index.html` pour que le routeur client gère les URL profondes.

Le préfixe `/api` n'est pas cosmétique : les routes se chevauchaient à la racine
(`/recipes` existe côté SPA **et** côté API), une même origine les rendait
inconciliables.

Reste inchangé : **Cloudflare R2** pour les photos. La base restait alors chez
**Neon** ; l'[ADR-0008](0008-sqlite-volume-fly.md) l'a depuis ramenée dans
l'app, sous forme d'un fichier SQLite sur volume Fly.

## Conséquences

- **Plus de CORS ni de `SameSite=None`** en prod : même origine, cookie `Lax`.
- **WebAuthn marche d'emblée** sur `week-meals.fly.dev` — `rp_id` = ce domaine,
  aucun domaine à acheter pour démarrer.
- **Le front n'est plus déployable seul** : un changement de CSS reconstruit
  l'image entière. Acceptable au rythme du projet ; le cache de couches Docker
  limite la casse (les dépendances npm et cargo ne sont pas retéléchargées).
- **Pas de CDN devant les assets.** Une seule région (`cdg`) et un seul foyer :
  sans intérêt ici.
- **Les migrations sont jouées au démarrage** par le serveur
  (`sqlx::migrate!`, embarquées dans le binaire) plutôt que par une étape
  `sqlx-cli` séparée. Le migrateur prend un verrou consultatif, deux machines
  qui démarrent simultanément ne divergent donc pas. (Sans objet depuis
  l'[ADR-0008](0008-sqlite-volume-fly.md) : la base étant un volume attaché à
  une machine unique, il n'y a jamais deux migrateurs.)
- **`scale-to-zero`** conservé (`min_machines_running = 0`) : cold start de
  ~1-2 s après inactivité, comme prévu par l'ADR-0001.
- **Le budget 0 € n'est plus garanti.** Fly facture à l'usage sans free tier
  équivalent à celui de Scaleway ; une `shared-cpu-1x`/512 Mo qui dort la
  majorité du temps reste de l'ordre de quelques euros par mois. C'est le prix
  assumé de la simplification.

## Alternatives écartées

- **Deux apps Fly** (api + web) : reproduit tous les inconvénients CORS /
  `SameSite` / `rp_id` de la séparation, sans le bénéfice du CDN de Pages.
- **Rester sur Scaleway + Pages** : conforme à l'ADR-0001 et moins cher, mais
  imposait l'achat d'un domaine avant de pouvoir tester les passkeys, et deux
  chaînes de déploiement à maintenir.
