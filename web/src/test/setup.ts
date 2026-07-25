import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(cleanup);

// jsdom n'implémente pas `EventSource` (utilisé par la synchro temps réel de la
// liste de courses). Stub inerte : il ne se connecte à rien et n'émet jamais,
// ce qui suffit à rendre les composants sans planter en test.
if (typeof globalThis.EventSource === "undefined") {
  class EventSourceStub extends EventTarget {
    static readonly CONNECTING = 0;
    static readonly OPEN = 1;
    static readonly CLOSED = 2;
    readonly readyState = EventSourceStub.CONNECTING;
    onopen: (() => void) | null = null;
    onerror: (() => void) | null = null;
    onmessage: (() => void) | null = null;
    close(): void {}
  }
  globalThis.EventSource = EventSourceStub as unknown as typeof EventSource;
}
