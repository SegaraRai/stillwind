import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";

import { analyzeModule } from "../../packages/host/src/compilation/compiler.ts";

const css = resolveFixtureCss('@import "stillwind"; @theme { --color-brand:red; }');

/** Loads the shared theme in `mode`, with canonical diagnostics disabled for sorting measurements. */
export function loadSortingDesignSystem(mode: "authored" | "tailwind"): NativeDesignSystemLoader {
  const loader = new NativeDesignSystemLoader({
    lint: { canonicalUtilities: { severity: "off" } },
  });
  loader.provide(
    0,
    "theme.css",
    `${css}\n@stillwind { utility-order:${mode}; }`,
    "file:///theme.css",
  );
  return loader;
}

/**
 * Analyzes `body` after importing the style API, using `loader`'s theme.
 * Compiler type facts remain disabled; editor sorting uses syntax identities and lexical bindings.
 * Returns the source, its analysis, and the session needed to construct an editor snapshot.
 */
export async function prepareSortingSource(loader: NativeDesignSystemLoader, body: string) {
  const entry = "/workspace/sorting.tsx";
  const source = `import {style} from 'stillwind/react';\n${body}`;
  const session = new NativeProjectSession(loader, false);
  const graph = await analyzeModule(session, () => undefined, entry, {
    describe: () => ({
      moduleId: "sorting.tsx",
      checkerFileName: entry,
      projectionFileName: "sorting.tsx",
      document: { uri: "file:///workspace/sorting.tsx", format: "script" },
      syntax: { language: "tsx", moduleKind: "module" },
    }),
    read: async () => source,
    resolve: async () => null,
    watch: () => {},
    warn: () => {},
  });
  return { entry, source, session, graph };
}
