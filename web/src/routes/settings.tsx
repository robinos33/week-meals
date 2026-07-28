import { useMemo, useRef, useState, type FormEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { THEME_ICONS } from "../components/theme-icons";
import { useTheme, type ThemePreference } from "../theme/theme-context";
import { api, ApiError } from "../api/client";
import { authApi, type DeviceInfo } from "../api/auth";
import { useHouseholdSettings, useSetWeekStartDay } from "../api/household";
import {
  useAisles,
  useCreateStore,
  useDeleteStore,
  useStores,
  useUpdateStore,
  type Store,
} from "../api/stores";
import { aisleEmoji } from "../lib/aisles";
import { useDragOrder } from "../lib/drag-order";
import type { RecipeView } from "../api/recipes";
import { backupFilename, buildBackup, downloadBackup } from "../lib/backup";
import {
  BackupImportError,
  importSummary,
  parseBackup,
  planImport,
} from "../lib/backup-import";
import "./screens.css";

const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Clair" },
  { value: "system", label: "Système" },
  { value: "dark", label: "Sombre" },
];

/** Jours de la semaine, indexés par la convention `Date.getDay()`. */
const WEEK_DAY_OPTIONS: { value: number; label: string }[] = [
  { value: 1, label: "Lundi" },
  { value: 2, label: "Mardi" },
  { value: 3, label: "Mercredi" },
  { value: 4, label: "Jeudi" },
  { value: 5, label: "Vendredi" },
  { value: 6, label: "Samedi" },
  { value: 0, label: "Dimanche" },
];

/**
 * Onglet Paramètres : apparence (thème), semaine, magasins, appareils enrôlés,
 * sauvegarde et déconnexion.
 */
export function SettingsScreen() {
  const { preference, setPreference } = useTheme();
  const queryClient = useQueryClient();
  const householdSettings = useHouseholdSettings();
  const setWeekStartDay = useSetWeekStartDay();
  const devices = useQuery({
    queryKey: ["devices"],
    queryFn: authApi.listDevices,
    retry: false,
  });

  const [revokeError, setRevokeError] = useState<string | null>(null);
  const [backupError, setBackupError] = useState<string | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);

  const [importError, setImportError] = useState<string | null>(null);
  const [importResult, setImportResult] = useState<string | null>(null);
  const [importBusy, setImportBusy] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  const backup = async () => {
    setBackupError(null);
    setBackupBusy(true);
    try {
      const recipes = await api.get<RecipeView[]>("/recipes");
      downloadBackup(buildBackup(recipes), backupFilename());
    } catch {
      setBackupError("La sauvegarde a échoué. Vérifiez votre connexion et réessayez.");
    } finally {
      setBackupBusy(false);
    }
  };

  const importBackup = async (file: File) => {
    setImportError(null);
    setImportResult(null);
    setImportBusy(true);
    try {
      const { recipes, invalidCount } = parseBackup(await file.text());
      const existing = await api.get<RecipeView[]>("/recipes");
      const { toImport, duplicateCount } = planImport(
        recipes,
        existing.map((r) => r.title),
      );

      let imported = 0;
      let failed = 0;
      for (const recipe of toImport) {
        try {
          await api.post<RecipeView>("/recipes", recipe);
          imported++;
        } catch {
          failed++;
        }
      }
      if (imported > 0) await queryClient.invalidateQueries({ queryKey: ["recipes"] });
      setImportResult(importSummary({ imported, duplicateCount, invalidCount, failed }));
    } catch (err) {
      setImportError(
        err instanceof BackupImportError
          ? err.message
          : "L'import a échoué. Vérifiez votre connexion et réessayez.",
      );
    } finally {
      setImportBusy(false);
    }
  };

  const onImportFile = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    // Réinitialise l'input pour permettre de réimporter le même fichier.
    event.target.value = "";
    if (file) await importBackup(file);
  };

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
        <h2>Semaine</h2>
        <label className="field">
          <span className="field-label">Premier jour de la semaine</span>
          <select
            className="input"
            value={householdSettings.data?.week_start_day ?? 1}
            disabled={householdSettings.isLoading || setWeekStartDay.isPending}
            onChange={(e) => setWeekStartDay.mutate(Number(e.target.value))}
          >
            {WEEK_DAY_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <p className="muted" style={{ marginTop: "0.6rem", fontSize: "0.85rem" }}>
          Découpe l'onglet Semaine et la liste de courses. Réglage partagé par
          tout le foyer.
        </p>
        {setWeekStartDay.isError && (
          <p className="settings-error" role="alert">
            La modification a échoué. Réessayez.
          </p>
        )}
      </div>

      <StoresSection />

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
        <h2>Sauvegarde</h2>
        <button
          className="btn btn--primary btn--block"
          type="button"
          onClick={backup}
          disabled={backupBusy}
        >
          {backupBusy ? "Préparation…" : "Sauvegarder mes recettes (JSON)"}
        </button>
        <p className="muted" style={{ marginTop: "0.6rem", fontSize: "0.85rem" }}>
          Télécharge toutes vos recettes sur cet appareil. Les photos ne sont
          conservées que si elles ont un lien web ; celles hébergées sur le
          serveur ne sont pas incluses.
        </p>
        {backupError && (
          <p className="settings-error" role="alert">
            {backupError}
          </p>
        )}

        <button
          className="btn btn--block"
          type="button"
          onClick={() => fileInput.current?.click()}
          disabled={importBusy}
          style={{ marginTop: "1rem" }}
        >
          {importBusy ? "Import en cours…" : "Importer des recettes (JSON)"}
        </button>
        <input
          ref={fileInput}
          type="file"
          accept="application/json,.json"
          hidden
          onChange={onImportFile}
        />
        <p className="muted" style={{ marginTop: "0.6rem", fontSize: "0.85rem" }}>
          Ajoute les recettes d'un fichier de sauvegarde. Les recettes déjà
          présentes (même titre) sont ignorées.
        </p>
        {importResult && (
          <p className="settings-note" role="status">
            {importResult}
          </p>
        )}
        {importError && (
          <p className="settings-error" role="alert">
            {importError}
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

/**
 * Réglage des **magasins** : un magasin, c'est un nom et l'ordre dans lequel on
 * en traverse les rayons. L'onglet Courses s'en sert pour trier la liste et
 * n'imposer qu'un seul aller.
 *
 * Tous les rayons du catalogue y figurent, même ceux qu'on n'achète jamais :
 * un rayon sans article ne produit aucune section dans la liste, alors qu'un
 * rayon oublié laisserait ses articles sans place.
 */
function StoresSection() {
  const stores = useStores();
  const createStore = useCreateStore();
  const [name, setName] = useState("");

  function create(event: FormEvent) {
    event.preventDefault();
    if (!name.trim()) return;
    createStore.mutate(name.trim(), { onSuccess: () => setName("") });
  }

  return (
    <div className="card settings-section">
      <h2>Magasins</h2>
      <p className="muted" style={{ marginBottom: "0.75rem", fontSize: "0.85rem" }}>
        Rangez les rayons dans l'ordre où vous les traversez : l'onglet Courses
        peut alors trier la liste magasin par magasin.
      </p>

      {stores.data && stores.data.length > 0 && (
        <ul className="store-list">
          {stores.data.map((store) => (
            <StoreRow key={store.id} store={store} />
          ))}
        </ul>
      )}

      <form className="store-add" onSubmit={create}>
        <input
          className="input"
          placeholder="Nom du magasin…"
          aria-label="Nom du magasin"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <button
          className="btn btn--primary"
          type="submit"
          disabled={createStore.isPending || !name.trim()}
        >
          Ajouter
        </button>
      </form>
      {createStore.isError && (
        <p className="settings-error" role="alert">
          La création a échoué. Réessayez.
        </p>
      )}
    </div>
  );
}

/** Un magasin : renommage, suppression, et son parcours de rayons dépliable. */
function StoreRow({ store }: { store: Store }) {
  const updateStore = useUpdateStore();
  const deleteStore = useDeleteStore();
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(store.name);

  function rename(event: FormEvent) {
    event.preventDefault();
    if (!name.trim()) return;
    updateStore.mutate({ id: store.id, name: name.trim() });
    setRenaming(false);
  }

  function remove() {
    if (!window.confirm(`Supprimer « ${store.name} » ? L'ordre de ses rayons sera perdu.`)) {
      return;
    }
    deleteStore.mutate(store.id);
  }

  if (renaming) {
    return (
      <li className="store-list__item">
        <form className="store-rename" onSubmit={rename}>
          <input
            className="input"
            aria-label={`Nom de ${store.name}`}
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoFocus
          />
          <button className="btn btn--primary" type="submit">
            OK
          </button>
          <button
            className="btn"
            type="button"
            onClick={() => {
              setName(store.name);
              setRenaming(false);
            }}
          >
            Annuler
          </button>
        </form>
      </li>
    );
  }

  return (
    <li className="store-list__item">
      <div className="store-list__head">
        <button
          className="store-list__name"
          type="button"
          onClick={() => setOpen((current) => !current)}
          aria-expanded={open}
        >
          {open ? "▾" : "▸"} {store.name}
        </button>
        <button className="btn" type="button" onClick={() => setRenaming(true)}>
          Renommer
        </button>
        <button className="btn btn--danger-ghost" type="button" onClick={remove}>
          Supprimer
        </button>
      </div>
      {open && <AisleOrder store={store} />}
    </li>
  );
}

/**
 * L'ordre de visite des rayons d'un magasin, réordonnable à la poignée (ou aux
 * flèches ↑/↓, pour qui ne fait pas glisser du doigt sur un écran).
 *
 * Chaque geste enregistre l'ordre complet : le serveur le renvoie normalisé,
 * ce qui garde l'affichage d'aplomb même si le catalogue a bougé entre deux
 * versions.
 */
function AisleOrder({ store }: { store: Store }) {
  const aisles = useAisles();
  const updateStore = useUpdateStore();

  // Les rayons du magasin, dans leur ordre, portant leur libellé de catalogue.
  const rows = useMemo(() => {
    const labels = new Map((aisles.data ?? []).map((aisle) => [aisle.slug, aisle.label]));
    return store.aisles.map((slug) => ({ id: slug, label: labels.get(slug) ?? slug }));
  }, [store.aisles, aisles.data]);

  const { order, activeId, listRef, handleProps } = useDragOrder(rows, (ordered) =>
    updateStore.mutate({ id: store.id, aisles: ordered.map((row) => row.id) }),
  );

  return (
    <>
      <p className="muted store-aisles__hint">
        Ordre de visite — glissez ≡ (ou ↑/↓ au clavier) pour réordonner.
      </p>
      <ul className="store-aisles" ref={listRef}>
        {order.map((row, index) => (
          <li className="store-aisles__item" key={row.id} data-dragging={activeId === row.id}>
            <button
              className="shopping-row__handle"
              type="button"
              aria-label={`Déplacer ${row.label}`}
              {...handleProps(row.id)}
            >
              ≡
            </button>
            <span className="store-aisles__rank">{index + 1}</span>
            <span aria-hidden="true">{aisleEmoji(row.id)}</span>
            <span className="store-aisles__label">{row.label}</span>
          </li>
        ))}
      </ul>
      {updateStore.isError && (
        <p className="settings-error" role="alert">
          L'ordre n'a pas pu être enregistré. Réessayez.
        </p>
      )}
    </>
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
