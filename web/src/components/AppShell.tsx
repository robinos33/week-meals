import { Outlet } from "@tanstack/react-router";
import { AppMenu } from "./AppMenu";
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
      <AppMenu />
      <main className="app-shell__content">
        <Outlet />
      </main>
      <TabBar />
      <InstallPrompt />
      <ReloadPrompt />
    </div>
  );
}
