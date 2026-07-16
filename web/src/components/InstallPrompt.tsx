import { useEffect, useState } from "react";

/** Événement `beforeinstallprompt` (non typé par défaut dans la lib DOM). */
interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

const DISMISS_KEY = "week-meals.install-dismissed";

function isStandalone(): boolean {
  return (
    matchMedia("(display-mode: standalone)").matches ||
    // iOS Safari.
    (navigator as unknown as { standalone?: boolean }).standalone === true
  );
}

/**
 * Invite discrète à installer la PWA (#24). Utilise l'API
 * `beforeinstallprompt` (Chromium) ; masquée si l'app est déjà installée ou si
 * l'utilisateur a déjà refusé. iOS ne supporte pas l'API : on n'insiste pas.
 */
export function InstallPrompt() {
  const [deferred, setDeferred] = useState<BeforeInstallPromptEvent | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (isStandalone() || localStorage.getItem(DISMISS_KEY) === "1") return;

    const onPrompt = (event: Event) => {
      event.preventDefault();
      setDeferred(event as BeforeInstallPromptEvent);
      setVisible(true);
    };
    const onInstalled = () => setVisible(false);
    window.addEventListener("beforeinstallprompt", onPrompt);
    window.addEventListener("appinstalled", onInstalled);
    return () => {
      window.removeEventListener("beforeinstallprompt", onPrompt);
      window.removeEventListener("appinstalled", onInstalled);
    };
  }, []);

  if (!visible || !deferred) return null;

  const dismiss = () => {
    localStorage.setItem(DISMISS_KEY, "1");
    setVisible(false);
  };

  const install = async () => {
    await deferred.prompt();
    await deferred.userChoice;
    setVisible(false);
  };

  return (
    <div className="install-prompt card" role="dialog" aria-label="Installer l'application">
      <div>
        <strong>Installer Week Meals</strong>
        <p className="muted">Accès plein écran, hors-ligne pour les courses.</p>
      </div>
      <div className="install-prompt__actions">
        <button className="btn btn--primary" onClick={install}>
          Installer
        </button>
        <button className="btn" onClick={dismiss}>
          Plus tard
        </button>
      </div>
    </div>
  );
}
