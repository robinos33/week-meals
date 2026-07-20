import { useTheme, type ThemePreference } from "../theme/theme-context";
import "./screens.css";

const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Clair" },
  { value: "system", label: "Système" },
  { value: "dark", label: "Sombre" },
];

/** Onglet Paramètres : apparence (thème) et foyer. */
// Mode public : pas de section « Compte » ni de déconnexion (aucune session) ;
// elle reviendra avec la garde d'auth (cf. AUTH_DISABLED).
export function SettingsScreen() {
  const { preference, setPreference } = useTheme();

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
    </section>
  );
}
