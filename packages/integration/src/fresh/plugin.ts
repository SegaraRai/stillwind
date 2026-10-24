import type { Plugin } from "vite";

import { createVitePlugins } from "../vite/plugin.ts";
import { createViteProject, type IntegrationConfig } from "../vite/project.ts";
import { registrationHooks } from "./manifest.ts";

export type { IntegrationConfig } from "../vite/project.ts";

/**
 * Registers Stillwind after Fresh 2's Deno source loader. Place it after `fresh()`.
 * Macro identities use the original files in both environments, so SSR and islands
 * receive the same classes. Vite lowers the compiled JSX for Preact.
 */
export function stillwind(config: IntegrationConfig = {}): Plugin[] {
  return [
    ...createVitePlugins(createViteProject(config), {
      order: "normal",
      originalSource: true,
    }),
    registrationHooks(),
  ];
}
