import { cleanModuleId, composeFrameworkSourceMap } from "@stillwind/host";
import type { Plugin } from "vite";

import type { ViteProject } from "./project.ts";

/**
 * Callable form of a Vite load hook, used to wrap either hook representation without changing its
 * options.
 */
type Load = Extract<NonNullable<Plugin["load"]>, Function>;

/**
 * Callable form of a Vite transform hook, used to wrap either hook representation without changing
 * its options.
 */
type Transform = Extract<NonNullable<Plugin["transform"]>, Function>;

/**
 * Original Vue hooks saved before wrapping. Reusing them prevents repeated configuration from
 * composing a wrapper with itself.
 */
const hooks = new WeakMap<Plugin, Pick<Plugin, "load" | "transform">>();

/**
 * Wraps Vue load and transform hooks to map generated blocks back through Stillwind transforms.
 *
 * Only Vue query results with a source map and a recorded Stillwind map are composed. Repeated
 * calls reuse the saved original hooks.
 *
 * @param plugins - Resolved plugin instances. Hooks on plugins named `vite:vue` are replaced in
 * place.
 * @param project - Project containing Stillwind transforms for the corresponding environment and
 * original Vue file.
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
    const input = project.inputs(name).get(cleanModuleId(id));
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
    const original = hooks.get(plugin) ?? {
      load: plugin.load,
      transform: plugin.transform,
    };
    hooks.set(plugin, original);
    if (original.load) {
      const hook = original.load;
      const handler = typeof hook === "function" ? hook : hook.handler;
      const wrapped: Load = async function (id, ...args) {
        return compose(await handler.call(this, id, ...args), id, this.environment?.name);
      };
      plugin.load =
        typeof hook === "function"
          ? wrapped
          : {
              ...hook,
              handler: wrapped,
            };
    }
    if (original.transform) {
      const hook = original.transform;
      const handler = typeof hook === "function" ? hook : hook.handler;
      const wrapped: Transform = async function (code, id, ...args) {
        return compose(await handler.call(this, code, id, ...args), id, this.environment?.name);
      };
      plugin.transform =
        typeof hook === "function"
          ? wrapped
          : {
              ...hook,
              handler: wrapped,
            };
    }
  }
}
