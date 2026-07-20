import {
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from "@tanstack/react-router";
import { AppShell } from "./components/AppShell";
import { RecipesScreen } from "./routes/recipes";
import { RecipeDetailScreen } from "./routes/recipe-detail";
import { EditRecipeScreen, NewRecipeScreen } from "./routes/recipe-form";
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

// La garde d'authentification est portée en amont par `AuthGate` (cf.
// ADR-0006), qui enveloppe le routeur dans `main.tsx` : les écrans ne sont
// montés qu'une fois l'identité résolue (session, ou foyer de démo en mode
// public). Les routes restent donc « nues » ici.
const recipesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/recipes",
  component: RecipesScreen,
});

// Route statique avant la route dynamique : `/recipes/new` doit primer sur
// `/recipes/$recipeId` (TanStack privilégie le statique, l'ordre reste explicite).
const recipeNewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/recipes/new",
  component: NewRecipeScreen,
});

const recipeDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/recipes/$recipeId",
  component: RecipeDetailScreen,
});

const recipeEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/recipes/$recipeId/edit",
  component: EditRecipeScreen,
});

const weekRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/week",
  component: WeekScreen,
});

const shoppingRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/shopping",
  component: ShoppingScreen,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsScreen,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  recipesRoute,
  recipeNewRoute,
  recipeDetailRoute,
  recipeEditRoute,
  weekRoute,
  shoppingRoute,
  settingsRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
