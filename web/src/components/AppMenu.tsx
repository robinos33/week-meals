import { Link } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { useTheme } from "../theme/theme-context";
import { THEME_ICONS } from "./theme-icons";

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
  // Thème proposé par le raccourci : l'inverse de celui affiché.
  const target = resolved === "dark" ? "light" : "dark";

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
              setPreference(target);
              setOpen(false);
            }}
          >
            {/* Le picto annonce le thème visé, comme le libellé. */}
            <span className="app-menu__icon">{THEME_ICONS[target]}</span>
            {target === "dark" ? "Thème sombre" : "Thème clair"}
          </button>
          <Link
            to="/settings"
            className="app-menu__item"
            role="menuitem"
            onClick={() => setOpen(false)}
          >
            <span className="app-menu__icon">
              {/* Curseurs de réglage. */}
              <svg
                viewBox="0 0 24 24"
                aria-hidden="true"
                width="18"
                height="18"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                strokeLinecap="round"
              >
                <path d="M4 7h9M19 7h1M4 12h5M15 12h5M4 17h9M19 17h1" />
                <circle cx="16" cy="7" r="2.2" />
                <circle cx="12" cy="12" r="2.2" />
                <circle cx="16" cy="17" r="2.2" />
              </svg>
            </span>
            Paramètres
          </Link>
        </div>
      )}
    </div>
  );
}
