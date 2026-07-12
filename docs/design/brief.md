# Brief design — Week Meals

> **Direction retenue : « Cantine ».** Après comparaison de deux pistes sur la
> [maquette](https://claude.ai/design/p/b3bc9702-d396-4cd4-abca-90c2be9641b2?file=Week+Meals.dc.html&via=share),
> on part sur la direction **Cantine** : vert potager, style net et lisible,
> barre d'onglets basse pleine largeur. La piste « Marché » (terracotta,
> cartes rondes, navigation flottante) est écartée. Titres serif (Fraunces
> léger) + texte sans-serif ; conception en clair, dark mode en bonus.

## Le produit

PWA mobile-first pour un foyer (2 personnes) : gérer ses recettes, planifier
les repas de la semaine (midi/soir), et générer une liste de courses
intelligente. Projet open source, auto-hébergeable.

## Utilisateurs & moments d'usage

Un couple, usage quotidien et rapide. Deux moments clés à optimiser :

1. **Le dimanche, sur le canapé** : on feuillette les recettes et on remplit
   le calendrier de la semaine à deux.
2. **En magasin, une main sur le caddie** : on coche la liste de courses,
   souvent avec un réseau médiocre (l'app gère l'offline).

## Plateforme & contraintes

- Mobile-first (viewport ~375 px), mais doit rester agréable sur desktop.
- PWA installée plein écran → prévoir la zone safe-area, navigation par
  barre d'onglets en bas (pouce).
- Cibles tactiles ≥ 44 px, utilisable à une main, accessibilité WCAG AA.
- Dark mode bienvenu (usage en soirée).

## Ton & direction

Chaleureux, appétissant, domestique — c'est un outil de cuisine de couple,
pas un SaaS corporate. Simple et joyeux sans être enfantin. Les photos de
plats sont le principal vecteur d'émotion : le design doit les mettre en
valeur (cartes photo généreuses).

## Principes UX

- **Friction minimale avant tout.** Référence explicite : Google Keep.
- Pas de modales bloquantes : bottom sheets, expansion inline, toasts.
- Chaque action fréquente (ajouter un article, cocher, planifier une
  recette) doit tenir en 1–2 taps.

## Écrans à designer

### 1. Connexion

Pseudo + mot de passe, sobre. Pas d'inscription publique (accès sur
invitation). Écran secondaire : accepter une invitation.

### 2. Recettes (onglet 1)

Grille de cartes : photo, titre, temps total (prépa + cuisson). Recherche
en haut, bouton flottant « + » pour créer. Action rapide sur une carte :
« planifier cette semaine ».

### 3. Détail / édition de recette

Photo en héro, titre, temps de prépa et de cuisson, liste d'ingrédients
(quantité + unité : g, kg, mL, L ou pièces). Édition inline des
ingrédients (pas de formulaire-tunnel).

### 4. Semaine (onglet 2)

Vue calendrier hebdomadaire : 7 jours × 2 créneaux (midi / soir). Un
créneau vide se remplit via un picker de recettes en bottom sheet. Un
créneau rempli montre une mini-carte de la recette (photo + titre).
Action clé : « générer la liste de courses » depuis une plage de jours.

### 5. Liste de courses (onglet 3) — inspiration Google Keep

- Champ d'ajout rapide toujours accessible en haut.
- Articles avec case à cocher ; un tap coche → l'article se barre et
  glisse dans une section « cochés » repliable en bas.
- Tap sur le texte : édition inline (nom, quantité, unité).
- Action « vider les cochés ».
- Les articles générés depuis les recettes affichent la conversion
  (ex. « 3 courgettes » plutôt que « 600 g ») ; distinguer subtilement
  les articles générés des ajouts manuels.
- Indicateur discret d'état offline / synchronisation.

### 6. Paramètres

Compte, générer un lien d'invitation, préférences (thème).

## Composants transverses

Barre d'onglets basse (Recettes / Semaine / Courses), bouton flottant,
bottom sheets, toasts, états vides sympathiques (première visite : pas de
recettes, semaine vide, liste vide).
