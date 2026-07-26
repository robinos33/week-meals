-- Nombre de personnes (portions), sur deux niveaux :
--
-- - `recipes.servings`   : la base à laquelle correspondent les quantités
--   d'ingrédients saisies. Une recette est « pour 2 » par défaut ; un import
--   par URL ramène les quantités à cette base (cf. scraper, resize à 2).
-- - `meal_plan.servings` : le nombre de convives voulu pour ce créneau précis.
--   La génération de liste de courses met les quantités à l'échelle par le
--   ratio `meal_plan.servings / recipes.servings`.
--
-- Défaut à 2 : c'est la base historique de toutes les recettes existantes, et
-- ADD COLUMN NOT NULL l'exige. Contrainte `> 0` alignée sur l'invariant métier.

alter table recipes
    add column servings integer not null default 2 check (servings > 0);

alter table meal_plan
    add column servings integer not null default 2 check (servings > 0);
