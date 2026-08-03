-- Point de mire de la photo d'une recette (#un89mr) : où recadrer l'image.
--
-- Les cadres de la grille et de la fiche sont en paysage (`aspect-ratio` 4/3 et
-- 16/10) ; une photo portrait — typique d'un reel Instagram importé — y est
-- recadrée `cover` sur son centre, qui tombe souvent sur un visage plutôt que
-- sur le plat. Ces deux pourcentages (`0..=100`, 50 = centre) laissent l'auteur
-- désigner le point à garder au cadrage ; le front les sert en `object-position`.
alter table recipes add column photo_focus_x integer not null default 50
    check (photo_focus_x between 0 and 100);
alter table recipes add column photo_focus_y integer not null default 50
    check (photo_focus_y between 0 and 100);
