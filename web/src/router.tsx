import {
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from "@tanstack/react-router";
import { requireSession } from "./api/session";
import { AppShell } from "./components/AppShell";
import { LoginScreen } from "./routes/login";
import { RecipesScreen } from "./routes/recipes";
import { WeekScreen } from "./routes/week";
import { ShoppingScreen } from "./routes/shopping";
import { SettingsScreen } from "./routes/settings";

const rootRoute = createRootRoute({ component: AppShell });

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => {
    throw redirect({ to: "/recipes" });
  },
});

const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/login",
  component: LoginScreen,
  // `?redirect=` mémorise l'écran demandé avant la redirection, pour y revenir
  // après connexion. Restreint aux chemins internes : une valeur absolue
  // (`//evil.tld`) permettrait une redirection ouverte via un lien forgé.
  validateSearch: (search: Record<string, unknown>) => {
    const target = search.redirect;
    return typeof target === "string" && target.startsWith("/") && !target.startsWith("//")
      ? { redirect: target }
      : {};
  },
});

/**
 * Route de mise en page sans chemin : tout ce qui vit dessous exige une session.
 * La garde est posée ici une fois, plutôt que répétée sur chaque écran — un
 * écran ajouté plus tard est protégé par défaut.
 */
const authenticatedRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "authenticated",
  beforeLoad: ({ location }) => requireSession(location.pathname),
});

const recipesRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/recipes",
  component: RecipesScreen,
});

const weekRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/week",
  component: WeekScreen,
});

const shoppingRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/shopping",
  component: ShoppingScreen,
});

const settingsRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/settings",
  component: SettingsScreen,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  loginRoute,
  authenticatedRoute.addChildren([
    recipesRoute,
    weekRoute,
    shoppingRoute,
    settingsRoute,
  ]),
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
