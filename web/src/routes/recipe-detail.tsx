import { useState } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import {
  UNITS,
  totalTime,
  useDeleteRecipe,
  useRecipe,
  type Ingredient,
} from "../api/recipes";
import "./screens.css";

/** Libellé lisible d'une quantité (« 600 g », « 3 pièce(s) »). */
function quantityLabel(ingredient: Ingredient): string {
  const unit = UNITS.find((u) => u.value === ingredient.unit)?.label ?? ingredient.unit;
  return `${ingredient.amount} ${unit}`;
}

/** Écran détail d'une recette (`/recipes/$recipeId`). */
export function RecipeDetailScreen() {
  const { recipeId } = useParams({ from: "/recipes/$recipeId" });
  const navigate = useNavigate();
  const query = useRecipe(recipeId);
  const remove = useDeleteRecipe(recipeId);
  const [confirming, setConfirming] = useState(false);

  if (query.isLoading) {
    return <p className="muted">Chargement…</p>;
  }

  if (query.isError || !query.data) {
    return (
      <div className="empty-state">
        <div className="empty-state__emoji">🤷</div>
        <h2>Recette introuvable</h2>
        <p>
          <Link to="/recipes">Retour aux recettes</Link>
        </p>
      </div>
    );
  }

  const recipe = query.data;
  const time = totalTime(recipe);

  function onDelete() {
    remove.mutate(undefined, {
      onSuccess: () => navigate({ to: "/recipes" }),
    });
  }

  return (
    <section className="recipe-detail">
      <div className="detail-topbar">
        <Link to="/recipes" className="link-back">
          ← Recettes
        </Link>
        <Link to="/recipes/$recipeId/edit" params={{ recipeId }} className="btn">
          Modifier
        </Link>
      </div>

      <div className="recipe-detail__photo">
        {recipe.photo ? <img src={recipe.photo} alt="" /> : <span aria-hidden="true">🍽️</span>}
      </div>

      <h1 className="recipe-detail__title">{recipe.title}</h1>
      {time && <p className="recipe-detail__time muted">⏱️ {time}</p>}

      <h2 className="recipe-detail__section">Ingrédients</h2>
      {recipe.ingredients.length ? (
        <ul className="ingredient-list">
          {recipe.ingredients.map((ingredient, index) => (
            <li key={index}>
              <span className="ingredient-list__qty">{quantityLabel(ingredient)}</span>
              <span>{ingredient.name}</span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted">Aucun ingrédient.</p>
      )}

      <h2 className="recipe-detail__section">Préparation</h2>
      {recipe.steps.length ? (
        <ol className="step-list">
          {recipe.steps.map((step, index) => (
            <li key={index}>{step}</li>
          ))}
        </ol>
      ) : (
        <p className="muted">Aucune étape.</p>
      )}

      <div className="recipe-detail__danger">
        {confirming ? (
          <div className="confirm">
            <p>Supprimer « {recipe.title} » ? Cette action est définitive.</p>
            <div className="confirm__actions">
              <button className="btn" type="button" onClick={() => setConfirming(false)}>
                Annuler
              </button>
              <button
                className="btn btn--danger"
                type="button"
                onClick={onDelete}
                disabled={remove.isPending}
              >
                {remove.isPending ? "Suppression…" : "Supprimer"}
              </button>
            </div>
            {remove.isError && (
              <p className="form-error" role="alert">
                Suppression impossible. Réessayez.
              </p>
            )}
          </div>
        ) : (
          <button className="btn btn--danger-ghost" type="button" onClick={() => setConfirming(true)}>
            Supprimer la recette
          </button>
        )}
      </div>
    </section>
  );
}
