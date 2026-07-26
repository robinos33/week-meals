import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { PersistQueryClientProvider } from "@tanstack/react-query-persist-client";
import "@fontsource-variable/fraunces";
import "./theme/tokens.css";
import "./theme/global.css";
import { ThemeProvider } from "./theme/ThemeProvider";
import { queryClient } from "./query";
import { dexiePersister } from "./lib/query-persister";
import { configureShoppingListOffline } from "./api/shopping-list";
import { router } from "./router";
import { AuthGate } from "./components/AuthGate";

// Enregistre les mutations rejouables AVANT que le cache persisté ne soit
// restauré : une mutation mise en file hors-ligne (puis l'app rechargée) doit
// retrouver sa fonction pour être rejouée au retour du réseau (#21).
configureShoppingListOffline(queryClient);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <PersistQueryClientProvider
        client={queryClient}
        persistOptions={{
          persister: dexiePersister,
          // La liste doit survivre plusieurs jours sans réseau (courses non
          // faites le week-end) ; au-delà, on repart d'une lecture serveur.
          maxAge: 1000 * 60 * 60 * 24 * 7,
          // Change de valeur = cache jeté : à bumper si la forme de `ShoppingItem`
          // évolue, pour ne pas réhydrater un ancien schéma.
          buster: "v1",
          dehydrateOptions: {
            // On ne persiste QUE la liste de courses (offline ciblé, ADR-0004) :
            // recettes et calendrier restent online-only. Les mutations en file
            // (en pause) le sont par défaut — c'est notre file hors-ligne.
            shouldDehydrateQuery: (query) => query.queryKey[0] === "shopping-list",
          },
        }}
        // Après restauration, on relance les mutations restées en file (le retour
        // du réseau en session est déjà géré seul par React Query).
        onSuccess={() => {
          void queryClient.resumePausedMutations();
        }}
      >
        <AuthGate>
          <RouterProvider router={router} />
        </AuthGate>
      </PersistQueryClientProvider>
    </ThemeProvider>
  </StrictMode>,
);
