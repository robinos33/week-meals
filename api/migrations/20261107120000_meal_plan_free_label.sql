-- Repas libre : un créneau peut porter un simple libellé (« Restes »,
-- « Resto », « Pizza maison »…) au lieu d'une recette. Il dit ce qu'on mange
-- sans rien ajouter à la liste de courses ni créer de recette vide.
--
-- Une case porte donc **soit** une recette, **soit** un libellé — jamais les
-- deux, jamais aucun (check XOR). `recipe_id` devient nullable : la FK
-- composite `(household_id, recipe_id)` n'est pas vérifiée quand une de ses
-- colonnes est NULL (comportement standard SQLite), elle reste donc le
-- garde-fou des cases « recette ».
--
-- Les jointures existantes sur `recipes` (génération de la liste, compteur
-- « cuisiné X fois ») écartent d'elles-mêmes les cases libres.
--
-- SQLite ne sait ni relâcher un NOT NULL ni ajouter un CHECK sur une table
-- existante : on reconstruit la table. Aucune table ne référence `meal_plan`,
-- le `drop` ne déclenche donc aucune cascade.

create table meal_plan_new (
    household_id blob not null,
    meal_date    text not null,
    slot         text not null check (slot in ('lunch', 'dinner')),
    recipe_id    blob,
    label        text check (label is null or length(trim(label)) between 1 and 80),
    updated_at   text not null default (datetime('now')),
    servings     integer not null default 2 check (servings > 0),
    counted_at   text,
    primary key (household_id, meal_date, slot),
    check ((recipe_id is null) <> (label is null)),
    foreign key (household_id) references households (id) on delete cascade,
    foreign key (household_id, recipe_id)
        references recipes (household_id, id) on delete cascade
);

insert into meal_plan_new
    (household_id, meal_date, slot, recipe_id, label, updated_at, servings, counted_at)
select household_id, meal_date, slot, recipe_id, null, updated_at, servings, counted_at
from meal_plan;

drop table meal_plan;
alter table meal_plan_new rename to meal_plan;
