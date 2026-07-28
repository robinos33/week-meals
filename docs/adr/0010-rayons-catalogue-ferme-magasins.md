# ADR-0010 — Rayons : un catalogue fermé, un ordre de visite par magasin

- **Statut :** acceptée (2026-07-28)
- **Complète** le dictionnaire d'ingrédients ([data/ingredients.yaml](../../data/ingredients.yaml)),
  dont le champ `category` devient le pivot du tri en magasin.

## Contexte

Le dictionnaire donne déjà un rayon à chaque ingrédient, mais ce rayon ne
servait qu'à décorer une suggestion de saisie : la liste de courses restait un
tas plat, dans l'ordre de génération. En magasin, cela veut dire des
allers-retours — la crème après le riz, le riz après les surgelés.

Trois choses manquaient pour trier la liste dans l'ordre du magasin :

1. un **vocabulaire de rayons** assez fin pour coller aux allées réelles :
   « épicerie » couvrait 46 entrées, soit trois ou quatre allées différentes ;
2. de quoi ranger dans la liste ce qui n'est **pas un aliment** — papier
   toilette, lessive, piles, croquettes : ça va dans le même caddie, et ça se
   perd tout autant ;
3. un **ordre de visite**, qui n'appartient ni au produit ni au foyer mais au
   **magasin** : chez l'un les surgelés sont à l'entrée, chez l'autre juste
   avant les caisses.

## Options considérées

### Le vocabulaire des rayons

1. **Catalogue fermé, dans le code du domaine** ✅ — une liste de slugs stables
   (`domain::aisle`), servie au front par `GET /aisles`, et que
   `data/ingredients.yaml` ne peut pas dépasser (un test le vérifie sur le
   fichier versionné).
2. **Texte libre** — chaque entrée du dictionnaire nomme son rayon. Rejeté :
   une faute de frappe crée un rayon fantôme qu'aucun magasin n'ordonne, et les
   articles concernés disparaissent silencieusement en fin de liste.
3. **Rayons par foyer, éditables** — plus souple, mais il faudrait alors
   re-router 200 entrées de dictionnaire à chaque foyer. Rejeté : le
   dictionnaire est global (cf. son en-tête), ses rayons doivent l'être aussi.

### L'ordre de visite

1. **Un magasin = un nom + l'ordre complet des rayons** ✅, par foyer.
2. **Un seul ordre par foyer** — insuffisant : on fait ses courses à plusieurs
   endroits, et c'est justement quand on change de magasin qu'on se perd.
3. **Ordre partiel (les rayons « présents » seulement)** — rejeté : un rayon
   oublié laisse ses articles sans place. Un magasin porte donc *tous* les
   rayons ; un rayon sans article ne produit simplement aucune section.

## Décision

- Le catalogue des rayons vit dans `shopping-list::domain::aisle` : 17 slugs
  stables, dans un ordre par défaut de supermarché type. `GET /aisles` le sert
  au front, qui n'en duplique ni la liste ni les libellés.
- `data/ingredients.yaml` est recatégorisé dessus (`epicerie` devient
  `epicerie-salee` / `epicerie-sucree` / `condiments`) et accueille les produits
  **hors alimentaires** courants — ils n'arrivent jamais d'une recette, mais y
  figurer leur donne l'auto-complétion, l'unité et le rayon.
- Un **magasin** (`stores`, `store_aisles`) appartient au foyer : un nom et
  l'ordre de visite de ses rayons, réglé dans les paramètres. `Store::reorder`
  normalise ce qu'on lui envoie — slugs inconnus écartés, doublons retirés,
  rayons non cités complétés dans l'ordre par défaut.
- L'onglet Courses propose le tri par magasin ; le magasin choisi est gardé
  **par appareil** (`localStorage`), pas par foyer : dans un foyer, chacun ne
  fait pas ses courses au même endroit.
- Le tri est un **affichage** : il ne réordonne jamais la liste côté serveur.
  L'ordre manuel (glisser-déposer) reste le mode par défaut, et le seul quand
  aucun magasin n'est paramétré.

## Conséquences

- Un article dont le rayon est inconnu du magasin — ou qui n'en a pas — n'est
  jamais perdu : il atterrit dans une section « Autres », en fin de parcours.
  C'est aussi le filet des lignes créées avant cette ADR, qui portent encore un
  ancien slug (`epicerie`) : elles se rangent d'elles-mêmes dès qu'on les
  ressaisit ou qu'on régénère la liste.
- Ajouter un rayon au catalogue est une modification de code, migration
  comprise dans le fait que les magasins existants le verront apparaître **en
  fin de parcours** (complété par `Store::reorder`), à replacer une fois.
- Deux tests gardent le dictionnaire honnête : tous ses rayons existent au
  catalogue, et aucune entrée n'est absorbée par une autre au rapprochement
  (« savon » ne doit pas devenir « saumon »).
