-- Le référentiel devient un vrai **dictionnaire** d'ingrédients : synonymes,
-- poids moyen facultatif et unité d'achat, pour rapprocher les formulations
-- des recettes (« Courgettes », « courgette jaune ») d'un nom canonique unique.
--
-- Trois changements, dont deux imposent de reconstruire la table (SQLite ne
-- sait ni relâcher un `not null` ni retirer un `check` par `alter table`) :
--
--   * `avg_weight_g` devient facultatif — une entrée peut n'apporter que le
--     nom canonique, les synonymes et le rayon (farine, lait…), sans pour
--     autant se faire convertir en pièces ;
--   * `aliases` liste les synonymes, séparés par `|` (aucun nom d'ingrédient
--     n'en contient) ;
--   * `unit` est l'unité proposée à la saisie manuelle.
--
-- Les lignes existantes sont conservées telles quelles : elles ont toutes un
-- poids moyen, donc s'achètent à la pièce. Le seed (`weekmeals
-- seed-ingredients`, rejoué au démarrage du serveur) les enrichira de leurs
-- synonymes.

create table ingredient_reference_new (
    name         text primary key,
    category     text not null,
    -- Poids moyen d'une pièce. `null` = ne s'achète pas à la pièce.
    avg_weight_g integer check (avg_weight_g is null or avg_weight_g > 0),
    -- 1 = s'achète toujours en pièces (œufs…), jamais reconverti en grammes.
    countable    integer not null default 0,
    -- Synonymes séparés par `|`, déjà normalisés (minuscules, sans espaces
    -- de bord). Vide si l'entrée n'en a pas.
    aliases      text not null default '',
    -- Unité d'achat conseillée (texte canonique du `kernel`).
    unit         text not null default 'piece',
    updated_at   text not null default (datetime('now'))
);

insert into ingredient_reference_new
    (name, category, avg_weight_g, countable, aliases, unit, updated_at)
select name, category, avg_weight_g, countable, '', 'piece', updated_at
from ingredient_reference;

drop table ingredient_reference;

alter table ingredient_reference_new rename to ingredient_reference;
