import { useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../api/client";
import {
  authApi,
  enrollDevice,
  loginWithPasskey,
  WebAuthnError,
} from "../api/auth";
import "./auth.css";

/**
 * Garde d'authentification (cf. ADR-0006). Interroge `/auth/me` :
 *  - en mode public (dev), l'API renvoie le foyer de démo → l'app s'affiche ;
 *  - authentifié → l'app s'affiche ;
 *  - sinon → écran « Continuer avec Face ID », complété d'un formulaire
 *    d'enrôlement si une fenêtre est ouverte.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const { data, isPending, error } = useQuery({
    queryKey: ["me"],
    queryFn: authApi.me,
    retry: false,
  });

  if (isPending) {
    return <Splash />;
  }
  if (data) {
    return <>{children}</>;
  }
  // 401 attendu (non connecté) ; toute autre erreur reste un souci technique.
  if (error instanceof ApiError && error.status === 401) {
    return <AuthScreen />;
  }
  return <Splash message="Connexion au serveur impossible pour le moment." />;
}

/** Voile de chargement / message neutre. */
function Splash({ message }: { message?: string }) {
  return (
    <div className="auth-splash">
      <span className="auth-splash__logo" aria-hidden="true">
        🥘
      </span>
      {message && <p className="muted">{message}</p>}
    </div>
  );
}

/** Écran non authentifié : login par passkey + enrôlement si fenêtre ouverte. */
function AuthScreen() {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const enrollStatus = useQuery({
    queryKey: ["enroll-status"],
    queryFn: authApi.enrollStatus,
    retry: false,
  });

  /** Rejoue `/auth/me` une fois la cérémonie réussie : la garde bascule. */
  const onAuthenticated = () => queryClient.invalidateQueries({ queryKey: ["me"] });

  const handleError = (err: unknown) => {
    if (err instanceof WebAuthnError || (err instanceof DOMException && err.name === "NotAllowedError")) {
      setMessage("Cérémonie annulée ou interrompue. Réessayez.");
    } else if (err instanceof ApiError && err.status === 401) {
      setMessage("Aucune passkey reconnue sur cet appareil.");
    } else if (err instanceof ApiError && err.status === 403) {
      setMessage("Code d'appairage invalide ou fenêtre fermée.");
    } else {
      setMessage("Une erreur est survenue. Réessayez.");
    }
  };

  const login = async () => {
    setBusy(true);
    setMessage(null);
    try {
      await loginWithPasskey();
      onAuthenticated();
    } catch (err) {
      handleError(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="auth-screen">
      <div className="auth-card card">
        <span className="auth-card__logo" aria-hidden="true">
          🥘
        </span>
        <h1 className="auth-card__title">Week Meals</h1>
        <p className="muted">Cette application est réservée aux appareils enrôlés.</p>

        <button className="btn btn--primary auth-card__cta" type="button" onClick={login} disabled={busy}>
          Continuer avec Face ID
        </button>

        {enrollStatus.data?.open ? (
          <EnrollForm busy={busy} setBusy={setBusy} onError={handleError} onDone={onAuthenticated} />
        ) : (
          <p className="muted auth-card__hint">
            Nouvel appareil ? Ouvrez une fenêtre d'enrôlement sur le serveur
            (<code>weekmeals device open-window</code>).
          </p>
        )}

        {message && (
          <p className="auth-card__message" role="alert">
            {message}
          </p>
        )}
      </div>
    </div>
  );
}

/** Formulaire d'enrôlement : code d'appairage + libellé de l'appareil. */
function EnrollForm({
  busy,
  setBusy,
  onError,
  onDone,
}: {
  busy: boolean;
  setBusy: (value: boolean) => void;
  onError: (err: unknown) => void;
  onDone: () => void;
}) {
  const [code, setCode] = useState("");
  const [label, setLabel] = useState("");

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      await enrollDevice(code.trim(), label.trim());
      onDone();
    } catch (err) {
      onError(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="auth-enroll" onSubmit={submit}>
      <hr className="auth-enroll__sep" />
      <h2 className="auth-enroll__title">Enrôler cet appareil</h2>
      <label className="auth-enroll__field">
        <span>Nom de l'appareil</span>
        <input
          type="text"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="iPhone de Robin"
          autoComplete="off"
          required
        />
      </label>
      <label className="auth-enroll__field">
        <span>Code d'appairage</span>
        <input
          type="text"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="XXXX-XXXX"
          autoComplete="off"
          autoCapitalize="characters"
          required
        />
      </label>
      <button className="btn btn--primary" type="submit" disabled={busy || !code || !label}>
        Enrôler
      </button>
    </form>
  );
}
