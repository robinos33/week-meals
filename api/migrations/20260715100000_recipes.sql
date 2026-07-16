-- Recettes, scopées au foyer (cf. ADR-0003 : la DB est la source de vérité).
-- Une recette porte ses ingrédients et ses étapes ordonnés (tables filles,
-- supprimées en cascade). L'unité reprend l'orthographe canonique du `kernel`
-- (`g`/`kg`/`ml`/`l`/`piece`).

create table recipes (
    id            uuid primary key default gen_random_uuid(),
    household_id  uuid not null references households (id) on delete cascade,
    title         text not null,
    photo         text,
    prep_time_min integer check (prep_time_min >= 0),
    cook_time_min integer check (cook_time_min >= 0),
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now()
);

create index recipes_household_id_idx on recipes (household_id);
-- Sert le tri de la grille du front (`order by lower(title)`). La recherche
-- `title ilike '%…%'` ne peut pas l'utiliser (joker en tête) : elle scanne le
-- foyer, ce qui reste négligeable à cette échelle. Si le volume le justifiait,
-- passer à un index trigram (`pg_trgm` + GIN).
create index recipes_title_idx on recipes (household_id, lower(title));

-- Ingrédients d'une recette, ordonnés par `position`.
create table recipe_ingredients (
    recipe_id uuid not null references recipes (id) on delete cascade,
    position  integer not null,
    name      text not null,
    quantity  double precision not null check (quantity > 0),
    unit      text not null,
    primary key (recipe_id, position)
);

-- Étapes de préparation, ordonnées par `position`.
create table recipe_steps (
    recipe_id   uuid not null references recipes (id) on delete cascade,
    position    integer not null,
    instruction text not null,
    primary key (recipe_id, position)
);
