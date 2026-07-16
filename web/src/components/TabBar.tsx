import { Link, useRouterState } from "@tanstack/react-router";
import type { ReactNode } from "react";

interface Tab {
  to: string;
  label: string;
  icon: ReactNode;
}

/** Icônes en trait, héritant de `currentColor` (pas de dépendance d'icônes). */
const icons = {
  recipes: (
    <svg viewBox="0 0 24 24" aria-hidden="true" width="24" height="24">
      <path
        d="M4 5h16v14H4zM8 5v14M4 9h4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
  week: (
    <svg viewBox="0 0 24 24" aria-hidden="true" width="24" height="24">
      <path
        d="M4 6h16v14H4zM4 10h16M8 3v4M16 3v4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
  shopping: (
    <svg viewBox="0 0 24 24" aria-hidden="true" width="24" height="24">
      <path
        d="M6 8h12l-1 11H7L6 8zM9 8a3 3 0 0 1 6 0"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
};

const tabs: Tab[] = [
  { to: "/recipes", label: "Recettes", icon: icons.recipes },
  { to: "/week", label: "Semaine", icon: icons.week },
  { to: "/shopping", label: "Courses", icon: icons.shopping },
];

/** Barre d'onglets basse, pleine largeur, au pouce (cf. brief « Cantine »). */
export function TabBar() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  return (
    <nav className="tabbar" aria-label="Navigation principale">
      {tabs.map((tab) => {
        const active = pathname.startsWith(tab.to);
        return (
          <Link
            key={tab.to}
            to={tab.to}
            className="tabbar__item"
            aria-current={active ? "page" : undefined}
            data-active={active}
          >
            <span className="tabbar__icon">{tab.icon}</span>
            <span className="tabbar__label">{tab.label}</span>
          </Link>
        );
      })}
    </nav>
  );
}
