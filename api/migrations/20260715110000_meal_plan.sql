-- Calendrier des repas : jour × créneau (midi / soir) → recette, par foyer.
-- Au plus une recette par case, d'où la clé primaire composite.

-- La FK composite (household_id, recipe_id) garantit qu'on ne planifie qu'une
-- recette DU foyer ; elle exige une contrainte d'unicité sur ces colonnes côté
-- recettes (ajoutée ici, migrations append-only).
alter table recipes add constraint recipes_household_id_id_key unique (household_id, id);

create table meal_plan (
    household_id uuid not null,
    meal_date    date not null,
    slot         text not null check (slot in ('lunch', 'dinner')),
    recipe_id    uuid not null,
    updated_at   timestamptz not null default now(),
    primary key (household_id, meal_date, slot),
    foreign key (household_id) references households (id) on delete cascade,
    -- Nommée : l'infra distingue cette violation (recette hors foyer ⇒ 404)
    -- de celle du foyer ci-dessus (panne ⇒ 500), via le nom de contrainte.
    constraint meal_plan_recipe_fkey foreign key (household_id, recipe_id)
        references recipes (household_id, id) on delete cascade
);

-- Pas d'index sur (household_id, meal_date) : la clé primaire composite
-- (household_id, meal_date, slot) couvre déjà ce préfixe, y compris pour la
-- lecture d'une plage de jours.
