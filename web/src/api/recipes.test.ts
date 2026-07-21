import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./client";
import { scrapeRecipe } from "./recipes";

/** Réponse `fetch` minimale, suffisante pour le client. */
function response(status: number, body?: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: `status ${status}`,
    json: async () => {
      if (body === undefined) throw new Error("pas de corps JSON");
      return body;
    },
    headers: new Headers({ "Content-Type": "application/json" }),
  } as Response;
}

function mockFetch(...responses: Response[]) {
  const fetchMock = vi.fn();
  for (const r of responses) fetchMock.mockResolvedValueOnce(r);
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => vi.unstubAllGlobals());

describe("scrapeRecipe", () => {
  const draft = {
    title: "Ratatouille",
    prep_time_min: 25,
    cook_time_min: 45,
    photo: "https://example.test/rata.jpg",
    ingredients: [{ name: "courgette", amount: 600, unit: "g" }],
    steps: ["Émincer.", "Laisser mijoter."],
  };

  it("POST /recipes/scrape avec l'URL et renvoie le brouillon", async () => {
    const fetchMock = mockFetch(response(200, draft));
    const result = await scrapeRecipe("https://example.test/rata");

    const [path, init] = fetchMock.mock.calls[0];
    expect(path).toContain("/recipes/scrape");
    expect(init.method).toBe("POST");
    expect(init.body).toBe(JSON.stringify({ url: "https://example.test/rata" }));
    expect(result).toEqual(draft);
  });

  it("remonte le message d'erreur de l'API (page sans recette)", async () => {
    mockFetch(response(422, { error: "aucune recette n'a été trouvée sur cette page" }));
    const error = (await scrapeRecipe("https://example.test/vide").catch(
      (e: unknown) => e,
    )) as ApiError;

    expect(error).toBeInstanceOf(ApiError);
    expect(error.status).toBe(422);
    expect(error.message).toBe("aucune recette n'a été trouvée sur cette page");
  });
});
