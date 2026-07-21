# ADR-0002 — Authentification sans email

- **Statut :** remplacée (2026-07-20) par
  [ADR-0006](0006-auth-passkeys-appareils-enroles.md)

## Contexte

Le projet est open source et destiné à être self-hosté. Les utilisateurs réels
(le couple) ne veulent **aucune donnée personnelle** — pas d'email — ni dans le
repo, ni exigée par l'app. Deux utilisateurs, pas d'inscription publique.

## Décision

- Login **pseudo + mot de passe** (hash **Argon2id**), sessions cookie
  (`HttpOnly`, `SameSite`) — pas de JWT (même origine).
- **Inscription publique désactivée.** Bootstrap du premier compte via variable
  d'env `BOOTSTRAP_INVITE_CODE` (consommée une fois).
- Ensuite, un membre du foyer génère un **lien d'invitation** (token à usage
  unique, expirant) pour ajouter quelqu'un.
- Pas de « mot de passe oublié » par mail : **reset via commande CLI** côté
  serveur — suffisant pour un foyer.

## Conséquences

- Zéro service de mail à configurer (ni coût, ni secret SMTP pour les
  self-hosters).
- Perte de mot de passe = intervention CLI de l'admin. Assumé à cette échelle ;
  si le besoin émerge, des codes de récupération pré-générés sont une piste.
