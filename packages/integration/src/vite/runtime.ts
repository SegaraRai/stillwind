import type { EncodedSourceMap } from "@jridgewell/trace-mapping";

/** Ordered CSS partition activated by its producers or included unconditionally. */
export interface StylesheetPart {
  /** CSS with resources already resolved for the browser. */
  css: string;
  /** Compiler module ids that need this part; an empty list keeps it active unconditionally. */
  producers: string[];
  /** Map for this part. Null or omission means that no map is available. */
  map?: EncodedSourceMap | null;
}

/** Complete stylesheet and composition state published for one adapter revision. */
export interface StylesheetSnapshot {
  /** Monotonic revision within the adapter environment. */
  revision: number;
  /** Partitions in stylesheet order, retained when filtering for active producers. */
  parts: StylesheetPart[];
  /** Composition dependencies keyed by compiler module id. */
  dependencies: Record<string, string[]>;
}

declare module "vite" {
  /** Extends Vite's typed HMR events with complete Stillwind stylesheet snapshots. */
  interface CustomEventMap {
    /** Stylesheet revision, ordered parts, and composition dependencies published by Stillwind. */
    "stillwind:css": StylesheetSnapshot;
  }
}

/** Mutable stylesheet state shared by runtime instances in the current global document context. */
interface DocumentState {
  /** Latest accepted revision. -1 identifies state before its first snapshot. */
  revision: number;
  /** Owned style element, recreated when it is no longer connected to the document. */
  style?: HTMLStyleElement;
  /** Registered compiler producer identities. Composition dependencies are expanded during rendering. */
  active: Set<string>;
  /** Latest accepted complete snapshot, retained by reference. */
  snapshot?: StylesheetSnapshot;
  /** Fetch-and-apply operation shared by overlapping synchronization requests. */
  pending?: Promise<void>;
}

const documentKey = Symbol.for("stillwind.document");

/**
 * Reuses document state across runtime instances and applies the initial snapshot.
 * `endpoint` supplies snapshots for this environment. `renderCss`, when supplied,
 * must return CSS for the selected parts in their existing order.
 * Returns operations for producer registration, snapshot updates and synchronization.
 * Snapshots older than the shared document revision are ignored.
 * @param options - Initial snapshot, snapshot endpoint, and optional renderer. Keep supplied snapshots immutable.
 * @returns Operations that update shared producer state and the owned style element.
 */
export function createDevelopmentRuntime(options: {
  /** Initial complete snapshot applied unless its revision is older than shared document state. */
  snapshot: StylesheetSnapshot;
  /** URL returning a JSON snapshot for this compiler environment. */
  endpoint: string;
  /** Optional renderer receiving selected parts in stylesheet order and returning their combined CSS. */
  renderCss?: (parts: StylesheetPart[]) => string;
}) {
  const globals = globalThis as typeof globalThis & {
    /** State shared through Symbol.for across runtime module replacements. */
    [documentKey]?: DocumentState;
  };
  const state = (globals[documentKey] ??= {
    revision: -1,
    active: new Set(),
  });

  /** Expands composition dependencies and writes active partitions into the shared style element. */
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

  /**
   * Activates a producer and rerenders its CSS and composition dependencies.
   * @param id - Compiler module identity. Repeated registration has no additional ownership count.
   */
  function register(id: string) {
    state.active.add(id);
    render();
  }

  /**
   * Removes a producer registration and rerenders the remaining active CSS.
   * @param id - Compiler module identity. Unknown identities have no effect on the set.
   */
  function unregister(id: string) {
    state.active.delete(id);
    render();
  }

  /**
   * Fetches the current snapshot, sharing pending requests between producers.
   * Resolves after the document reaches `revision`; rejects on fetch or response
   * failures. Does not fetch when no document exists.
   * @param revision - Minimum adapter revision required before completion.
   * @returns Completion after a fetched snapshot reaches the requested revision, or immediately without a document.
   * @throws If fetching, decoding, or applying a snapshot fails.
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

  /**
   * Accepts a current or newer snapshot and rerenders shared document styles.
   * @param update - Complete immutable snapshot. Older revisions are ignored, and equal revisions are accepted.
   */
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
