import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "./client";

/** Réponse `fetch` minimale, suffisante pour le client. */
function response(
  status: number,
  body?: unknown,
  contentType = "application/json",
): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: `status ${status}`,
    json: async () => {
      if (body === undefined) throw new Error("pas de corps JSON");
      return body;
    },
    headers: new Headers({ "Content-Type": contentType }),
  } as Response;
}

function mockFetch(...responses: Response[]) {
  const fetchMock = vi.fn();
  for (const r of responses) fetchMock.mockResolvedValueOnce(r);
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => vi.unstubAllGlobals());

describe("client API", () => {
  it("n'envoie pas de Content-Type sur un GET", async () => {
    // `Content-Type: application/json` n'est pas safelisté CORS : le poser sur
    // une lecture déclencherait un préflight OPTIONS à chaque appel, alors que
    // le front et l'API sont sur des origines distinctes en prod.
    const fetchMock = mockFetch(response(200, []));
    await api.get("/recipes");

    const headers = fetchMock.mock.calls[0][1].headers as Record<string, string>;
    expect(headers["Content-Type"]).toBeUndefined();
  });

  it("envoie le Content-Type JSON quand il y a un corps", async () => {
    const fetchMock = mockFetch(response(201, { id: "abc" }));
    await api.post("/recipes", { title: "Ratatouille" });

    const init = fetchMock.mock.calls[0][1];
    expect((init.headers as Record<string, string>)["Content-Type"]).toBe(
      "application/json",
    );
    expect(init.body).toBe(JSON.stringify({ title: "Ratatouille" }));
  });

  it("envoie toujours le cookie de session", async () => {
    const fetchMock = mockFetch(response(200, []));
    await api.get("/recipes");
    expect(fetchMock.mock.calls[0][1].credentials).toBe("include");
  });

  it("remonte le statut HTTP dans ApiError", async () => {
    mockFetch(response(401, { error: "non connecté" }));
    const error = await api.get("/auth/me").catch((e: unknown) => e);

    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(401);
    expect((error as ApiError).message).toBe("non connecté");
  });

  it("retombe sur statusText quand le corps d'erreur n'est pas du JSON", async () => {
    mockFetch(response(500, undefined, "text/html"));
    const error = (await api.get("/recipes").catch((e: unknown) => e)) as ApiError;
    expect(error.message).toBe("status 500");
  });

  it("ne tente pas de parser un 204", async () => {
    mockFetch(response(204));
    await expect(api.delete("/recipes/abc")).resolves.toBeUndefined();
  });
});
