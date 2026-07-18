import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { totalTime, useRecipes } from "../api/recipes";
import "./screens.css";

/** Onglet Recettes : grille de cartes, recherche, bouton flottant « + ». */
export function RecipesScreen() {
  const [search, setSearch] = useState("");
  const query = useRecipes(search);
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
      ) : query.isError ? (
        <div className="empty-state">
          <div className="empty-state__emoji">🌩️</div>
          <h2>Recettes indisponibles</h2>
          <p>La liste n'a pas pu être chargée.</p>
          <button className="btn" type="button" onClick={() => query.refetch()}>
            Réessayer
          </button>
        </div>
      ) : recipes.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state__emoji">{search.trim() ? "🔍" : "🥬"}</div>
          {search.trim() ? (
            <>
              <h2>Aucun résultat</h2>
              <p>Aucune recette ne correspond à « {search.trim()} ».</p>
            </>
          ) : (
            <>
              <h2>Aucune recette pour l'instant</h2>
              <p>Ajoutez votre première recette avec le bouton +.</p>
            </>
          )}
        </div>
      ) : (
        <div className="recipe-grid">
          {recipes.map((recipe) => (
            <Link
              key={recipe.id}
              to="/recipes/$recipeId"
              params={{ recipeId: recipe.id }}
              className="card recipe-card"
            >
              <div className="recipe-card__photo">
                {recipe.photo ? <img src={recipe.photo} alt="" /> : "🍽️"}
              </div>
              <div className="recipe-card__body">
                <div className="recipe-card__title">{recipe.title}</div>
                {totalTime(recipe) && (
                  <div className="recipe-card__time">{totalTime(recipe)}</div>
                )}
              </div>
            </Link>
          ))}
        </div>
      )}

      <Link to="/recipes/new" className="fab" aria-label="Ajouter une recette">
        +
      </Link>
    </section>
  );
}
