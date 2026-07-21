import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { THEME_ICONS } from "../components/theme-icons";
import { useTheme, type ThemePreference } from "../theme/theme-context";
import { ApiError } from "../api/client";
import { authApi, type DeviceInfo } from "../api/auth";
import "./screens.css";

const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Clair" },
  { value: "system", label: "Système" },
  { value: "dark", label: "Sombre" },
];

/** Onglet Paramètres : apparence (thème), appareils enrôlés et déconnexion. */
export function SettingsScreen() {
  const { preference, setPreference } = useTheme();
  const queryClient = useQueryClient();
  const devices = useQuery({
    queryKey: ["devices"],
    queryFn: authApi.listDevices,
    retry: false,
  });

  const [revokeError, setRevokeError] = useState<string | null>(null);

  const revoke = async (id: string) => {
    if (!window.confirm("Révoquer cet appareil ? Il devra être ré-enrôlé.")) return;
    setRevokeError(null);
    try {
      await authApi.revokeDevice(id);
    } catch (err) {
      // 409 : c'est le dernier appareil du foyer, l'API refuse le verrouillage.
      setRevokeError(
        err instanceof ApiError && err.status === 409
          ? "Impossible de révoquer le dernier appareil : personne ne pourrait plus se connecter. Enrôlez-en un autre d'abord."
          : "La révocation a échoué. Réessayez.",
      );
      return;
    }
    await queryClient.invalidateQueries({ queryKey: ["devices"] });
  };

  const logout = async () => {
    await authApi.logout();
    await queryClient.invalidateQueries({ queryKey: ["me"] });
  };

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Paramètres</h1>
      </header>

      <div className="card settings-section">
        <h2>Apparence</h2>
        <div className="segmented" role="group" aria-label="Thème de l'application">
          {THEME_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              data-active={preference === option.value}
              aria-pressed={preference === option.value}
              onClick={() => setPreference(option.value)}
            >
              <span className="segmented__icon">{THEME_ICONS[option.value]}</span>
              {option.label}
            </button>
          ))}
        </div>
        <p className="muted" style={{ marginTop: "0.6rem", fontSize: "0.85rem" }}>
          « Système » suit le réglage clair/sombre de votre appareil.
        </p>
      </div>

      <div className="card settings-section">
        <h2>Appareils</h2>
        {devices.data && devices.data.length > 0 ? (
          <ul className="device-list">
            {devices.data.map((device) => (
              <DeviceRow key={device.id} device={device} onRevoke={() => revoke(device.id)} />
            ))}
          </ul>
        ) : (
          <p className="muted" style={{ fontSize: "0.85rem" }}>
            Aucun appareil enrôlé. Ouvrez une fenêtre d'enrôlement sur le serveur
            (<code>weekmeals device open-window</code>).
          </p>
        )}
        {revokeError && (
          <p className="settings-error" role="alert">
            {revokeError}
          </p>
        )}
      </div>

      <div className="card settings-section">
        <h2>Compte</h2>
        <button className="btn btn--danger-ghost btn--block" type="button" onClick={logout}>
          Se déconnecter
        </button>
      </div>
    </section>
  );
}

/** Une ligne d'appareil : libellé, dernière activité, révocation. */
function DeviceRow({ device, onRevoke }: { device: DeviceInfo; onRevoke: () => void }) {
  const lastSeen = device.last_seen_at
    ? new Date(device.last_seen_at).toLocaleDateString("fr-FR", {
        day: "numeric",
        month: "short",
      })
    : "jamais utilisé";

  return (
    <li className="device-list__item">
      <div>
        <span className="device-list__label">{device.label}</span>
        <span className="muted device-list__meta">
          {device.backup_state ? "Synchronisée · " : ""}
          {lastSeen}
        </span>
      </div>
      <button
        className="btn btn--danger-ghost"
        type="button"
        onClick={onRevoke}
        aria-label={`Révoquer ${device.label}`}
      >
        Révoquer
      </button>
    </li>
  );
}
