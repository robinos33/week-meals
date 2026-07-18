import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  STORAGE_KEY,
  THEME_COLOR,
  ThemeContext,
  type ResolvedTheme,
  type ThemeContextValue,
  type ThemePreference,
} from "./theme-context";

function readStored(): ThemePreference {
  const value =
    typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  return value === "light" || value === "dark" || value === "system" ? value : "system";
}

function systemPrefersDark(): boolean {
  return (
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-color-scheme: dark)").matches
  );
}

/**
 * Applique le thème « Cantine » : pose le thème **résolu** dans `data-theme`
 * sur `<html>` — seule source lue par les tokens (cf. `tokens.css`) — et
 * synchronise la couleur de la barre système (`theme-color`).
 *
 * Le même calcul est fait par le script inline d'`index.html` avant le premier
 * paint ; ce provider prend ensuite le relais. Les deux doivent rester d'accord
 * sur la clé de stockage et les couleurs (partagées via `theme-context`).
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [preference, setPreferenceState] = useState<ThemePreference>(readStored);
  const [systemDark, setSystemDark] = useState<boolean>(systemPrefersDark);

  // Suit les changements de préférence système quand on est en mode « system ».
  useEffect(() => {
    if (typeof matchMedia === "undefined") return;
    const query = matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const resolved: ResolvedTheme =
    preference === "system" ? (systemDark ? "dark" : "light") : preference;

  useEffect(() => {
    // Toujours le thème résolu : « Système » n'est pas un état visuel, il se
    // résout en clair ou sombre. Les tokens ne lisent que `data-theme`.
    document.documentElement.setAttribute("data-theme", resolved);
    document
      .querySelectorAll('meta[name="theme-color"]')
      .forEach((meta) => meta.setAttribute("content", THEME_COLOR[resolved]));
  }, [resolved]);

  const setPreference = (next: ThemePreference) => {
    setPreferenceState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Stockage indisponible (mode privé) : la préférence reste en mémoire.
    }
  };

  const value = useMemo<ThemeContextValue>(
    () => ({ preference, resolved, setPreference }),
    [preference, resolved],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
