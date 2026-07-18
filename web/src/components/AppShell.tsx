import { Link, Outlet } from "@tanstack/react-router";
import { InstallPrompt } from "./InstallPrompt";
import { ReloadPrompt } from "./ReloadPrompt";
import { TabBar } from "./TabBar";
import "./components.css";

/**
 * Coquille applicative : zone de contenu défilante + barre d'onglets basse,
 * en tenant compte des zones sûres (encoches) du plein écran PWA. En mode
 * public il n'y a plus d'écran plein écran (mire), la coquille est donc
 * toujours présente.
 */
export function AppShell() {
  return (
    <div className="app-shell">
      <Link to="/settings" className="app-shell__settings" aria-label="Paramètres">
        <svg viewBox="0 0 24 24" aria-hidden="true" width="22" height="22">
          <path
            d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
          />
          <path
            d="M19.4 13a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-2.87 1.21V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8 19.4a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 14a1.7 1.7 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 10 3.09V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 2.87 1.21l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 10v.09a1.7 1.7 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1z"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
          />
        </svg>
      </Link>
      <main className="app-shell__content">
        <Outlet />
      </main>
      <TabBar />
      <InstallPrompt />
      <ReloadPrompt />
    </div>
  );
}
