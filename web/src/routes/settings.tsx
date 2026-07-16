import { useNavigate } from "@tanstack/react-router";
import { api } from "../api/client";
import { clearSession } from "../api/session";
import { queryClient } from "../query";
import { useTheme, type ThemePreference } from "../theme/ThemeProvider";
import "./screens.css";

const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Clair" },
  { value: "system", label: "Système" },
  { value: "dark", label: "Sombre" },
];

/** Onglet Paramètres : compte, invitation, préférences (thème). */
export function SettingsScreen() {
  const navigate = useNavigate();
  const { preference, setPreference } = useTheme();

  async function logout() {
    try {
      await api.post("/auth/logout");
    } finally {
      // Purge tout le cache, pas seulement la session : les recettes du foyer
      // qu'on quitte ne doivent pas rester lisibles au prochain compte connecté.
      clearSession();
      queryClient.clear();
      await navigate({ to: "/login" });
    }
  }

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Paramètres</h1>
      </header>

      <div className="card settings-section">
        <h2>Apparence</h2>
        <div
          className="segmented"
          role="group"
          aria-label="Thème de l'application"
        >
          {THEME_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              data-active={preference === option.value}
              aria-pressed={preference === option.value}
              onClick={() => setPreference(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>
        <p className="muted" style={{ marginTop: "0.6rem", fontSize: "0.85rem" }}>
          « Système » suit le réglage clair/sombre de votre appareil.
        </p>
      </div>

      <div className="card settings-section">
        <h2>Foyer</h2>
        <button className="btn" type="button">
          Générer un lien d'invitation
        </button>
      </div>

      <div className="card settings-section">
        <h2>Compte</h2>
        <button className="btn" type="button" onClick={logout}>
          Se déconnecter
        </button>
      </div>
    </section>
  );
}
