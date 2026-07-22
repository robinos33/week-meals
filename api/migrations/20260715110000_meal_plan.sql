-- Calendrier des repas : jour × créneau (midi / soir) → recette, par foyer.
-- Au plus une recette par case, d'où la clé primaire composite.

-- La FK composite (household_id, recipe_id) garantit qu'on ne planifie qu'une
-- recette DU foyer ; elle exige une contrainte d'unicité sur ces colonnes côté
-- recettes. SQLite n'a pas d'`alter table … add constraint` : c'est un index
-- unique nommé, que la FK ci-dessous référence tout aussi bien.
create unique index recipes_household_id_id_key on recipes (household_id, id);

create table meal_plan (
    household_id blob not null,
    -- `date` en Postgres ; ici du texte `YYYY-MM-DD`, dont l'ordre
    -- lexicographique est aussi l'ordre chronologique (`between` reste juste).
    meal_date    text not null,
    slot         text not null check (slot in ('lunch', 'dinner')),
    recipe_id    blob not null,
    updated_at   text not null default (datetime('now')),
    primary key (household_id, meal_date, slot),
    foreign key (household_id) references households (id) on delete cascade,
    -- SQLite ne nomme pas la contrainte violée dans ses erreurs : l'infra ne
    -- peut plus distinguer « recette hors foyer » (404) d'une panne (500) après
    -- coup. Elle vérifie donc l'appartenance avant d'écrire (cf. ADR-0008) ;
    -- cette FK reste le garde-fou d'intégrité.
    foreign key (household_id, recipe_id)
        references recipes (household_id, id) on delete cascade
);

-- Pas d'index sur (household_id, meal_date) : la clé primaire composite
-- (household_id, meal_date, slot) couvre déjà ce préfixe, y compris pour la
-- lecture d'une plage de jours.
