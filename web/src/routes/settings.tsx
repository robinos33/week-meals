import "./screens.css";

/** Onglet Paramètres : foyer, préférences. */
// Mode public : plus de section « Compte » ni de déconnexion — il n'y a pas de
// session. Elle reviendra avec la garde d'auth (cf. AUTH_DISABLED).
export function SettingsScreen() {
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
    </section>
  );
}
