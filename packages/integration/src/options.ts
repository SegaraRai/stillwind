import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import type { NativeIdentifierNaming, NativeDesignSystemLoader } from "@stillwind/native";

/**
 * Shared compiler configuration for build integrations. `css` or `theme` must identify
 * an entry stylesheet; `css` takes precedence when both are supplied. Relative paths
 * are resolved against the adapter project root.
 */
export interface StillwindOptions {
  /** Project root override, relative to the bundler root. */
  root?: string;
  /** Apply producer CSS to the document. Disable for libraries that supply explicit CSS values. */
  automatic?: boolean;
  /** Entry stylesheet alias used when `css` is absent; absolute or project-root relative. */
  theme?: string;
  /** Entry stylesheet defining the design system; takes precedence over `theme`. */
  css?: string;
  /**
   * Destination for the theme vocabulary's module augmentation, absolute or project-root
   * relative. Defaults to `src/stillwind.generated.d.ts`; `false` disables generation.
   */
  types?: false | string;
  /** Namespace for Stillwind-owned CSS identifiers. */
  prefix?: string;
  /** Namespace for Tailwind-compatible internal custom properties. */
  tailwindPrefix?: string;
  /** Reject Tailwind group/peer/container relationships in favor of typed refs. */
  forbidTailwindRelationships?: boolean;
  /**
   * Stable namespace included in generated class identities. Use the same value for
   * client and server builds; changing it changes those identities. Defaults to empty.
   */
  classNamespace?: string;
  /**
   * Emit native CSS if() for experimental values and variants; false uses fallback helpers.
   * Defaults to false. Comparisons, negated variants, and CSS-wide helper branches require true.
   */
  useCssIf?: boolean;
  /**
   * Host name generators, one optional function per identifier family. A function
   * returns the name to write, or `undefined` to keep the compiler's own name. A
   * generator must be pure: the same request has to produce the same name. Keep the
   * functions stable for the life of the session, and update `namingRevision` with them.
   */
  naming?: NativeIdentifierNaming;
  /** Hash of `naming`, used to invalidate cached artifacts when it changes. */
  namingRevision?: string;
  /**
   * Base-36 digits a built-in generated name keeps from its request hash. Defaults to 5;
   * values from 1 to 50 are accepted. Raising it lowers the chance of a collision.
   */
  hashLength?: number;
}

export function cssEntrypoint(options: StillwindOptions): string {
  const path = options.css ?? options.theme;
  if (!path) {
    throw new Error("Stillwind requires a CSS-first entrypoint (`css`)");
  }
  return path;
}

export function compilerOptions(options: StillwindOptions) {
  return {
    prefix: options.prefix ?? "--sw-",
    tailwindPrefix: options.tailwindPrefix ?? "--tw-",
    forbidTailwindRelationships: options.forbidTailwindRelationships ?? false,
    classNamespace: options.classNamespace ?? "",
    useCssIf: options.useCssIf ?? false,
    namingRevision: options.namingRevision ?? "",
    hashLength: options.hashLength,
  };
}

export function writeVocabulary(
  loader: NativeDesignSystemLoader,
  root: string,
  options: StillwindOptions,
): void {
  if (options.types === false) {
    return;
  }
  const file = resolve(root, options.types ?? "src/stillwind.generated.d.ts");
  const content = loader.generateVocabularyTypes();
  try {
    if (readFileSync(file, "utf8") === content) {
      return;
    }
  } catch {}
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, content);
}
