import type { CssPart } from "./module-css.ts";

export type { CssPart } from "./module-css.ts";

/**
 * Constructed sheets cached by document and part key. A changed CSS value replaces only that key's
 * cached sheet.
 */
const documents = new WeakMap<Document, Map<string, { css: string; sheet: CSSStyleSheet }>>();

/**
 * Creates or reuses constructed sheets in the supplied document's Window.
 *
 * Updating a cached part creates a new sheet. Callers must adopt the returned sheets to apply it.
 *
 * @param parts - Ordered CSS parts. A part reuses the cached sheet for its key only when its CSS
 * matches the cached text.
 * @param document - Target document whose Window provides `CSSStyleSheet` and `replaceSync`.
 * @returns One sheet per input part, in input order, without adopting them into the document.
 * @throws If the document has no Window or constructing or parsing a stylesheet fails.
 */
export function styleSheets(parts: readonly CssPart[], document: Document): CSSStyleSheet[] {
  const realm = document.defaultView;
  if (!realm) {
    throw new Error("Stillwind stylesheets require a document with a Window");
  }
  let cache = documents.get(document);
  if (!cache) {
    documents.set(document, (cache = new Map()));
  }
  return parts.map((part) => {
    const previous = cache.get(part.key);
    if (previous?.css === part.css) {
      return previous.sheet;
    }
    const sheet = new realm.CSSStyleSheet();
    sheet.replaceSync(part.css);
    cache!.set(part.key, {
      css: part.css,
      sheet,
    });
    return sheet;
  });
}

/**
 * Concatenates CSS parts without removing repeated occurrences or adding separators.
 *
 * @param parts - CSS parts in the order required by the cascade.
 * @returns The combined stylesheet text.
 */
export function cssText(parts: readonly CssPart[]): string {
  return parts.map((part) => part.css).join("");
}

/** Build manifest that maps producer modules to stylesheet occurrences in cascade order. */
export interface StylesheetManifest {
  /** Manifest format version. The supported shape uses version 1. */
  readonly version: 1;
  /**
   * Build revision identifying the manifest. Link selection and loading do not compare this value.
   */
  readonly revision: string;
  /**
   * Stylesheet occurrences in cascade order. Each occurrence has a distinct ID even when URLs
   * repeat.
   */
  readonly parts: readonly {
    /**
     * Unique occurrence identity used by module selections and `data-stillwind-part` attributes.
     */
    readonly id: string;
    /**
     * Stylesheet URL assigned to a link element. Relative URLs resolve against the target document.
     */
    readonly href: string;
  }[];
  /**
   * Producer module identities mapped to occurrence IDs from `parts`. An empty list means no CSS.
   */
  readonly modules: Readonly<Record<string, readonly string[]>>;
}

/**
 * Selects stylesheet occurrences for producer modules in manifest order.
 *
 * @param manifest - Manifest whose module selections refer to IDs in its ordered parts.
 * @param modules - Producer identities to include. Repeated requests select each occurrence once.
 * @returns Selected parts with their zero-based position in the complete manifest as `order`.
 * @throws If a requested producer has no entry in `manifest.modules`.
 */
export function stylesheetLinks(manifest: StylesheetManifest, modules: readonly string[]) {
  const selected = new Set(
    modules.flatMap((module) => {
      const parts = manifest.modules[module];
      if (!parts) {
        throw new Error(`Unknown Stillwind stylesheet producer: ${module}`);
      }
      return parts;
    }),
  );
  return manifest.parts.flatMap((part, order) =>
    selected.has(part.id) ? [{ ...part, order }] : [],
  );
}

/**
 * Adds selected stylesheet links to the document head and waits for them to load.
 *
 * Existing links are matched by occurrence ID and retained, including their current URL. New links
 * are inserted before the first existing Stillwind link that follows them in manifest order. IDs
 * must identify stable resources across calls. This function does not refresh links by revision or
 * remove unselected links.
 *
 * @param manifest - Manifest containing the requested producers and their ordered stylesheet
 * occurrences.
 * @param modules - Producer identities whose stylesheet occurrences should be loaded.
 * @param document - Target document with a head for inserting links.
 * @returns A promise that resolves when all selected links have loaded or already expose a sheet.
 * @throws If a producer is unknown or a selected stylesheet emits an error event.
 */
export async function loadStyles(
  manifest: StylesheetManifest,
  modules: readonly string[],
  document: Document,
): Promise<void> {
  const selected = stylesheetLinks(manifest, modules);
  const order = new Map(manifest.parts.map((part, index) => [part.id, index]));
  await Promise.all(
    selected.map((part) => {
      const previous = [
        ...document.querySelectorAll<HTMLLinkElement>("link[data-stillwind-part]"),
      ].find((link) => link.dataset.stillwindPart === part.id);
      if (previous?.sheet) {
        return Promise.resolve();
      }
      const link = previous ?? document.createElement("link");
      const loaded = new Promise<void>((resolve, reject) => {
        link.addEventListener("load", () => resolve(), { once: true });
        link.addEventListener("error", () => reject(new Error(`Cannot load ${part.href}`)), {
          once: true,
        });
      });
      if (!previous) {
        link.rel = "stylesheet";
        link.href = part.href;
        link.dataset.stillwindPart = part.id;
        link.dataset.stillwindOrder = String(part.order);
        const next = [
          ...document.querySelectorAll<HTMLLinkElement>("link[data-stillwind-part]"),
        ].find(
          (existing) => (order.get(existing.dataset.stillwindPart!) ?? -1) > order.get(part.id)!,
        );
        document.head.insertBefore(link, next ?? null);
      }
      return loaded;
    }),
  );
}
