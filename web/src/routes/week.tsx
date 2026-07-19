import { useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useGenerateList } from "../api/shopping-list";
import {
  useClearEntry,
  useRecipeSummaries,
  useSetEntry,
  useWeekPlan,
  type MealSlot,
  type RecipeSummary,
} from "../api/meal-plan";
import "./screens.css";

const DAY_NAMES = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const SLOTS: { slot: MealSlot; label: string }[] = [
  { slot: "lunch", label: "Midi" },
  { slot: "dinner", label: "Soir" },
];

/** Date locale au format ISO `YYYY-MM-DD` (sans décalage UTC de `toISOString`). */
function isoDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

/** Les 7 jours de la semaine décalée de `weekOffset`, à partir du lundi. */
function weekDays(weekOffset: number): { name: string; date: string; label: string }[] {
  const today = new Date();
  const monday = new Date(today);
  const offset = (today.getDay() + 6) % 7; // 0 = lundi
  monday.setDate(today.getDate() - offset + weekOffset * 7);
  return DAY_NAMES.map((name, index) => {
    const day = new Date(monday);
    day.setDate(monday.getDate() + index);
    return { name, date: isoDate(day), label: `${day.getDate()}/${day.getMonth() + 1}` };
  });
}

/** Onglet Semaine : 7 jours × 2 créneaux (midi / soir), remplis via un picker. */
export function WeekScreen() {
  const [weekOffset, setWeekOffset] = useState(0);
  const days = useMemo(() => weekDays(weekOffset), [weekOffset]);
  const from = days[0].date;
  const to = days[6].date;

  const plan = useWeekPlan(from, to);
  const recipesQuery = useRecipeSummaries();
  const setEntry = useSetEntry();
  const clearEntry = useClearEntry();
  const generateList = useGenerateList();
  const navigate = useNavigate();

  // Créneau en cours de remplissage (picker ouvert), ou `null`.
  const [picking, setPicking] = useState<{ date: string; slot: MealSlot } | null>(null);

  const recipesById = useMemo(() => {
    const map = new Map<string, RecipeSummary>();
    for (const recipe of recipesQuery.data ?? []) map.set(recipe.id, recipe);
    return map;
  }, [recipesQuery.data]);

  const planBySlot = useMemo(() => {
    const map = new Map<string, string>();
    for (const entry of plan.data ?? []) map.set(`${entry.date}|${entry.slot}`, entry.recipe_id);
    return map;
  }, [plan.data]);

  function choose(recipeId: string) {
    if (!picking) return;
    setEntry.mutate({ ...picking, recipe_id: recipeId });
    setPicking(null);
  }

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Semaine</h1>
        <button
          className="btn"
          type="button"
          disabled={generateList.isPending}
          onClick={() =>
            // Génère sur la semaine affichée, puis bascule sur l'onglet Courses.
            generateList.mutate(
              { from, to },
              { onSuccess: () => void navigate({ to: "/shopping" }) },
            )
          }
        >
          {generateList.isPending ? "Génération…" : "Liste de courses"}
        </button>
      </header>

      <div className="week-nav">
        <button
          className="icon-btn"
          type="button"
          aria-label="Semaine précédente"
          onClick={() => setWeekOffset((o) => o - 1)}
        >
          ‹
        </button>
        <span className="week-nav__label">
          {weekOffset === 0 ? "Cette semaine" : `${days[0].label} – ${days[6].label}`}
        </span>
        <button
          className="icon-btn"
          type="button"
          aria-label="Semaine suivante"
          onClick={() => setWeekOffset((o) => o + 1)}
        >
          ›
        </button>
      </div>

      {days.map((day) => (
        <div className="week-day" key={day.date}>
          <div className="week-day__name">
            {day.name} <span className="muted">{day.label}</span>
          </div>
          <div className="week-day__slots">
            {SLOTS.map(({ slot, label }) => {
              const recipeId = planBySlot.get(`${day.date}|${slot}`);
              const recipe = recipeId ? recipesById.get(recipeId) : undefined;
              if (recipeId) {
                return (
                  <div className="slot slot--filled" key={slot}>
                    <span className="slot__label">{label}</span>
                    <div className="slot__recipe">
                      <div className="slot__thumb">
                        {recipe?.photo ? <img src={recipe.photo} alt="" /> : "🍽️"}
                      </div>
                      <span className="slot__title">{recipe?.title ?? "Recette"}</span>
                      <button
                        className="slot__remove"
                        type="button"
                        aria-label="Retirer du créneau"
                        onClick={() => clearEntry.mutate({ date: day.date, slot })}
                      >
                        ×
                      </button>
                    </div>
                  </div>
                );
              }
              return (
                <button
                  className="slot"
                  type="button"
                  key={slot}
                  onClick={() => setPicking({ date: day.date, slot })}
                >
                  <span className="slot__label">{label}</span>
                  <span>+ Ajouter</span>
                </button>
              );
            })}
          </div>
        </div>
      ))}

      {picking && (
        <RecipePicker
          recipes={recipesQuery.data ?? []}
          loading={recipesQuery.isLoading}
          onPick={choose}
          onClose={() => setPicking(null)}
        />
      )}
    </section>
  );
}

/** Feuille de sélection d'une recette pour un créneau (recherche incluse). */
function RecipePicker({
  recipes,
  loading,
  onPick,
  onClose,
}: {
  recipes: RecipeSummary[];
  loading: boolean;
  onPick: (recipeId: string) => void;
  onClose: () => void;
}) {
  const [search, setSearch] = useState("");
  const filtered = recipes.filter((r) =>
    r.title.toLowerCase().includes(search.trim().toLowerCase()),
  );

  return (
    <div
      className="sheet-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label="Choisir une recette"
      onClick={onClose}
    >
      <div className="sheet" onClick={(e) => e.stopPropagation()}>
        <div className="sheet__handle" aria-hidden="true" />
        <div className="sheet__header">
          <h2>Choisir une recette</h2>
          <button className="icon-btn" type="button" aria-label="Fermer" onClick={onClose}>
            ×
          </button>
        </div>
        <input
          className="input"
          type="search"
          placeholder="Rechercher…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Rechercher une recette"
          autoFocus
        />
        <div className="sheet__list">
          {loading ? (
            <p className="muted">Chargement…</p>
          ) : filtered.length === 0 ? (
            <p className="muted">Aucune recette. Ajoutez-en dans l'onglet Recettes.</p>
          ) : (
            filtered.map((recipe) => (
              <button
                className="picker-row"
                type="button"
                key={recipe.id}
                onClick={() => onPick(recipe.id)}
              >
                <div className="slot__thumb">
                  {recipe.photo ? <img src={recipe.photo} alt="" /> : "🍽️"}
                </div>
                <span>{recipe.title}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
