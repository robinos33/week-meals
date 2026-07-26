import { useState } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import {
  UNITS,
  totalTime,
  useDeleteRecipe,
  useRecipe,
  type Ingredient,
} from "../api/recipes";
import { formatAmount, formatDecimal } from "../lib/quantity";
import "./screens.css";

/**
 * Libellé lisible d'une quantité mise à l'échelle (« 600 g », « ½ pièce(s) »).
 * `factor` = personnes voulues / base de la recette. Fractions pour les pièces,
 * décimal pour masses et volumes.
 */
function quantityLabel(ingredient: Ingredient, factor: number): string {
  const unit = UNITS.find((u) => u.value === ingredient.unit)?.label ?? ingredient.unit;
  const scaled = ingredient.amount * factor;
  const amount = ingredient.unit === "piece" ? formatAmount(scaled) : formatDecimal(scaled);
  return `${amount} ${unit}`;
}

/** Écran détail d'une recette (`/recipes/$recipeId`). */
export function RecipeDetailScreen() {
  const { recipeId } = useParams({ from: "/recipes/$recipeId" });
  const navigate = useNavigate();
  const query = useRecipe(recipeId);
  const remove = useDeleteRecipe(recipeId);
  const [confirming, setConfirming] = useState(false);
  // Nombre de convives choisi pour l'affichage des quantités (null = base de la
  // recette, tant que l'utilisateur n'a pas ajusté). Purement local : rien
  // n'est persisté, c'est une aide à la préparation.
  const [people, setPeople] = useState<number | null>(null);

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
  const effectivePeople = people ?? recipe.servings;
  const factor = effectivePeople / recipe.servings;

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
      <p className="recipe-detail__meta muted">
        {time && <span>⏱️ {time}</span>}
        {recipe.cooked_count > 0 && (
          <span>
            🍳 Cuisiné {recipe.cooked_count} fois
          </span>
        )}
      </p>

      <div className="recipe-detail__section-head">
        <h2 className="recipe-detail__section">Ingrédients</h2>
        <div className="servings-picker" role="group" aria-label="Nombre de personnes">
          <button
            type="button"
            className="stepper__btn"
            aria-label="Moins de personnes"
            disabled={effectivePeople <= 1}
            onClick={() => setPeople(Math.max(1, effectivePeople - 1))}
          >
            −
          </button>
          <span className="servings-picker__value">
            👥 {effectivePeople} pers.
          </span>
          <button
            type="button"
            className="stepper__btn"
            aria-label="Plus de personnes"
            onClick={() => setPeople(effectivePeople + 1)}
          >
            +
          </button>
        </div>
      </div>
      {recipe.ingredients.length ? (
        <ul className="ingredient-list">
          {recipe.ingredients.map((ingredient, index) => (
            <li key={index}>
              <span className="ingredient-list__qty">{quantityLabel(ingredient, factor)}</span>
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
