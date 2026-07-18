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

// Mode public (preview) : plus de mire ni de garde de session. Les écrans
// vivent directement sous la racine et l'API scope au foyer de démo (cf.
// AUTH_DISABLED). Réactiver l'auth = reposer la garde `requireSession` ici.
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
