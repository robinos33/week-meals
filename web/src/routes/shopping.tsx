import { useMemo, useState, type FormEvent } from "react";
import {
  formatQuantity,
  UNITS,
  UNIT_LABELS,
  useAddItem,
  useClearChecked,
  useDeleteItem,
  useShoppingList,
  useUpdateItem,
  type ShoppingItem,
  type Unit,
} from "../api/shopping-list";
import "./screens.css";

/**
 * Onglet Courses (UX inspirée de Google Keep) : ajout rapide en haut, articles
 * cochables, les cochés glissant dans une section repliable en bas.
 */
export function ShoppingScreen() {
  const query = useShoppingList();
  const addItem = useAddItem();
  const clearChecked = useClearChecked();
  const [showChecked, setShowChecked] = useState(true);

  const items = useMemo(() => query.data ?? [], [query.data]);
  const pending = items.filter((item) => !item.checked);
  const done = items.filter((item) => item.checked);

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Courses</h1>
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

      <QuickAdd onAdd={(item) => addItem.mutate(item)} pending={addItem.isPending} />

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
          <ul className="shopping-list">
            {pending.map((item) => (
              <ShoppingRow key={item.id} item={item} />
            ))}
          </ul>

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

/** Champ d'ajout rapide, toujours accessible en haut de l'écran. */
function QuickAdd({
  onAdd,
  pending,
}: {
  onAdd: (item: { name: string; amount: number; unit: Unit }) => void;
  pending: boolean;
}) {
  const [name, setName] = useState("");
  const [amount, setAmount] = useState("1");
  const [unit, setUnit] = useState<Unit>("piece");

  function submit(event: FormEvent) {
    event.preventDefault();
    const parsed = Number(amount.replace(",", "."));
    if (!name.trim() || !Number.isFinite(parsed) || parsed <= 0) return;
    onAdd({ name: name.trim(), amount: parsed, unit });
    setName("");
    setAmount("1");
  }

  return (
    <form className="quick-add" onSubmit={submit}>
      <input
        className="input quick-add__name"
        placeholder="Ajouter un article…"
        aria-label="Nom de l'article"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <input
        className="input input--amount"
        aria-label="Quantité"
        inputMode="decimal"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
      />
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
      <button className="btn btn--primary" type="submit" disabled={pending}>
        +
      </button>
    </form>
  );
}

/** Une ligne : case à cocher, texte (tap = édition inline), suppression. */
function ShoppingRow({ item }: { item: ShoppingItem }) {
  const updateItem = useUpdateItem();
  const deleteItem = useDeleteItem();
  const [editing, setEditing] = useState(false);

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
    <li className="shopping-row" data-checked={item.checked}>
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

/** Édition inline d'une ligne : nom, quantité, unité. */
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
    const parsed = Number(amount.replace(",", "."));
    if (!name.trim() || !Number.isFinite(parsed) || parsed <= 0) return;
    onSave({ name: name.trim(), amount: parsed, unit });
  }

  return (
    <form className="inline-edit" onSubmit={save}>
      <input
        className="input"
        aria-label="Nom"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      <input
        className="input input--amount"
        aria-label="Quantité"
        inputMode="decimal"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
      />
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
