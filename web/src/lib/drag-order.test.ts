import { describe, expect, it } from "vitest";
import { move } from "./drag-order";

describe("move", () => {
  const items = ["a", "b", "c", "d"];

  it("déplace vers le bas", () => {
    expect(move(items, 0, 2)).toEqual(["b", "c", "a", "d"]);
  });

  it("déplace vers le haut", () => {
    expect(move(items, 3, 1)).toEqual(["a", "d", "b", "c"]);
  });

  it("ne touche à rien pour un rang identique ou hors bornes", () => {
    expect(move(items, 1, 1)).toBe(items);
    expect(move(items, 0, 9)).toBe(items);
    expect(move(items, -1, 0)).toBe(items);
  });

  it("ne modifie pas le tableau d'origine", () => {
    move(items, 0, 3);
    expect(items).toEqual(["a", "b", "c", "d"]);
  });
});
