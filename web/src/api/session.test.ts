import { afterEach, describe, expect, it, vi } from "vitest";
import { queryClient } from "../query";
import { ApiError } from "./client";
import { requireSession } from "./session";

vi.mock("./client", async () => {
  const actual = await vi.importActual<typeof import("./client")>("./client");
  return { ...actual, api: { get: vi.fn() } };
});

const { api } = await import("./client");
const get = vi.mocked(api.get);

afterEach(() => {
  queryClient.clear();
  vi.clearAllMocks();
});

const user = {
  user_id: "u-1",
  household_id: "h-1",
  username: "robin",
};

describe("garde de session", () => {
  it("laisse passer une session valide", async () => {
    get.mockResolvedValue(user);
    await expect(requireSession("/recipes")).resolves.toEqual(user);
  });

  it("redirige vers /login sur 401, en mémorisant l'écran demandé", async () => {
    // Le bug d'origine : sans garde, un utilisateur déconnecté atterrissait sur
    // /recipes, l'appel partait en 401, et l'écran affichait « Aucune recette »
    // — l'écran de connexion n'était jamais atteint.
    get.mockRejectedValue(new ApiError(401, "non connecté"));

    const thrown = await requireSession("/week").catch((e: unknown) => e);
    expect(thrown).toMatchObject({
      options: { to: "/login", search: { redirect: "/week" } },
    });
  });

  it("ne déguise pas une panne d'API en déconnexion", async () => {
    // Rediriger vers /login sur un 500 ferait croire à une session expirée et
    // renverrait l'utilisateur sur un formulaire qui échouerait tout autant.
    get.mockRejectedValue(new ApiError(500, "boom"));

    const thrown = await requireSession("/recipes").catch((e: unknown) => e);
    expect(thrown).toBeInstanceOf(ApiError);
    expect((thrown as ApiError).status).toBe(500);
  });

  it("ne rappelle pas l'API à chaque navigation", async () => {
    get.mockResolvedValue(user);
    await requireSession("/recipes");
    await requireSession("/week");
    expect(get).toHaveBeenCalledTimes(1);
  });
});
