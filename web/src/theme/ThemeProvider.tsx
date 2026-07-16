import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

/** Préférence de thème : suivre le système, ou forcer clair / sombre. */
export type ThemePreference = "system" | "light" | "dark";

const STORAGE_KEY = "week-meals.theme";

interface ThemeContextValue {
  /** Préférence choisie par l'utilisateur. */
  preference: ThemePreference;
  /** Thème effectivement appliqué (résolution de « system »). */
  resolved: "light" | "dark";
  /** Change la préférence (persistée). */
  setPreference: (preference: ThemePreference) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

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
 * Applique le thème « Cantine » : pose (ou retire) `data-theme` sur `<html>`
 * pour que la bascule prime sur `prefers-color-scheme`, et synchronise la
 * couleur de la barre système (`theme-color`).
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

  const resolved: "light" | "dark" =
    preference === "system" ? (systemDark ? "dark" : "light") : preference;

  useEffect(() => {
    const root = document.documentElement;
    if (preference === "system") {
      root.removeAttribute("data-theme");
    } else {
      root.setAttribute("data-theme", preference);
    }
    // Aligne la couleur de la barre d'état sur le fond du thème résolu.
    const themeColor = resolved === "dark" ? "#1c2a22" : "#3f7d54";
    document
      .querySelectorAll('meta[name="theme-color"]')
      .forEach((meta) => meta.setAttribute("content", themeColor));
  }, [preference, resolved]);

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

/** Accès au thème courant depuis les composants. */
export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme doit être utilisé dans un <ThemeProvider>");
  }
  return context;
}
