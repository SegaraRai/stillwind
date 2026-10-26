import { type AnalysisProjectConfig } from "@stillwind/config";

import { BrowserProject } from "../../src/browser/index.ts";

export const theme = `
@stillwind-theme-namespace --spacing-* [length] scale;
@stillwind-theme-namespace --color-* [color];
@theme { --spacing: .25rem; --color-brand: #123456; --color-bright: red; }
@utility block { display: block; }
@utility flex { display: flex; }
@utility p-* { padding: --value(--spacing-*, --spacing(), [length]); }
@utility border-* { border-width: calc(--value(integer) * 1px); border-width: --value([length]); }
@utility bg-* { background-color: --value(--color-*, [color]); }
@variant hover { &:hover { @slot; } }
@variant focus { &:focus { @slot; } }
@stillwind-slot utilities;
`;

/**
 * Creates an isolated virtual project for queries that do not require semantic type facts.
 *
 * @param source - Original TSX text stored at `/workspace/entry.tsx`.
 * @param config - Overrides for the default entry, CSS path, and disabled type facts.
 * @param css - Complete stylesheet stored at `/workspace/theme.css`.
 * @returns A project whose lifecycle the caller must end with `close()`.
 */
export function project(source: string, config: Partial<AnalysisProjectConfig> = {}, css = theme) {
  return new BrowserProject({
    root: "/workspace",
    files: new Map([
      ["/workspace/entry.tsx", source],
      ["/workspace/theme.css", css],
    ]),
    config: {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
      ...config,
    },
  });
}
