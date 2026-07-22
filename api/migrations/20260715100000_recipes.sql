-- Recettes, scopées au foyer (cf. ADR-0003 : la DB est la source de vérité).
-- Une recette porte ses ingrédients et ses étapes ordonnés (tables filles,
-- supprimées en cascade). L'unité reprend l'orthographe canonique du `kernel`
-- (`g`/`kg`/`ml`/`l`/`piece`).

create table recipes (
    id            blob primary key,
    household_id  blob not null references households (id) on delete cascade,
    title         text not null,
    -- Clé de recherche et de tri : titre en minuscules, accents dépliés,
    -- calculée en Rust (`normalize_title`). SQLite ne sait ni comparer ni
    -- trier hors ASCII — sans elle « Éclair » passerait après « Zeste » et
    -- une recherche « CRÈME » ne trouverait rien (cf. ADR-0008).
    title_norm    text not null,
    photo         text,
    prep_time_min integer check (prep_time_min >= 0),
    cook_time_min integer check (cook_time_min >= 0),
    created_at    text not null default (datetime('now')),
    updated_at    text not null default (datetime('now'))
);

create index recipes_household_id_idx on recipes (household_id);
-- Sert le tri de la grille du front (`order by title_norm`). La recherche
-- `title_norm like '%…%'` ne peut pas l'utiliser (joker en tête) : elle scanne
-- le foyer, ce qui reste négligeable à cette échelle. Si le volume le
-- justifiait, passer à une table FTS5.
create index recipes_title_norm_idx on recipes (household_id, title_norm);

-- Ingrédients d'une recette, ordonnés par `position`.
create table recipe_ingredients (
    recipe_id blob not null references recipes (id) on delete cascade,
    position  integer not null,
    name      text not null,
    quantity  real not null check (quantity > 0),
    unit      text not null,
    primary key (recipe_id, position)
);

-- Étapes de préparation, ordonnées par `position`.
create table recipe_steps (
    recipe_id   blob not null references recipes (id) on delete cascade,
    position    integer not null,
    instruction text not null,
    primary key (recipe_id, position)
);
