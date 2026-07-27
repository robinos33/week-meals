/**
 * L'auto-complétion de la barre de saisie, vue de l'écran Courses.
 *
 * Le cœur du sujet : retrouver un article **déjà dans la liste** pour que
 * l'ajout s'y cumule, même si on l'écrit autrement que la recette qui l'a
 * généré.
 */

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ShoppingScreen } from "./shopping";
import type { DictionaryEntry, ShoppingItem } from "../api/shopping-list";

const ITEMS: ShoppingItem[] = [
  {
    id: "1",
    name: "courgette",
    amount: 3,
    unit: "piece",
    category: "legumes",
    checked: false,
    generated: true,
  },
  {
    id: "2",
    name: "lait",
    amount: 500,
    unit: "ml",
    category: "cremerie",
    checked: false,
    generated: true,
  },
];

const DICTIONARY: DictionaryEntry[] = [
  { name: "courgette", category: "legumes", unit: "piece", countable: false, aliases: [] },
  { name: "œuf", category: "cremerie", unit: "piece", countable: true, aliases: ["oeuf"] },
  { name: "farine", category: "epicerie", unit: "g", countable: false, aliases: ["farine de blé"] },
  { name: "lait de coco", category: "epicerie", unit: "ml", countable: false, aliases: [] },
];

function json(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/shopping-list/dictionary")) return Promise.resolve(json(DICTIONARY));
      if (url.endsWith("/shopping-list")) return Promise.resolve(json(ITEMS));
      return Promise.resolve(json(null));
    }),
  );
});

afterEach(() => vi.unstubAllGlobals());

/** Rend l'écran avec un client neuf (pas de cache partagé entre tests). */
function renderScreen() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <ShoppingScreen />
    </QueryClientProvider>,
  );
}

/** Le champ d'ajout, une fois la liste chargée. */
async function nameField() {
  return screen.findByLabelText("Nom de l'article");
}

/**
 * Les suggestions affichées. La recherche est **cadrée sur la liste** : le
 * sélecteur d'unité est lui aussi fait d'`option`, il polluerait les résultats.
 */
function suggestions() {
  const listbox = screen.queryByRole("listbox");
  return listbox ? within(listbox).queryAllByRole("option") : [];
}

/** Attend l'apparition d'une suggestion dont le libellé matche. */
async function findSuggestion(label: RegExp) {
  const listbox = await screen.findByRole("listbox");
  return within(listbox).findByRole("option", { name: label });
}

describe("auto-complétion de l'ajout rapide", () => {
  it("propose un article déjà dans la liste, avec sa quantité", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(await nameField(), "courg");

    const option = await findSuggestion(/courgette/);
    expect(option).toHaveTextContent("déjà 3 pièce(s) — sera cumulé");
  });

  it("tolère pluriel, accents et qualificatifs", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(await nameField(), "Courgettes jaunes");

    expect(await findSuggestion(/courgette/)).toBeInTheDocument();
  });

  it("retrouve une entrée du dictionnaire par son synonyme", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(await nameField(), "oeuf");

    // Le libellé reste le nom canonique, c'est lui qui partira à l'API.
    expect(await findSuggestion(/œuf/)).toBeInTheDocument();
  });

  it("fait passer la liste courante devant le dictionnaire", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(await nameField(), "lait");

    await findSuggestion(/lait de coco/);
    // Le « lait » de la liste passe devant le « lait de coco » du
    // dictionnaire : c'est sur lui que la quantité se cumulera.
    const labels = suggestions().map((option) => option.textContent ?? "");
    expect(labels[0]).toMatch(/déjà 500 mL/);
    expect(labels[1]).toMatch(/lait de coco/);
  });

  it("reprend le nom et l'unité de la suggestion choisie", async () => {
    const user = userEvent.setup();
    renderScreen();
    const field = await nameField();
    await user.type(field, "farin");
    await user.click(await findSuggestion(/farine/));

    expect(field).toHaveValue("farine");
    // L'unité du dictionnaire est présélectionnée : la farine se pèse.
    expect(screen.getByLabelText("Unité")).toHaveValue("g");
    // La liste se referme une fois le choix fait.
    expect(suggestions()).toHaveLength(0);
  });

  it("ne propose rien quand la saisie ne ressemble à rien de connu", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(await nameField(), "zzz");

    expect(suggestions()).toHaveLength(0);
  });
});
