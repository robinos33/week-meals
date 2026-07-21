import { useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useGenerateList } from "../api/shopping-list";
import { DEFAULT_WEEK_START_DAY, useHouseholdSettings } from "../api/household";
import {
  useClearEntry,
  useRecipeSummaries,
  useSetEntry,
  useWeekPlan,
  type MealSlot,
  type RecipeSummary,
} from "../api/meal-plan";
import "./screens.css";

/** Noms des jours indexés par `Date.getDay()` (0 = dimanche … 6 = samedi). */
const DAY_NAMES = ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"];
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

/**
 * Les 7 jours de la semaine décalée de `weekOffset`, à partir de `startDay`
 * (premier jour du foyer, convention `Date.getDay()` : 0 = dimanche … 6 =
 * samedi). Le nom de chaque jour se déduit de son propre `getDay()`, si bien
 * que le tableau tourne tout seul quand le foyer change de premier jour.
 */
function weekDays(
  weekOffset: number,
  startDay: number,
): { name: string; date: string; label: string }[] {
  const today = new Date();
  const start = new Date(today);
  // Nombre de jours écoulés depuis la dernière occurrence de `startDay`.
  const offset = (today.getDay() - startDay + 7) % 7;
  start.setDate(today.getDate() - offset + weekOffset * 7);
  return Array.from({ length: 7 }, (_, index) => {
    const day = new Date(start);
    day.setDate(start.getDate() + index);
    return {
      name: DAY_NAMES[day.getDay()],
      date: isoDate(day),
      label: `${day.getDate()}/${day.getMonth() + 1}`,
    };
  });
}

/** Onglet Semaine : 7 jours × 2 créneaux (midi / soir), remplis via un picker. */
export function WeekScreen() {
  const [weekOffset, setWeekOffset] = useState(0);
  const settings = useHouseholdSettings();
  const startDay = settings.data?.week_start_day ?? DEFAULT_WEEK_START_DAY;
  const days = useMemo(() => weekDays(weekOffset, startDay), [weekOffset, startDay]);
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
          <svg viewBox="0 0 24 24" aria-hidden="true" width="18" height="18">
            <path
              d="M6 8h12l-1 11H7L6 8zM9 8a3 3 0 0 1 6 0M12 11.5v4M10 13.5h4"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          {generateList.isPending ? "Génération…" : "Générer la liste"}
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
