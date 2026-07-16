import { createContext, useContext } from "react";

/**
 * Contexte du thème, séparé du composant `ThemeProvider` : un fichier qui
 * exporte à la fois un composant et un hook casse le Fast Refresh de Vite
 * (cf. `react-refresh/only-export-components`).
 */

/** Préférence de thème : suivre le système, ou forcer clair / sombre. */
export type ThemePreference = "system" | "light" | "dark";

/** Thème effectivement appliqué — « system » une fois résolu. */
export type ResolvedTheme = "light" | "dark";

/** Clé de persistance. Partagée avec le script inline d'`index.html`. */
export const STORAGE_KEY = "week-meals.theme";

/** Couleur de la barre système, par thème résolu. Alignée sur `--color-bg`. */
export const THEME_COLOR: Record<ResolvedTheme, string> = {
  light: "#3f7d54",
  dark: "#1c2a22",
};

export interface ThemeContextValue {
  /** Préférence choisie par l'utilisateur. */
  preference: ThemePreference;
  /** Thème effectivement appliqué (résolution de « system »). */
  resolved: ResolvedTheme;
  /** Change la préférence (persistée). */
  setPreference: (preference: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

/** Accès au thème courant depuis les composants. */
export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme doit être utilisé dans un <ThemeProvider>");
  }
  return context;
}
