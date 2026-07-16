import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeProvider } from "./ThemeProvider";
import { useTheme } from "./theme-context";

/** Pilote `prefers-color-scheme`, absent de jsdom. */
function stubSystemDark(dark: boolean) {
  const listeners = new Set<(e: MediaQueryListEvent) => void>();
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({
      matches: query.includes("dark") && dark,
      media: query,
      addEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) =>
        listeners.add(fn),
      removeEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) =>
        listeners.delete(fn),
    })),
  );
  return {
    emit: (nowDark: boolean) =>
      listeners.forEach((fn) => fn({ matches: nowDark } as MediaQueryListEvent)),
  };
}

function Probe() {
  const { preference, resolved, setPreference } = useTheme();
  return (
    <div>
      <span data-testid="state">{`${preference}/${resolved}`}</span>
      <button onClick={() => setPreference("light")}>Clair</button>
      <button onClick={() => setPreference("system")}>Système</button>
    </div>
  );
}

const state = () => screen.getByTestId("state").textContent;
const attr = () => document.documentElement.getAttribute("data-theme");

beforeEach(() => localStorage.clear());
afterEach(() => {
  vi.unstubAllGlobals();
  document.documentElement.removeAttribute("data-theme");
});

describe("ThemeProvider", () => {
  it("résout « système » selon la préférence de l'appareil", () => {
    stubSystemDark(true);
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    expect(state()).toBe("system/dark");
    // `data-theme` porte toujours le thème résolu : c'est la seule source lue
    // par les tokens, il ne doit jamais être absent en mode « système ».
    expect(attr()).toBe("dark");
  });

  it("laisse la bascule explicite primer sur l'appareil, dans les deux sens", async () => {
    stubSystemDark(true);
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    await userEvent.click(screen.getByText("Clair"));

    expect(state()).toBe("light/light");
    expect(attr()).toBe("light");
  });

  it("persiste la préférence", async () => {
    stubSystemDark(false);
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    await userEvent.click(screen.getByText("Clair"));
    expect(localStorage.getItem("week-meals.theme")).toBe("light");
  });

  it("suit un changement d'appareil quand la préférence est « système »", () => {
    const system = stubSystemDark(false);
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    expect(state()).toBe("system/light");

    act(() => system.emit(true));
    expect(state()).toBe("system/dark");
    expect(attr()).toBe("dark");
  });

  it("synchronise theme-color sur le thème résolu", () => {
    const meta = document.createElement("meta");
    meta.setAttribute("name", "theme-color");
    document.head.appendChild(meta);
    stubSystemDark(true);

    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    expect(meta.getAttribute("content")).toBe("#1c2a22");
    meta.remove();
  });
});
