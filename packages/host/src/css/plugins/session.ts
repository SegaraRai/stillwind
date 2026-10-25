import { NativeDesignSystemLoader, type NativeProjectSession } from "@stillwind/native";

import type { PluginRuntime } from "./runtime.ts";

/**
 * A session's plugin runtime and the definition generation that its design system contains.
 */
interface SessionPlugins {
  /** Runtime of the CSS graph that the session was created or reloaded from. */
  runtime: PluginRuntime;
  /** Value of {@link PluginRuntime.generation} that the session's design system contains. */
  generation: number;
  /** Loader of the session's design system after the last reload with definitions. */
  loader?: NativeDesignSystemLoader;
}

/** Runtimes of sessions whose design systems load plugins with dynamic families. */
const sessions = new WeakMap<NativeProjectSession, SessionPlugins>();

/**
 * Associates a session with the plugin runtime of the CSS graph that it was created or reloaded
 * from, so that compilation can define dynamic candidates.
 *
 * @param session - Native session.
 * @param runtime - Runtime returned with the graph, or undefined to detach.
 * @param generation - Definition generation of the loader that the session uses. Defaults to
 * zero, the generation of the graph's original loader.
 */
export function attachPlugins(
  session: NativeProjectSession,
  runtime?: PluginRuntime,
  generation = 0,
): void {
  if (runtime?.dynamic) {
    sessions.set(session, {
      runtime,
      generation,
    });
  } else {
    sessions.delete(session);
  }
}

/**
 * Compiles with the definitions of the attached plugins, and defines the dynamic candidates that
 * the compilation reports.
 *
 * A session whose design system lacks definitions that the runtime added later is reloaded
 * first. After each compilation, new candidates go to the plugins, and when they return CSS the
 * session is reloaded and `compile` runs again, at most `rounds` more times. A session without
 * plugins runs `compile` once.
 *
 * @param session - Native session whose output `compile` returns.
 * @param compile - Compiles the graph into the session and returns its module outputs.
 * @param rounds - Most reloads after the first compilation.
 * @returns The output of the last compilation.
 * @throws If a plugin, a reload, or `compile` fails.
 */
export async function compileWithPlugins<
  Output extends {
    modules: readonly {
      dynamicCandidates: readonly string[];
    }[];
  },
>(session: NativeProjectSession, compile: () => Output, rounds: number): Promise<Output> {
  const binding = sessions.get(session);
  if (!binding) {
    return compile();
  }
  /** Reloads the session with the runtime's current definitions. */
  const sync = () => {
    const loader = binding.runtime.replay(session, true);
    session.reload(loader);
    binding.generation = binding.runtime.generation;
    binding.loader = loader;
  };
  if (binding.generation < binding.runtime.generation) {
    sync();
  }
  let output = compile();
  for (let round = 0; round < rounds; round++) {
    const candidates = output.modules.flatMap((module) => module.dynamicCandidates);
    if (!(await binding.runtime.resolve(candidates))) {
      break;
    }
    sync();
    output = compile();
  }
  return output;
}

/**
 * Returns the loader whose design system a session uses after plugin definitions were added.
 *
 * @param session - Native session.
 * @returns The loader of the last reload with definitions, or undefined when the session still
 * uses the loader that it was created or attached with.
 */
export function pluginLoader(session: NativeProjectSession): NativeDesignSystemLoader | undefined {
  return sessions.get(session)?.loader;
}

/**
 * Finds plugin candidates across source texts before individual modules are compiled.
 *
 * @param session - Session attached to the project's plugin runtime.
 * @param loader - Initial loader of the same session.
 * @param texts - Original source documents, including templates and embedded scripts.
 * @returns Completion of candidate resolution and any required CSS reload, without publishing modules.
 * @throws If a plugin or CSS reload fails. Discard this runtime before retrying individual modules.
 */
export async function prepareSourcePlugins(
  session: NativeProjectSession,
  loader: NativeDesignSystemLoader,
  texts: readonly string[],
): Promise<void> {
  if (!sessions.has(session)) {
    return;
  }
  await compileWithPlugins(
    session,
    () => {
      const query = (pluginLoader(session) ?? loader).editorQuery();
      return {
        modules: [
          {
            dynamicCandidates: [...new Set(texts.flatMap((text) => query.dynamicCandidates(text)))],
          },
        ],
      };
    },
    1,
  );
}
