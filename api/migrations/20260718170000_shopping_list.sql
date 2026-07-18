-- Référentiel d'ingrédients (#18) et liste de courses (#19), cf. plan.md.

-- Référentiel **global** (non scopé au foyer) : versionné dans
-- data/ingredients.yaml et seedé par `weekmeals seed-ingredients` (upsert par
-- nom). `name` est stocké normalisé (trim + minuscules), ce qui en fait
-- directement la clé de rapprochement utilisée par le service de conversion.
create table ingredient_reference (
    name         text primary key,
    category     text not null,
    avg_weight_g integer not null check (avg_weight_g > 0),
    -- true = s'achète toujours en pièces (œufs…), jamais reconverti en grammes.
    countable    boolean not null default false,
    updated_at   timestamptz not null default now()
);

-- Liste de courses du foyer. Une seule liste courante par foyer : les lignes
-- issues de la génération (`generated`) sont remplaçables en bloc, tandis que
-- les ajouts manuels survivent à une régénération — c'est ce qui rend
-- `POST /shopping-list/generate` idempotent.
create table shopping_list_items (
    id           uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id) on delete cascade,
    name         text not null,
    amount       double precision not null check (amount > 0),
    unit         text not null,
    -- Rayon, connu seulement pour les ingrédients référencés.
    category     text,
    checked      boolean not null default false,
    generated    boolean not null default false,
    -- Ordre d'affichage : suit l'ordre de sortie du service de conversion.
    position     integer not null default 0,
    created_at   timestamptz not null default now()
);

create index shopping_list_items_household_idx
    on shopping_list_items (household_id, generated);
