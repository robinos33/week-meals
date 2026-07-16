import { useNavigate } from "@tanstack/react-router";
import { api } from "../api/client";
import { clearSession } from "../api/session";
import { queryClient } from "../query";
import "./screens.css";

/** Onglet Paramètres : compte, invitation, préférences. */
export function SettingsScreen() {
  const navigate = useNavigate();

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
