import type { ReactNode } from "react";
import type { ThemePreference } from "../theme/theme-context";

/**
 * Pictos de thème, en trait et héritant de `currentColor` (même style que la
 * barre d'onglets). Partagés par le menu « ⋮ » et l'écran Paramètres pour que
 * clair / système / sombre se reconnaissent au même dessin partout.
 */
const stroke = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

export const THEME_ICONS: Record<ThemePreference, ReactNode> = {
  // Soleil.
  light: (
    <svg viewBox="0 0 24 24" aria-hidden="true" width="18" height="18">
      <circle cx="12" cy="12" r="4" {...stroke} />
      <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M18.4 5.6L17 7M7 17l-1.4 1.4" {...stroke} />
    </svg>
  ),
  // Écran : suit le réglage de l'appareil.
  system: (
    <svg viewBox="0 0 24 24" aria-hidden="true" width="18" height="18">
      <path d="M3 5h18v11H3zM9 20h6M12 16v4" {...stroke} />
    </svg>
  ),
  // Croissant de lune.
  dark: (
    <svg viewBox="0 0 24 24" aria-hidden="true" width="18" height="18">
      <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z" {...stroke} />
    </svg>
  ),
};
