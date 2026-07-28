-- Magasins du foyer et **ordre de visite de leurs rayons**.
--
-- Le dictionnaire d'ingrédients donne un rayon à chaque produit ; il manquait
-- de quoi parcourir ces rayons dans l'ordre d'un magasin donné — chez l'un les
-- surgelés sont à l'entrée, chez l'autre juste avant les caisses. Un magasin
-- n'est donc pas un catalogue de produits mais un **trajet** : un nom, et
-- l'ordre dans lequel on en traverse les rayons.
--
-- Le catalogue des rayons, lui, est fermé et vit dans le code
-- (`shopping_list::domain::aisle`) : rien à seeder ici. Chaque magasin en
-- stocke l'ordre complet, un rayon par ligne — l'ordre est une donnée à part
-- entière, pas une chaîne à découper.

create table stores (
    id           blob primary key,
    household_id blob not null references households (id) on delete cascade,
    name         text not null,
    -- Ordre d'affichage des magasins entre eux (le premier sert de défaut).
    position     integer not null default 0,
    created_at   text not null default (datetime('now'))
);

create index stores_household_idx on stores (household_id, position);

-- Un rayon d'un magasin, à son rang dans le parcours. La clé primaire
-- (magasin, rayon) interdit le doublon ; `position` porte l'ordre.
create table store_aisles (
    store_id blob not null references stores (id) on delete cascade,
    aisle    text not null,
    position integer not null,
    primary key (store_id, aisle)
);
