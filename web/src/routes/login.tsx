import { useState, type FormEvent } from "react";
import { useNavigate } from "@tanstack/react-router";
import { api, ApiError } from "../api/client";
import "./screens.css";

/** Écran de connexion : pseudo + mot de passe, sobre (pas d'inscription). */
export function LoginScreen() {
  const navigate = useNavigate();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError("");
    setPending(true);
    try {
      await api.post("/auth/login", { username, password });
      await navigate({ to: "/recipes" });
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 401
          ? "Pseudo ou mot de passe incorrect."
          : "Connexion impossible. Réessayez.",
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="login">
      <div className="login__brand">
        <h1>Week Meals</h1>
        <p className="muted">La cuisine de la semaine, à deux.</p>
      </div>
      <form onSubmit={onSubmit}>
        <div className="field">
          <label htmlFor="username">Pseudo</label>
          <input
            id="username"
            className="input"
            autoComplete="username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
        </div>
        <div className="field">
          <label htmlFor="password">Mot de passe</label>
          <input
            id="password"
            className="input"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </div>
        <p className="form-error" role="alert">
          {error}
        </p>
        <button className="btn btn--primary btn--block" type="submit" disabled={pending}>
          {pending ? "Connexion…" : "Se connecter"}
        </button>
      </form>
    </div>
  );
}
