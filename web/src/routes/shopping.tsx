import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import {
  adjustQuantity,
  formatQuantity,
  UNITS,
  UNIT_LABELS,
  useAddItem,
  useClearChecked,
  useDeleteItem,
  sameCombo,
  useIngredientDictionary,
  useReorderItems,
  useShoppingList,
  useShoppingSync,
  useUpdateItem,
  type DictionaryEntry,
  type ShoppingItem,
  type SyncState,
  type Unit,
} from "../api/shopping-list";
import { foodEmoji } from "../lib/food-emoji";
import { canonicalKey, rankMatches } from "../lib/ingredient-match";
import { parseAmount } from "../lib/quantity";
import "./screens.css";

/** Libellé (title + a11y) du point de synchro selon l'état. */
const SYNC_LABELS: Record<SyncState, string> = {
  offline: "Hors-ligne — les changements seront synchronisés au retour du réseau",
  syncing: "Synchronisation…",
  live: "Synchronisé en direct",
};

/**
 * Onglet Courses (UX inspirée de Google Keep) : ajout rapide en haut, articles
 * cochables, les cochés glissant dans une section repliable en bas.
 */
export function ShoppingScreen() {
  const query = useShoppingList();
  const { state: syncState } = useShoppingSync();
  const addItem = useAddItem();
  const updateItem = useUpdateItem();
  const clearChecked = useClearChecked();
  const [showChecked, setShowChecked] = useState(true);

  const items = useMemo(() => query.data ?? [], [query.data]);
  const pending = items.filter((item) => !item.checked);
  const done = items.filter((item) => item.checked);

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">
          Courses
          <span
            className="sync-dot"
            data-state={syncState}
            title={SYNC_LABELS[syncState]}
            aria-label={SYNC_LABELS[syncState]}
            role="status"
          />
        </h1>
        {done.length > 0 && (
          <button
            className="btn btn--danger-ghost"
            type="button"
            onClick={() => clearChecked.mutate()}
            disabled={clearChecked.isPending}
          >
            Vider les cochés
          </button>
        )}
      </header>

      <QuickAdd
        items={items}
        onAdd={(item) => addItem.mutate(item)}
        onRestore={(item) => updateItem.mutate({ id: item.id, checked: false })}
        pending={addItem.isPending}
      />

      {query.isLoading ? (
        <p className="muted">Chargement…</p>
      ) : query.isError ? (
        <div className="empty-state">
          <div className="empty-state__emoji">🌩️</div>
          <h2>Liste indisponible</h2>
          <button className="btn" type="button" onClick={() => query.refetch()}>
            Réessayer
          </button>
        </div>
      ) : items.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state__emoji">🛒</div>
          <h2>Liste vide</h2>
          <p>Générez-la depuis la semaine, ou ajoutez un article ci-dessus.</p>
        </div>
      ) : (
        <>
          <PendingList items={pending} tailIds={done.map((item) => item.id)} />

          {done.length > 0 && (
            <div className="checked-section">
              <button
                className="checked-section__toggle"
                type="button"
                onClick={() => setShowChecked((open) => !open)}
                aria-expanded={showChecked}
              >
                {showChecked ? "▾" : "▸"} {done.length} article{done.length > 1 ? "s" : ""} coché
                {done.length > 1 ? "s" : ""}
              </button>
              {showChecked && (
                <ul className="shopping-list">
                  {done.map((item) => (
                    <ShoppingRow key={item.id} item={item} />
                  ))}
                </ul>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}

/**
 * Liste des articles à cocher, réordonnable par glisser-déposer via la poignée.
 *
 * Le drag est géré aux Pointer Events (tactile + souris, sans dépendance) :
 * l'ordre local suit le doigt en échangeant avec la ligne voisine dès que son
 * milieu est franchi, et n'est persisté qu'au relâchement. `tailIds` (les
 * lignes cochées) est réémis à la suite pour garder un ordre global cohérent.
 */
function PendingList({ items, tailIds }: { items: ShoppingItem[]; tailIds: string[] }) {
  const reorder = useReorderItems();
  const [order, setOrder] = useState(items);
  // Ordre courant en ref : lu de façon synchrone pendant le glissement (l'état
  // React n'est pas encore à jour au moment du `pointerup`).
  const orderRef = useRef(items);
  const listRef = useRef<HTMLUListElement>(null);
  const draggingId = useRef<string | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);

  // Resynchronise sur le serveur, sauf pendant un glissement en cours.
  useEffect(() => {
    if (!draggingId.current) {
      orderRef.current = items;
      setOrder(items);
    }
  }, [items]);

  /** Place la ligne glissée au slot correspondant à l'ordonnée `y` du doigt. */
  function reposition(y: number) {
    const id = draggingId.current;
    if (!id) return;
    const current = orderRef.current;
    const from = current.findIndex((item) => item.id === id);
    const rows = Array.from(listRef.current?.children ?? []) as HTMLElement[];

    // Slot cible = première ligne dont le milieu passe sous le doigt.
    let target = rows.findIndex((row) => {
      const rect = row.getBoundingClientRect();
      return y < rect.top + rect.height / 2;
    });
    if (target === -1) target = rows.length - 1;
    else if (target > from) target -= 1; // on retire d'abord la ligne glissée

    if (target !== from && target >= 0) {
      const next = [...current];
      const [moved] = next.splice(from, 1);
      next.splice(target, 0, moved);
      orderRef.current = next;
      setOrder(next);
    }
  }

  function onPointerDown(event: ReactPointerEvent, id: string) {
    event.preventDefault();
    draggingId.current = id;
    setActiveId(id);
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: ReactPointerEvent) {
    reposition(event.clientY);
  }

  function onPointerUp(event: ReactPointerEvent) {
    if (!draggingId.current) return;
    reposition(event.clientY); // capte la position de relâchement
    draggingId.current = null;
    setActiveId(null);
    reorder.mutate([...orderRef.current.map((item) => item.id), ...tailIds]);
  }

  return (
    <ul className="shopping-list" ref={listRef}>
      {order.map((item) => (
        <ShoppingRow
          key={item.id}
          item={item}
          dragging={activeId === item.id}
          handle={
            <button
              className="shopping-row__handle"
              type="button"
              aria-label={`Déplacer ${item.name}`}
              onPointerDown={(event) => onPointerDown(event, item.id)}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={onPointerUp}
            >
              ≡
            </button>
          }
        />
      ))}
    </ul>
  );
}

/**
 * Sélecteur de quantité tactile : boutons − / + de part et d'autre d'un champ
 * numérique, avec un pas adapté à l'unité (1 pièce, 50 g/mL, 0,5 kg/L).
 */
function QuantityStepper({
  value,
  unit,
  onChange,
  ariaLabel = "Quantité",
}: {
  value: string;
  unit: Unit;
  onChange: (value: string) => void;
  ariaLabel?: string;
}) {
  function step(direction: 1 | -1) {
    const current = parseAmount(value) ?? 0;
    onChange(String(adjustQuantity(current, unit, direction)));
  }

  return (
    <div className="stepper">
      <button
        type="button"
        className="stepper__btn"
        aria-label="Diminuer la quantité"
        onClick={() => step(-1)}
      >
        −
      </button>
      <input
        type="text"
        className="stepper__value"
        aria-label={ariaLabel}
        inputMode="decimal"
        placeholder="1/2"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      <button
        type="button"
        className="stepper__btn"
        aria-label="Augmenter la quantité"
        onClick={() => step(1)}
      >
        +
      </button>
    </div>
  );
}

/** Libellé lisible d'un rayon du dictionnaire. */
const CATEGORY_LABELS: Record<string, string> = {
  legumes: "Légumes",
  fruits: "Fruits",
  cremerie: "Crèmerie",
  boucherie: "Boucherie",
  poissonnerie: "Poissonnerie",
  epicerie: "Épicerie",
  boulangerie: "Boulangerie",
  surgeles: "Surgelés",
  boissons: "Boissons",
  entretien: "Entretien",
};

/** Nombre de suggestions affichées : au-delà, la liste masque l'écran. */
const MAX_SUGGESTIONS = 6;

/** Une proposition d'auto-complétion, venue de la liste ou du dictionnaire. */
interface Suggestion {
  /** Libellé affiché et inséré dans le champ. */
  name: string;
  /** Synonymes reconnus à la frappe (dictionnaire). */
  aliases?: string[];
  /** Unité présélectionnée à la sélection. */
  unit: Unit;
  /** Rayon, pour les entrées du dictionnaire. */
  category?: string;
  /** Ligne déjà dans la liste : la quantité s'y cumulera. */
  existing?: ShoppingItem;
}

/**
 * Candidats à l'auto-complétion : la liste courante d'abord, puis le
 * dictionnaire dont on retire ce qui y figure déjà (sinon « courgette »
 * apparaîtrait deux fois).
 *
 * `excludeId` écarte la ligne en cours d'édition : se proposer soi-même n'aide
 * pas, et la mention « sera cumulé » y serait fausse.
 */
function useSuggestionCandidates(excludeId?: string): Suggestion[] {
  const list = useShoppingList();
  const dictionary = useIngredientDictionary();

  return useMemo<Suggestion[]>(() => {
    const fromList: Suggestion[] = (list.data ?? [])
      .filter((item) => item.id !== excludeId)
      .map((item) => ({ name: item.name, unit: item.unit, existing: item }));
    const listed = new Set(fromList.map((suggestion) => canonicalKey(suggestion.name)));
    const fromDictionary = (dictionary.data ?? [])
      .filter((entry: DictionaryEntry) => !listed.has(canonicalKey(entry.name)))
      .map((entry: DictionaryEntry) => ({
        name: entry.name,
        aliases: entry.aliases,
        unit: entry.unit,
        category: entry.category,
      }));
    return [...fromList, ...fromDictionary];
  }, [list.data, dictionary.data, excludeId]);
}

/**
 * Champ de nom d'ingrédient auto-complété (combobox), partagé par l'ajout
 * rapide et l'édition d'une ligne.
 *
 * Deux sources de suggestions : les articles **déjà dans la liste** — c'est là
 * que la quantité se cumulera, on les fait donc passer devant — puis le
 * dictionnaire d'ingrédients de l'API. Le rapprochement tolère casse, accents,
 * pluriels, qualificatifs et fautes de frappe (cf. `lib/ingredient-match`),
 * avec les mêmes règles que le serveur : la ligne proposée est bien celle sur
 * laquelle la saisie atterrira.
 *
 * La liste ne s'ouvre qu'à la frappe (ou sur ↓), jamais à la simple prise de
 * focus : à l'édition, le champ est déjà rempli et se proposerait des
 * suggestions avant qu'on ait rien demandé.
 */
function IngredientInput({
  value,
  onChange,
  onPick,
  excludeId,
  className = "input",
  placeholder,
  ariaLabel,
  autoFocus = false,
}: {
  value: string;
  onChange: (name: string) => void;
  /** Suggestion retenue : nom canonique **et** unité d'achat. */
  onPick: (suggestion: Suggestion) => void;
  excludeId?: string;
  className?: string;
  placeholder?: string;
  ariaLabel: string;
  autoFocus?: boolean;
}) {
  const candidates = useSuggestionCandidates(excludeId);
  const inputRef = useRef<HTMLInputElement>(null);
  // Identifiants ARIA propres à l'instance : deux combobox peuvent coexister
  // (ajout rapide + ligne en édition).
  const listId = useId();
  // Suggestion mise en avant au clavier (-1 = aucune, la saisie fait foi).
  const [highlight, setHighlight] = useState(-1);
  const [open, setOpen] = useState(false);

  const suggestions = useMemo(
    // Le bonus fait remonter les articles de la liste à pertinence comparable.
    () =>
      rankMatches(value, candidates, (candidate) => (candidate.existing ? 0.3 : 0)).slice(
        0,
        MAX_SUGGESTIONS,
      ),
    [value, candidates],
  );

  const showSuggestions = open && suggestions.length > 0;

  function close() {
    setOpen(false);
    setHighlight(-1);
  }

  /** Reprend une suggestion : nom et unité, prêt à valider. */
  function choose(suggestion: Suggestion) {
    onPick(suggestion);
    close();
    inputRef.current?.focus();
  }

  /** Navigation clavier dans la liste (le tactile passe par le tap). */
  function onKeyDown(event: ReactKeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      close();
      return;
    }
    if (!showSuggestions) {
      // ↓ rouvre une liste refermée sans avoir à retaper.
      if (event.key === "ArrowDown" && suggestions.length > 0) setOpen(true);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlight((current) => (current + 1) % suggestions.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlight((current) => (current <= 0 ? suggestions.length - 1 : current - 1));
    } else if (event.key === "Enter" && highlight >= 0) {
      // Entrée valide la suggestion mise en avant plutôt que le formulaire.
      event.preventDefault();
      choose(suggestions[highlight]);
    }
  }

  return (
    <div className="combo">
      <input
        ref={inputRef}
        className={className}
        placeholder={placeholder}
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
          setHighlight(-1);
          setOpen(true);
        }}
        onKeyDown={onKeyDown}
        onBlur={close}
        role="combobox"
        aria-expanded={showSuggestions}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={highlight >= 0 ? `${listId}-${highlight}` : undefined}
        autoComplete="off"
        autoFocus={autoFocus}
      />
      {showSuggestions && (
        <ul className="suggestions" id={listId} role="listbox">
          {suggestions.map((suggestion, index) => (
            <li key={`${suggestion.existing?.id ?? "dict"}-${suggestion.name}`}>
              <button
                type="button"
                id={`${listId}-${index}`}
                className="suggestions__option"
                role="option"
                aria-selected={index === highlight}
                data-highlighted={index === highlight}
                // Le clic doit gagner contre le `blur` du champ, qui referme
                // la liste : on choisit dès l'appui, sans laisser le focus.
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => choose(suggestion)}
              >
                <span className="suggestions__emoji" aria-hidden="true">
                  {foodEmoji(suggestion.name) ?? "•"}
                </span>
                <span className="suggestions__name">{suggestion.name}</span>
                <span className="suggestions__hint">
                  {suggestion.existing
                    ? `déjà ${formatQuantity(suggestion.existing)}${
                        suggestion.existing.checked ? " (coché)" : " — sera cumulé"
                      }`
                    : (CATEGORY_LABELS[suggestion.category ?? ""] ?? "")}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * Champ d'ajout rapide, toujours accessible en haut de l'écran.
 *
 * Le nom est auto-complété (cf. [`IngredientInput`]) sur la liste courante et
 * le dictionnaire : l'article déjà présent se retrouve en une frappe, et c'est
 * sur lui que la quantité se cumulera.
 *
 * Un ingrédient déjà présent (non coché) n'est jamais dupliqué : l'API cumule
 * la quantité sur la ligne existante (« courgette 3 » + « courgette 2 » →
 * « courgette 5 »). S'il n'existe plus que **coché** (déjà acheté), on propose
 * de le remettre dans la liste plutôt que d'ouvrir une nouvelle ligne.
 */
function QuickAdd({
  items,
  onAdd,
  onRestore,
  pending,
}: {
  items: ShoppingItem[];
  onAdd: (item: { name: string; amount: number; unit: Unit }) => void;
  onRestore: (item: ShoppingItem) => void;
  pending: boolean;
}) {
  const [name, setName] = useState("");
  const [amount, setAmount] = useState("1");
  const [unit, setUnit] = useState<Unit>("piece");

  // Candidat courant (null tant que la saisie n'est pas valide). La quantité
  // accepte les fractions (« 1/2 », « 1 1/2 », « 0,5 »).
  const parsed = parseAmount(amount);
  const candidate =
    name.trim() && parsed != null
      ? { name: name.trim(), amount: parsed, unit }
      : null;

  // Même combo mais déjà **coché** : plutôt que d'ouvrir une nouvelle ligne, on
  // propose de le remettre (décoché) dans la liste. Un combo non coché, lui,
  // n'est pas bloqué : l'API cumule la quantité côté serveur.
  const restorable = candidate
    ? items.find((item) => item.checked && sameCombo(item, candidate))
    : undefined;

  function reset() {
    setName("");
    setAmount("1");
  }

  function restore(item: ShoppingItem) {
    onRestore(item);
    reset();
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    if (!candidate) return;
    if (restorable) {
      restore(restorable); // même combo coché : on le remet plutôt que dupliquer
      return;
    }
    onAdd(candidate); // l'API cumule si l'ingrédient est déjà présent
    reset();
  }

  return (
    <form className="quick-add" onSubmit={submit}>
      <div className="quick-add__field">
        <IngredientInput
          className="input quick-add__name"
          placeholder="Ajouter un article…"
          ariaLabel="Nom de l'article"
          value={name}
          onChange={setName}
          onPick={(suggestion) => {
            setName(suggestion.name);
            setUnit(suggestion.unit);
          }}
        />
      </div>
      <QuantityStepper value={amount} unit={unit} onChange={setAmount} />
      <select
        className="input input--unit"
        aria-label="Unité"
        value={unit}
        onChange={(e) => setUnit(e.target.value as Unit)}
      >
        {UNITS.map((u) => (
          <option key={u} value={u}>
            {UNIT_LABELS[u]}
          </option>
        ))}
      </select>
      <button
        className="btn btn--primary"
        type="submit"
        aria-label="Ajouter à la liste"
        disabled={pending}
      >
        +
      </button>

      {restorable && (
        <button
          type="button"
          className="quick-add__note quick-add__restore"
          onClick={() => restore(restorable)}
        >
          « {formatQuantity(restorable)} {restorable.name} » est coché — le remettre dans la
          liste ?
        </button>
      )}
    </form>
  );
}

/** Une ligne : poignée optionnelle, case à cocher, texte (tap = édition), suppression. */
function ShoppingRow({
  item,
  handle,
  dragging = false,
}: {
  item: ShoppingItem;
  handle?: ReactNode;
  dragging?: boolean;
}) {
  const updateItem = useUpdateItem();
  const deleteItem = useDeleteItem();
  const [editing, setEditing] = useState(false);
  const emoji = foodEmoji(item.name);

  if (editing) {
    return (
      <li className="shopping-row shopping-row--editing">
        <InlineEdit
          item={item}
          onCancel={() => setEditing(false)}
          onSave={(patch) => {
            updateItem.mutate({ id: item.id, ...patch });
            setEditing(false);
          }}
        />
      </li>
    );
  }

  return (
    <li className="shopping-row" data-checked={item.checked} data-dragging={dragging}>
      {handle}
      <input
        type="checkbox"
        className="shopping-row__check"
        checked={item.checked}
        aria-label={`Cocher ${item.name}`}
        onChange={(e) => updateItem.mutate({ id: item.id, checked: e.target.checked })}
      />
      <button
        className="shopping-row__text"
        type="button"
        onClick={() => setEditing(true)}
        aria-label={`Modifier ${item.name}`}
      >
        <span className="shopping-row__emoji" aria-hidden="true">
          {emoji ?? "•"}
        </span>
        <span className="shopping-row__qty">{formatQuantity(item)}</span>
        <span className="shopping-row__name">{item.name}</span>
      </button>
      <button
        className="shopping-row__remove"
        type="button"
        aria-label={`Supprimer ${item.name}`}
        onClick={() => deleteItem.mutate(item.id)}
      >
        ×
      </button>
    </li>
  );
}

/**
 * Édition inline d'une ligne : nom, quantité, unité.
 *
 * Le nom est auto-complété comme à l'ajout — corriger « courgetes » en
 * « courgette » ne devrait pas demander de le réécrire —, la ligne en cours
 * d'édition étant écartée de ses propres suggestions.
 */
function InlineEdit({
  item,
  onSave,
  onCancel,
}: {
  item: ShoppingItem;
  onSave: (patch: { name: string; amount: number; unit: Unit }) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(item.name);
  const [amount, setAmount] = useState(String(item.amount));
  const [unit, setUnit] = useState<Unit>(item.unit);

  function save(event: FormEvent) {
    event.preventDefault();
    const parsed = parseAmount(amount);
    if (!name.trim() || parsed == null) return;
    onSave({ name: name.trim(), amount: parsed, unit });
  }

  return (
    <form className="inline-edit" onSubmit={save}>
      <IngredientInput
        className="input inline-edit__name"
        ariaLabel="Nom"
        value={name}
        onChange={setName}
        onPick={(suggestion) => {
          setName(suggestion.name);
          setUnit(suggestion.unit);
        }}
        excludeId={item.id}
        autoFocus
      />
      <QuantityStepper value={amount} unit={unit} onChange={setAmount} />
      <select
        className="input input--unit"
        aria-label="Unité"
        value={unit}
        onChange={(e) => setUnit(e.target.value as Unit)}
      >
        {UNITS.map((u) => (
          <option key={u} value={u}>
            {UNIT_LABELS[u]}
          </option>
        ))}
      </select>
      <button className="btn btn--primary" type="submit">
        OK
      </button>
      <button className="btn" type="button" onClick={onCancel}>
        Annuler
      </button>
    </form>
  );
}
