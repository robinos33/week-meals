/**
 * Session courante : qui est connecté, et garde des routes protégées.
 *
 * L'API n'expose pas de token au client — le cookie de session est `HttpOnly`.
 * Le seul moyen de savoir si l'on est connecté est donc de demander à l'API
 * (`GET /auth/me`), qui répond 401 si la session est absente ou expirée.
 */

import { redirect } from "@tanstack/react-router";
import { queryClient } from "../query";
import { ApiError, api } from "./client";

/** Utilisateur connecté, tel qu'exposé par `GET /auth/me` (`UserView`). */
export interface SessionUser {
  user_id: string;
  household_id: string;
  username: string;
}

export const sessionQuery = {
  queryKey: ["session"] as const,
  queryFn: () => api.get<SessionUser>("/auth/me"),
  // Une session perdue doit se voir tout de suite : pas de réessai sur 401,
  // et pas de cache long qui laisserait naviguer dans une app déconnectée.
  retry: false,
  staleTime: 0,
};

/**
 * `beforeLoad` des routes protégées : redirige vers `/login` si la session est
 * absente. Le résultat est mis en cache par TanStack Query, donc naviguer entre
 * onglets ne relance pas un appel réseau à chaque fois.
 *
 * Toute autre erreur (API en panne) est laissée remonter : rediriger vers la
 * connexion sur un 500 ferait croire à une déconnexion.
 */
export async function requireSession(pathname: string): Promise<SessionUser> {
  try {
    return await queryClient.ensureQueryData(sessionQuery);
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      throw redirect({ to: "/login", search: { redirect: pathname } });
    }
    throw error;
  }
}

/** Vide la session en cache — à appeler après un logout ou un login. */
export function clearSession(): void {
  queryClient.removeQueries({ queryKey: sessionQuery.queryKey });
}
