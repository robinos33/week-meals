/**
 * Réordonnancement par glisser-déposer, aux Pointer Events (tactile + souris,
 * sans dépendance).
 *
 * Extrait de la liste de courses, qui n'est plus seule à s'en servir : l'ordre
 * de visite des rayons d'un magasin se règle avec la même poignée. Le principe
 * est celui de Keep : l'ordre local suit le doigt en échangeant avec la ligne
 * voisine dès que son milieu est franchi, et n'est **persisté qu'au
 * relâchement** — on n'écrit pas une fois par pixel parcouru.
 *
 * Les flèches ↑/↓ sur la poignée déplacent la ligne d'un cran : le glisser
 * n'est pas atteignable au clavier, et un ordre qu'on ne peut régler qu'au
 * doigt n'est pas réglable par tout le monde.
 */

import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

/** Ce qu'une poignée de déplacement doit brancher sur son bouton. */
export interface DragHandleProps {
  onPointerDown: (event: PointerEvent) => void;
  onPointerMove: (event: PointerEvent) => void;
  onPointerUp: (event: PointerEvent) => void;
  onPointerCancel: (event: PointerEvent) => void;
  onKeyDown: (event: KeyboardEvent) => void;
}

/** Déplace l'élément du rang `from` au rang `to` (copie). */
export function move<T>(items: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) {
    return items;
  }
  const next = [...items];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

/**
 * Rend `items` réordonnable. `onCommit` reçoit l'ordre complet, une fois le
 * geste terminé.
 *
 * `listRef` doit être posé sur le conteneur dont les enfants sont les lignes,
 * dans le même ordre que `items` : ce sont leurs positions à l'écran qui
 * décident du slot visé.
 */
export function useDragOrder<T extends { id: string }>(
  items: T[],
  onCommit: (ordered: T[]) => void,
) {
  const [order, setOrder] = useState(items);
  // Ordre courant en ref : lu de façon synchrone pendant le glissement (l'état
  // React n'est pas encore à jour au moment du `pointerup`).
  const orderRef = useRef(items);
  const listRef = useRef<HTMLUListElement>(null);
  const draggingId = useRef<string | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);

  // Resynchronise sur la source, sauf pendant un glissement en cours.
  useEffect(() => {
    if (!draggingId.current) {
      orderRef.current = items;
      setOrder(items);
    }
  }, [items]);

  function apply(next: T[]) {
    orderRef.current = next;
    setOrder(next);
  }

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

    apply(move(current, from, target));
  }

  /** Décale d'un cran la ligne `id` et persiste (navigation clavier). */
  function nudge(id: string, direction: 1 | -1) {
    const current = orderRef.current;
    const from = current.findIndex((item) => item.id === id);
    const to = from + direction;
    if (to < 0 || to >= current.length) return;
    const next = move(current, from, to);
    apply(next);
    onCommit(next);
  }

  function handleProps(id: string): DragHandleProps {
    return {
      onPointerDown: (event) => {
        event.preventDefault();
        draggingId.current = id;
        setActiveId(id);
        event.currentTarget.setPointerCapture(event.pointerId);
      },
      onPointerMove: (event) => reposition(event.clientY),
      onPointerUp: (event) => {
        if (!draggingId.current) return;
        reposition(event.clientY); // capte la position de relâchement
        draggingId.current = null;
        setActiveId(null);
        onCommit(orderRef.current);
      },
      onPointerCancel: (event) => {
        if (!draggingId.current) return;
        reposition(event.clientY);
        draggingId.current = null;
        setActiveId(null);
        onCommit(orderRef.current);
      },
      onKeyDown: (event) => {
        if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
        event.preventDefault();
        nudge(id, event.key === "ArrowDown" ? 1 : -1);
      },
    };
  }

  return { order, activeId, listRef, handleProps };
}
