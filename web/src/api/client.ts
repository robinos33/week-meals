/**
 * Client HTTP de l'API Week Meals.
 *
 * L'URL de base vient de `VITE_API_URL` (config par environnement, #23) ; vide
 * en dev si un proxy Vite est utilisé. Les requêtes envoient le cookie de
 * session (`credentials: "include"`) — l'API autorise l'origine du front via
 * CORS.
 */

const BASE_URL = (import.meta.env.VITE_API_URL ?? "").replace(/\/$/, "");

/** Erreur d'appel API portant le code HTTP. */
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    ...init,
    credentials: "include",
    headers: {
      // Uniquement quand il y a un corps : `Content-Type: application/json`
      // n'est pas une valeur safelistée CORS, elle déclencherait un préflight
      // `OPTIONS` sur chaque GET — or le front et l'API sont sur des origines
      // distinctes en prod (cf. docs/deploiement-front.md).
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...(init?.headers ?? {}),
    },
  });

  if (!response.ok) {
    let message = response.statusText;
    try {
      const body = await response.json();
      if (body && typeof body.error === "string") message = body.error;
    } catch {
      // Corps non-JSON : on garde le statusText.
    }
    throw new ApiError(response.status, message);
  }

  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: "POST", body: body ? JSON.stringify(body) : undefined }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: "PUT", body: body ? JSON.stringify(body) : undefined }),
  delete: <T>(path: string) => request<T>(path, { method: "DELETE" }),
};
