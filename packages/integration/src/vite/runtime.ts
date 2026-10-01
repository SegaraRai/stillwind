import type { EncodedSourceMap } from "@jridgewell/trace-mapping";

export interface StylesheetPart {
  css: string;
  /** Compiler module ids that need this part; an empty list keeps it active unconditionally. */
  producers: string[];
  map?: EncodedSourceMap;
}

export interface StylesheetSnapshot {
  /** Monotonic revision within the adapter environment. */
  revision: number;
  parts: StylesheetPart[];
  /** Composition dependencies keyed by compiler module id. */
  dependencies: Record<string, string[]>;
}

interface DocumentState {
  revision: number;
  style?: HTMLStyleElement;
  active: Set<string>;
  snapshot?: StylesheetSnapshot;
  pending?: Promise<void>;
}

/**
 * Reuses document state across runtime instances and applies the initial snapshot.
 * `endpoint` supplies snapshots for this environment. `renderCss`, when supplied,
 * must return CSS for the selected parts in their existing order.
 * Returns operations for producer registration, snapshot updates and synchronization.
 * Snapshots older than the shared document revision are ignored.
 */
export function createDevelopmentRuntime(options: {
  snapshot: StylesheetSnapshot;
  endpoint: string;
  renderCss?: (parts: StylesheetPart[]) => string;
}) {
  const key = Symbol.for("stillwind.document");
  const globals = globalThis as typeof globalThis & {
    [key: symbol]: DocumentState | undefined;
  };
  const state = (globals[key] ??= {
    revision: -1,
    active: new Set(),
  });

  function render() {
    if (!state.snapshot || typeof document === "undefined") {
      return;
    }
    const active = new Set(state.active);
    for (const module of active) {
      for (const dependency of state.snapshot.dependencies[module] ?? []) {
        active.add(dependency);
      }
    }
    const parts = state.snapshot.parts.filter(
      (part) => !part.producers.length || part.producers.some((producer) => active.has(producer)),
    );
    if (!state.style?.isConnected) {
      state.style = document.createElement("style");
      state.style.dataset.stillwind = "";
      document.head.append(state.style);
    }
    state.style.textContent = options.renderCss
      ? options.renderCss(parts)
      : parts.map((part) => part.css).join("");
  }

  function register(id: string) {
    state.active.add(id);
    render();
  }

  function unregister(id: string) {
    state.active.delete(id);
    render();
  }

  /**
   * Fetches the current snapshot, sharing pending requests between producers.
   * Resolves after the document reaches `revision`; rejects on fetch or response
   * failures. Does not fetch when no document exists.
   */
  async function synchronize(revision: number) {
    if (typeof document === "undefined") {
      return;
    }
    do {
      state.pending ??= fetch(options.endpoint)
        .then((response) => {
          if (!response.ok) {
            throw new Error("Stillwind stylesheet synchronization failed");
          }
          return response.json() as Promise<StylesheetSnapshot>;
        })
        .then(apply)
        .finally(() => {
          state.pending = undefined;
        });
      await state.pending;
    } while (state.revision < revision);
  }

  function apply(update: StylesheetSnapshot) {
    if (update.revision < state.revision) {
      return;
    }
    state.revision = update.revision;
    state.snapshot = update;
    render();
  }

  apply(options.snapshot);
  return {
    register,
    unregister,
    synchronize,
    apply,
  };
}
