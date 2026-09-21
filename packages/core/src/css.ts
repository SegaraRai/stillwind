import type { CssPart } from "./module-css.ts";

export type { CssPart } from "./module-css.ts";

const documents = new WeakMap<Document, Map<string, { css: string; sheet: CSSStyleSheet }>>();

/**
 * Returns shared constructed sheets in plan order, without adopting them.
 * Treat the sheets as immutable: mutations affect every adopter in this Document.
 * Throws if the Document has no Window or constructing a stylesheet fails.
 */
export function styleSheets(parts: readonly CssPart[], document: Document): CSSStyleSheet[] {
  const realm = document.defaultView;
  if (!realm) throw new Error("Stillwind stylesheets require a document with a Window");
  let cache = documents.get(document);
  if (!cache) documents.set(document, (cache = new Map()));
  return parts.map((part) => {
    const previous = cache.get(part.key);
    if (previous?.css === part.css) return previous.sheet;
    const sheet = new realm.CSSStyleSheet();
    sheet.replaceSync(part.css);
    cache!.set(part.key, { css: part.css, sheet });
    return sheet;
  });
}

/** Concatenates every occurrence, including repeated payloads. */
export function cssText(parts: readonly CssPart[]): string {
  return parts.map((part) => part.css).join("");
}

/** Ordered stylesheet occurrences emitted by a complete build. */
export interface StylesheetManifest {
  readonly version: 1;
  readonly revision: string;
  readonly parts: readonly { readonly id: string; readonly href: string }[];
  readonly modules: Readonly<Record<string, readonly string[]>>;
}

/**
 * Resolves producer selections to link descriptors in manifest order.
 * Throws if a selected producer is absent from the manifest.
 */
export function stylesheetLinks(manifest: StylesheetManifest, modules: readonly string[]) {
  const selected = new Set(
    modules.flatMap((module) => {
      const parts = manifest.modules[module];
      if (!parts) throw new Error(`Unknown Stillwind stylesheet producer: ${module}`);
      return parts;
    }),
  );
  return manifest.parts.flatMap((part, order) =>
    selected.has(part.id) ? [{ ...part, order }] : [],
  );
}

/**
 * Installs missing links in manifest order and resolves after loading.
 * Existing Stillwind links must use this manifest and already be in manifest order.
 * Rejects if a producer is unknown or a stylesheet fails to load.
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
      if (previous?.sheet) return Promise.resolve();
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
