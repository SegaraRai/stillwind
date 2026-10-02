import { cleanModuleId, composeFrameworkSourceMap } from "@stillwind/host";
import type { Plugin } from "vite";

import type { ViteProject } from "./project.ts";

type Load = Extract<NonNullable<Plugin["load"]>, Function>;
type Transform = Extract<NonNullable<Plugin["transform"]>, Function>;
const hooks = new WeakMap<Plugin, Pick<Plugin, "load" | "transform">>();

/**
 * Composes Vue subrequest maps before Vite adds them to its map chain.
 * The supplied plugins must be the resolved plugins for this project.
 * Vue loads inline templates through its script subrequests during production builds.
 */
export function attachVueSourceMaps(plugins: readonly Plugin[], project: ViteProject): void {
  const compose = <T extends Awaited<ReturnType<Load>> | Awaited<ReturnType<Transform>>>(
    result: T,
    id: string,
    name?: string,
  ) => {
    if (!result || typeof result !== "object" || !/[?&]vue(?:[=&]|$)/.test(id) || !result.map) {
      return result;
    }
    const input = project.inputMap(name).get(cleanModuleId(id));
    if (!input?.sourceMap) {
      return result;
    }
    const map = typeof result.map === "string" ? JSON.parse(result.map) : result.map;
    if (!map.sources?.length) {
      return result;
    }
    return {
      ...result,
      map: JSON.stringify(
        composeFrameworkSourceMap(map, JSON.stringify(input.sourceMap), input.javascript),
      ),
    };
  };
  for (const plugin of plugins) {
    if (plugin.name !== "vite:vue") {
      continue;
    }
    const original = hooks.get(plugin) ?? { load: plugin.load, transform: plugin.transform };
    hooks.set(plugin, original);
    if (original.load) {
      const hook = original.load;
      const handler = typeof hook === "function" ? hook : hook.handler;
      const wrapped: Load = async function (id, ...args) {
        return compose(await handler.call(this, id, ...args), id, this.environment?.name);
      };
      plugin.load = typeof hook === "function" ? wrapped : { ...hook, handler: wrapped };
    }
    if (original.transform) {
      const hook = original.transform;
      const handler = typeof hook === "function" ? hook : hook.handler;
      const wrapped: Transform = async function (code, id, ...args) {
        return compose(await handler.call(this, code, id, ...args), id, this.environment?.name);
      };
      plugin.transform = typeof hook === "function" ? wrapped : { ...hook, handler: wrapped };
    }
  }
}
