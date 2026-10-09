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
  /**
   * Composes a Vue block's map through the recorded Stillwind transform for its source file.
   * @param result - Original load or transform result, left unchanged when a required map is absent.
   * @param id - Full Vue block identity, including its `vue` query.
   * @param name - Vite environment name, or undefined for `client`.
   * @returns The original result or a copy containing a serialized composed map.
   * @throws If the input map cannot be parsed or the maps cannot be composed.
   */
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
      /**
       * Runs the saved Vue load hook with its original receiver, then composes its map.
       * @param id - Resolver identity passed by Vite.
       * @param args - Remaining load-hook arguments forwarded unchanged.
       * @returns The delegated result with its map composed when the recorded input permits it.
       * @throws If the original hook or map composition fails.
       */
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
      /**
       * Runs the saved Vue transform hook with its original receiver, then composes its map.
       * @param code - Vue block source passed by Vite.
       * @param id - Resolver identity passed by Vite.
       * @param args - Remaining transform-hook arguments forwarded unchanged.
       * @returns The delegated result with its map composed when the recorded input permits it.
       * @throws If the original hook or map composition fails.
       */
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
