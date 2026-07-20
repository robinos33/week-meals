/**
 * Authentification par passkeys WebAuthn (cf. ADR-0006).
 *
 * Deux cérémonies, chacune en deux temps (challenge serveur → cérémonie
 * navigateur → validation serveur). Les encodages base64url des champs binaires
 * sont pris en charge par `@simplewebauthn/browser` ; le serveur (webauthn-rs)
 * renvoie ses options sous la clé `publicKey`, exactement la forme attendue.
 */

import { startAuthentication, startRegistration } from "@simplewebauthn/browser";
import { api } from "./client";

/** Identité renvoyée par l'API (identique à `/auth/me`). */
export interface Identity {
  user_id: string;
  household_id: string;
  username: string;
}

/** Un appareil enrôlé, pour la carte Appareils des réglages. */
export interface DeviceInfo {
  id: string;
  label: string;
  backup_state: boolean;
  created_at: string;
  last_seen_at: string | null;
}

/** Options de cérémonie telles qu'attendues par `@simplewebauthn/browser`. */
type RegistrationOptions = Parameters<typeof startRegistration>[0]["optionsJSON"];
type AuthenticationOptions = Parameters<typeof startAuthentication>[0]["optionsJSON"];

/** Enveloppe WebAuthn du serveur : les options vivent sous `publicKey`. */
interface CreationChallenge {
  publicKey: RegistrationOptions;
}
interface RequestChallenge {
  publicKey: AuthenticationOptions;
}

export const authApi = {
  /** Identité courante, ou `ApiError(401)` si non authentifié. */
  me: () => api.get<Identity>("/auth/me"),
  /** La fenêtre d'enrôlement est-elle ouverte ? */
  enrollStatus: () => api.get<{ open: boolean }>("/auth/enroll/status"),
  /** Invalide la session. */
  logout: () => api.post<void>("/auth/logout"),
  /** Liste les appareils enrôlés du foyer. */
  listDevices: () => api.get<DeviceInfo[]>("/auth/devices"),
  /** Révoque un appareil. */
  revokeDevice: (id: string) => api.delete<void>(`/auth/devices/${id}`),
};

/**
 * Enrôle l'appareil courant : vérifie le code d'appairage, déclenche la
 * cérémonie d'enregistrement (Face ID / empreinte) et ouvre la session.
 */
export async function enrollDevice(code: string, label: string): Promise<Identity> {
  const challenge = await api.post<CreationChallenge>("/auth/enroll/start", { code, label });
  const credential = await startRegistration({ optionsJSON: challenge.publicKey });
  return api.post<Identity>("/auth/enroll/finish", credential);
}

/**
 * « Continuer avec Face ID » : authentification découvrable, sans identifiant.
 * Le téléphone présente la passkey de son choix.
 */
export async function loginWithPasskey(): Promise<Identity> {
  const challenge = await api.post<RequestChallenge>("/auth/login/start");
  const credential = await startAuthentication({ optionsJSON: challenge.publicKey });
  return api.post<Identity>("/auth/login/finish", credential);
}

export { WebAuthnError } from "@simplewebauthn/browser";
