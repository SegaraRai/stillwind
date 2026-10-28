/**
 * Storage that keeps entries in memory for the lifetime of the preview frame.
 * It implements the Storage methods; named property access such as `storage.theme` is not supported.
 */
export class MemoryStorage implements Storage {
  readonly #entries = new Map<string, string>();

  get length(): number {
    return this.#entries.size;
  }

  key(index: number): string | null {
    return [...this.#entries.keys()][index] ?? null;
  }

  getItem(key: string): string | null {
    return this.#entries.get(String(key)) ?? null;
  }

  setItem(key: string, value: string): void {
    this.#entries.set(String(key), String(value));
  }

  removeItem(key: string): void {
    this.#entries.delete(String(key));
  }

  clear(): void {
    this.#entries.clear();
  }

  [name: string]: unknown;
}

/**
 * Adapts the preview frame to its sandbox. The frame has an opaque origin, and it must keep its
 * document because the runtime and the build channel live in it.
 *
 * - Form submissions never navigate the frame. Submit handlers still run.
 * - Clicks on links that would load another document in the frame are cancelled unless a handler
 *   already cancelled them. A fragment link scrolls to its target instead, because the frame
 *   resolves `#id` against the playground URL.
 * - `localStorage` and `sessionStorage` become memory storage when the opaque origin denies access.
 *
 * @param target - Window of the preview frame. Call once before application code runs.
 */
export function installSandboxGuards(target: Window): void {
  target.addEventListener(
    "submit",
    (event) => {
      event.preventDefault();
    },
    {
      capture: true,
    },
  );

  // A bubbling listener on the window runs after React's handlers on the root element.
  target.addEventListener("click", (event) => {
    if (event.defaultPrevented || !(event.target instanceof Element)) {
      return;
    }

    const link = event.target.closest("a[href]");
    const frame = link?.getAttribute("target") ?? "";
    if (!link || link.hasAttribute("download") || !["", "_self"].includes(frame)) {
      return;
    }

    event.preventDefault();
    const href = link.getAttribute("href")!;
    if (href.startsWith("#")) {
      target.document.getElementById(decodeURIComponent(href.slice(1)))?.scrollIntoView();
    }
  });

  for (const name of ["localStorage", "sessionStorage"] as const) {
    try {
      void target[name];
    } catch {
      Object.defineProperty(target, name, {
        value: new MemoryStorage(),
        configurable: true,
      });
    }
  }
}
