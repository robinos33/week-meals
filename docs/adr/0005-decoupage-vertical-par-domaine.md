# ADR-0005 — Découpage vertical par domaine (une crate par domaine)

- **Statut :** acceptée (2026-07-13)

## Contexte

Le plan initial prévoyait un découpage **horizontal** : une crate par couche
(`domain`, `application`, `infrastructure`, `presentation`), avec pour bénéfice
que la crate `domain` ne peut physiquement pas dépendre de SQLx/Axum — la pureté
des couches est alors opposable par le compilateur.

À l'usage, deux limites apparaissent pour un projet de cette taille (un foyer,
développement souvent en parallèle sur plusieurs worktrees) :

- **Cohésion faible** : une fonctionnalité (« recettes ») est éclatée dans les
  quatre crates ; la toucher impose d'éditer partout.
- **Frictions de parallélisme** : deux features développées en parallèle se
  disputent les mêmes crates (et le même `Cargo.toml`), d'où des conflits de
  merge évitables.

## Options considérées

1. **Vertical par domaine + enforcement par convention** ✅
   Une crate par domaine (`auth`, `recipes`, `meal-plan`, `shopping-list`), les
   couches en modules internes, plus un `kernel` pur et un binaire `server`.
2. **Horizontal (une crate par couche)** — pureté opposable par le compilateur,
   mais cohésion faible et parallélisme plus conflictuel (voir Contexte).
3. **Vertical + split `core`/`adapters`** — chaque domaine scindé en deux crates
   (`*-core` pur / `*-adapters`) pour garder la pureté opposable par le
   compilateur tout en restant vertical. Écarté : ~2× de crates et de cérémonie,
   disproportionné à cette échelle.

## Décision

Option 1 :

- **Une crate par domaine métier**, chacune organisée en modules internes
  `domain` / `application` (`commands` + `queries`) / `infrastructure` /
  `presentation`.
- Une crate **`kernel`** pure pour les types transverses (value objects communs,
  identifiants, erreurs) ; les domaines dépendent de `kernel`, jamais l'inverse,
  et ne dépendent pas les uns des autres (les références croisées passent par
  `kernel`).
- Un binaire **`server`** qui compose les sous-routers exposés par chaque domaine
  et démarre Axum.
- **Règle de couches par convention** : le module `domain` reste pur (ni SQLx ni
  Axum). Cette pureté n'est plus opposable par le compilateur au sein d'une
  crate — elle est garantie par la revue de code.

## Conséquences

- Meilleure cohésion (« screaming architecture ») et découpage qui calque les
  jalons/tickets : un domaine = un flux de travail = une branche/worktree.
- Moins de conflits de merge : chaque domaine évolue dans sa propre crate ; la
  liste des membres du workspace est stable (les crates de domaine existent dès
  le squelette). Le principal fichier partagé restant est `server` (montage des
  routers) — collision triviale à résoudre.
- On perd la garantie *compilateur* de pureté du domaine ; à surveiller en revue.
  Si le besoin d'une garantie dure émerge, le split `core`/`adapters` (option 3)
  reste une évolution possible, domaine par domaine.
