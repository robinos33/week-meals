import { Link } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { useTheme } from "../theme/theme-context";

/**
 * Menu « ⋮ » du coin haut-droit : un bouton discret qui déploie au clic les
 * raccourcis les plus courants (bascule de thème, accès Paramètres).
 *
 * Se ferme au clic extérieur, à la touche Échap et après un choix.
 */
export function AppMenu() {
  const { resolved, setPreference } = useTheme();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;

    function onPointerDown(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div className="app-menu" ref={rootRef}>
      <button
        className="app-menu__trigger"
        type="button"
        aria-label="Menu"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true" width="18" height="18">
          <path
            d="M12 6.5v.01M12 12v.01M12 17.5v.01"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.6"
            strokeLinecap="round"
          />
        </svg>
      </button>

      {open && (
        <div className="app-menu__panel" role="menu">
          <button
            className="app-menu__item"
            type="button"
            role="menuitem"
            onClick={() => {
              setPreference(resolved === "dark" ? "light" : "dark");
              setOpen(false);
            }}
          >
            {resolved === "dark" ? "Thème clair" : "Thème sombre"}
          </button>
          <Link
            to="/settings"
            className="app-menu__item"
            role="menuitem"
            onClick={() => setOpen(false)}
          >
            Paramètres
          </Link>
        </div>
      )}
    </div>
  );
}
