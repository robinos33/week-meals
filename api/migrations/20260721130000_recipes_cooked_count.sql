-- Podium des recettes les plus cuisinées (#58) : compteur « cuisiné X fois »
-- et médailles 🥇🥈🥉 sur les trois recettes en tête.

-- Le compteur vit sur la recette : lu par la fiche (détail) et par le résumé
-- `/recipes` (la grille en a besoin pour classer). `last_cooked_at` sert de
-- départage déterministe entre recettes à égalité (la plus récemment cuisinée
-- devant), avant le titre.
alter table recipes
    add column cooked_count   integer     not null default 0 check (cooked_count >= 0),
    add column last_cooked_at timestamptz;

-- Garde anti-double-comptage. Le compteur s'incrémente à la **génération de la
-- liste de courses**, pas à la planification : générer vaut engagement. Or on
-- régénère souvent la même semaine (ajout d'une recette en cours de route) —
-- sans garde, chaque régénération recompterait tout. On marque donc chaque
-- créneau `(household, date, slot)` au moment où il est compté, et seules les
-- cases encore vierges (`counted_at is null`) incrémentent aux générations
-- suivantes. Une recette posée sur deux créneaux compte bien deux fois.
alter table meal_plan
    add column counted_at timestamptz;
