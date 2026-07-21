-- Premier jour de la semaine, paramétrable au niveau du foyer (#57).
-- Le planning est partagé : le réglage porte donc sur le foyer, pas sur
-- l'utilisateur. Convention `Date.getDay()` du front : 0 = dimanche … 6 =
-- samedi. Défaut 1 = lundi (comportement historique). Notre foyer fait les
-- courses le samedi (valeur 6), d'où le besoin de le rendre configurable.
alter table households
    add column week_start_day smallint not null default 1
        check (week_start_day between 0 and 6);
