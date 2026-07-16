import { useRegisterSW } from "virtual:pwa-register/react";

/**
 * Bandeau de mise à jour du service worker (#24). En mode `prompt`, quand une
 * nouvelle version est précachée, on propose à l'utilisateur de recharger —
 * plutôt que de le faire dans son dos. Affiche aussi l'état « prêt hors-ligne ».
 */
export function ReloadPrompt() {
  const {
    offlineReady: [offlineReady, setOfflineReady],
    needRefresh: [needRefresh, setNeedRefresh],
    updateServiceWorker,
  } = useRegisterSW();

  if (!offlineReady && !needRefresh) return null;

  const close = () => {
    setOfflineReady(false);
    setNeedRefresh(false);
  };

  return (
    <div className="reload-prompt card" role="status" aria-live="polite">
      <span>
        {needRefresh
          ? "Une nouvelle version est disponible."
          : "Prêt à fonctionner hors-ligne."}
      </span>
      <div className="reload-prompt__actions">
        {needRefresh && (
          <button className="btn btn--primary" onClick={() => updateServiceWorker(true)}>
            Mettre à jour
          </button>
        )}
        <button className="btn" onClick={close}>
          Fermer
        </button>
      </div>
    </div>
  );
}
