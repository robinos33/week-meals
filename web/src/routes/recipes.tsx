import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import "./screens.css";

/** Vue d'une recette telle qu'exposée par l'API (`RecipeView`). */
interface RecipeView {
  id: string;
  title: string;
  photo: string | null;
  prep_time_min: number | null;
  cook_time_min: number | null;
}

function totalTime(recipe: RecipeView): string | null {
  const total = (recipe.prep_time_min ?? 0) + (recipe.cook_time_min ?? 0);
  return total > 0 ? `${total} min` : null;
}

/** Onglet Recettes : grille de cartes, recherche, bouton flottant « + ». */
export function RecipesScreen() {
  const [search, setSearch] = useState("");
  const query = useQuery({
    queryKey: ["recipes", search],
    queryFn: () =>
      api.get<RecipeView[]>(
        `/recipes${search.trim() ? `?search=${encodeURIComponent(search.trim())}` : ""}`,
      ),
  });

  const recipes = query.data ?? [];

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Recettes</h1>
      </header>

      <div className="search">
        <input
          className="input"
          type="search"
          placeholder="Rechercher une recette…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Rechercher une recette"
        />
      </div>

      {query.isLoading ? (
        <p className="muted">Chargement…</p>
      ) : recipes.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state__emoji">🥬</div>
          <h2>Aucune recette pour l'instant</h2>
          <p>Ajoutez votre première recette avec le bouton +.</p>
        </div>
      ) : (
        <div className="recipe-grid">
          {recipes.map((recipe) => (
            <article key={recipe.id} className="card recipe-card">
              <div className="recipe-card__photo">
                {recipe.photo ? (
                  <img
                    src={recipe.photo}
                    alt=""
                    style={{ width: "100%", height: "100%", objectFit: "cover" }}
                  />
                ) : (
                  "🍽️"
                )}
              </div>
              <div className="recipe-card__body">
                <div className="recipe-card__title">{recipe.title}</div>
                {totalTime(recipe) && (
                  <div className="recipe-card__time">{totalTime(recipe)}</div>
                )}
              </div>
            </article>
          ))}
        </div>
      )}

      <button className="fab" aria-label="Ajouter une recette" type="button">
        +
      </button>
    </section>
  );
}
