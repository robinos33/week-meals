import { useState, type ChangeEvent, type FormEvent, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import {
  UNITS,
  uploadPhoto,
  useCreateRecipe,
  useRecipe,
  useUpdateRecipe,
  type RecipeInput,
  type RecipeView,
  type Unit,
} from "../api/recipes";
import { ApiError } from "../api/client";
import "./screens.css";

/** Ligne d'ingrédient éditable ; `key` local stable pour la liste React. */
interface IngredientRow {
  key: number;
  name: string;
  amount: string;
  unit: Unit;
}

/** Ligne d'étape éditable. */
interface StepRow {
  key: number;
  text: string;
}

let nextKey = 0;
const newKey = () => (nextKey += 1);

const emptyIngredient = (): IngredientRow => ({
  key: newKey(),
  name: "",
  amount: "",
  unit: "g",
});
const emptyStep = (): StepRow => ({ key: newKey(), text: "" });

/** Champ « minutes » optionnel, converti en `number | null`. */
function toMinutes(value: string): number | null {
  const n = Number(value);
  return value.trim() && Number.isFinite(n) && n >= 0 ? Math.round(n) : null;
}

/**
 * Formulaire partagé création / édition. Contrôlé, avec listes dynamiques
 * d'ingrédients et d'étapes. Les lignes vides sont ignorées à l'envoi.
 */
function RecipeForm({
  heading,
  initial,
  submitting,
  error,
  onSubmit,
}: {
  heading: string;
  initial?: RecipeView;
  submitting: boolean;
  error: string;
  onSubmit: (input: RecipeInput) => void;
}) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [prep, setPrep] = useState(
    initial?.prep_time_min != null ? String(initial.prep_time_min) : "",
  );
  const [cook, setCook] = useState(
    initial?.cook_time_min != null ? String(initial.cook_time_min) : "",
  );
  const [photo, setPhoto] = useState(initial?.photo ?? "");
  const [uploading, setUploading] = useState(false);
  const [photoError, setPhotoError] = useState("");
  const [showPhotoUrl, setShowPhotoUrl] = useState(false);
  const [ingredients, setIngredients] = useState<IngredientRow[]>(
    initial?.ingredients.length
      ? initial.ingredients.map((i) => ({
          key: newKey(),
          name: i.name,
          amount: String(i.amount),
          unit: i.unit,
        }))
      : [emptyIngredient()],
  );
  const [steps, setSteps] = useState<StepRow[]>(
    initial?.steps.length
      ? initial.steps.map((text) => ({ key: newKey(), text }))
      : [emptyStep()],
  );

  function patchIngredient(key: number, patch: Partial<IngredientRow>) {
    setIngredients((rows) => rows.map((r) => (r.key === key ? { ...r, ...patch } : r)));
  }

  async function onPhotoSelected(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = ""; // autorise la re-sélection du même fichier
    if (!file) return;
    setPhotoError("");
    setUploading(true);
    try {
      setPhoto(await uploadPhoto(file));
    } catch (err) {
      if (err instanceof ApiError && err.status === 503) {
        setShowPhotoUrl(true);
        setPhotoError("Stockage photo indisponible — collez une URL à la place.");
      } else {
        setPhotoError("Échec de l'upload. Réessayez.");
      }
    } finally {
      setUploading(false);
    }
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const input: RecipeInput = {
      title: title.trim(),
      prep_time_min: toMinutes(prep),
      cook_time_min: toMinutes(cook),
      photo: photo.trim() || null,
      // On ne garde que les lignes exploitables : un nom et une quantité > 0.
      ingredients: ingredients
        .map((r) => ({ name: r.name.trim(), amount: Number(r.amount), unit: r.unit }))
        .filter((i) => i.name && Number.isFinite(i.amount) && i.amount > 0),
      steps: steps.map((s) => s.text.trim()).filter(Boolean),
    };
    onSubmit(input);
  }

  const cancelTo = initial ? `/recipes/${initial.id}` : "/recipes";

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">{heading}</h1>
      </header>

      <form className="recipe-form" onSubmit={submit}>
        <div className="field">
          <label htmlFor="title">Titre</label>
          <input
            id="title"
            className="input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
            autoFocus
          />
        </div>

        <div className="field-row">
          <div className="field">
            <label htmlFor="prep">Préparation (min)</label>
            <input
              id="prep"
              className="input"
              type="number"
              min="0"
              inputMode="numeric"
              value={prep}
              onChange={(e) => setPrep(e.target.value)}
            />
          </div>
          <div className="field">
            <label htmlFor="cook">Cuisson (min)</label>
            <input
              id="cook"
              className="input"
              type="number"
              min="0"
              inputMode="numeric"
              value={cook}
              onChange={(e) => setCook(e.target.value)}
            />
          </div>
        </div>

        <div className="field">
          <span className="field-label">Photo</span>
          <div className="photo-upload">
            {photo && (
              <div className="photo-upload__preview">
                <img src={photo} alt="" />
                <button
                  type="button"
                  className="btn btn--danger-ghost"
                  onClick={() => setPhoto("")}
                >
                  Retirer
                </button>
              </div>
            )}
            <label className="btn photo-upload__pick">
              {uploading ? "Envoi…" : photo ? "Changer la photo" : "Choisir une photo"}
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp"
                onChange={onPhotoSelected}
                disabled={uploading}
                className="visually-hidden"
              />
            </label>
            {showPhotoUrl && (
              <input
                className="input"
                type="url"
                placeholder="https://…"
                value={photo}
                onChange={(e) => setPhoto(e.target.value)}
                aria-label="URL de la photo"
              />
            )}
            {photoError && (
              <p className="form-error" role="alert">
                {photoError}
              </p>
            )}
          </div>
        </div>

        <fieldset className="form-group">
          <legend>Ingrédients</legend>
          {ingredients.map((row) => (
            <div className="ingredient-row" key={row.key}>
              <input
                className="input"
                aria-label="Nom de l'ingrédient"
                placeholder="Courgette"
                value={row.name}
                onChange={(e) => patchIngredient(row.key, { name: e.target.value })}
              />
              <input
                className="input input--amount"
                aria-label="Quantité"
                type="number"
                min="0"
                step="any"
                inputMode="decimal"
                placeholder="0"
                value={row.amount}
                onChange={(e) => patchIngredient(row.key, { amount: e.target.value })}
              />
              <select
                className="input input--unit"
                aria-label="Unité"
                value={row.unit}
                onChange={(e) => patchIngredient(row.key, { unit: e.target.value as Unit })}
              >
                {UNITS.map((u) => (
                  <option key={u.value} value={u.value}>
                    {u.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="icon-btn"
                aria-label="Retirer l'ingrédient"
                onClick={() =>
                  setIngredients((rows) =>
                    rows.length > 1 ? rows.filter((r) => r.key !== row.key) : rows,
                  )
                }
              >
                ×
              </button>
            </div>
          ))}
          <button
            type="button"
            className="btn add-row"
            onClick={() => setIngredients((rows) => [...rows, emptyIngredient()])}
          >
            + Ajouter un ingrédient
          </button>
        </fieldset>

        <fieldset className="form-group">
          <legend>Étapes</legend>
          {steps.map((row, index) => (
            <div className="step-row" key={row.key}>
              <span className="step-row__num" aria-hidden="true">
                {index + 1}
              </span>
              <textarea
                className="input step-row__text"
                aria-label={`Étape ${index + 1}`}
                rows={2}
                placeholder="Décrire l'étape…"
                value={row.text}
                onChange={(e) =>
                  setSteps((rows) =>
                    rows.map((r) => (r.key === row.key ? { ...r, text: e.target.value } : r)),
                  )
                }
              />
              <button
                type="button"
                className="icon-btn"
                aria-label="Retirer l'étape"
                onClick={() =>
                  setSteps((rows) =>
                    rows.length > 1 ? rows.filter((r) => r.key !== row.key) : rows,
                  )
                }
              >
                ×
              </button>
            </div>
          ))}
          <button
            type="button"
            className="btn add-row"
            onClick={() => setSteps((rows) => [...rows, emptyStep()])}
          >
            + Ajouter une étape
          </button>
        </fieldset>

        <p className="form-error" role="alert">
          {error}
        </p>

        <div className="form-actions">
          <Link to={cancelTo} className="btn">
            Annuler
          </Link>
          <button
            className="btn btn--primary"
            type="submit"
            disabled={submitting || !title.trim()}
          >
            {submitting ? "Enregistrement…" : "Enregistrer"}
          </button>
        </div>
      </form>
    </section>
  );
}

/** Message d'erreur uniforme pour les mutations recette. */
function mutationError(err: unknown): string {
  if (err instanceof ApiError && err.status === 422) return err.message;
  return "Enregistrement impossible. Réessayez.";
}

/** Écran de création (`/recipes/new`). */
export function NewRecipeScreen() {
  const navigate = useNavigate();
  const create = useCreateRecipe();
  return (
    <RecipeForm
      heading="Nouvelle recette"
      submitting={create.isPending}
      error={create.isError ? mutationError(create.error) : ""}
      onSubmit={(input) =>
        create.mutate(input, {
          onSuccess: (recipe) => navigate({ to: `/recipes/${recipe.id}` }),
        })
      }
    />
  );
}

/** Écran d'édition (`/recipes/$recipeId/edit`). */
export function EditRecipeScreen() {
  const { recipeId } = useParams({ from: "/recipes/$recipeId/edit" });
  const navigate = useNavigate();
  const query = useRecipe(recipeId);
  const update = useUpdateRecipe(recipeId);

  if (query.isLoading) return <Loader />;
  if (query.isError || !query.data) return <NotFound />;

  return (
    <RecipeForm
      heading="Modifier la recette"
      initial={query.data}
      submitting={update.isPending}
      error={update.isError ? mutationError(update.error) : ""}
      onSubmit={(input) =>
        update.mutate(input, {
          onSuccess: () => navigate({ to: `/recipes/${recipeId}` }),
        })
      }
    />
  );
}

function Loader(): ReactNode {
  return <p className="muted">Chargement…</p>;
}

function NotFound(): ReactNode {
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
