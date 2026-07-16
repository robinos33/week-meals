import { useNavigate } from "@tanstack/react-router";
import { api } from "../api/client";
import "./screens.css";

/** Onglet Paramètres : compte, invitation, préférences. */
export function SettingsScreen() {
  const navigate = useNavigate();

  async function logout() {
    try {
      await api.post("/auth/logout");
    } finally {
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
