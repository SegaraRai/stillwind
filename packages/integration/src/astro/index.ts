import type { AstroIntegration } from "astro";

import { stillwind as vitePlugin, type StillwindOptions } from "../vite/index.ts";

export type { StillwindOptions } from "../vite/index.ts";

/** Astro configuration hook wrapping the public Vite adapter. */
export function stillwind(options: StillwindOptions): AstroIntegration {
  return {
    name: "@stillwind/integration",
    hooks: {
      "astro:config:setup"({ updateConfig }) {
        updateConfig({ vite: { plugins: [vitePlugin(options)] } });
      },
    },
  };
}
