import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";

import { moduleSourceMap } from "@stillwind/host";
import type { AstroIntegration } from "astro";
import type { ResolvedConfig } from "vite";

import { renderChunkStyles } from "../vite/chunk-styles.ts";
import { createVitePlugins } from "../vite/plugin.ts";
import {
  createViteProject,
  type EnvironmentOwner,
  type IntegrationConfig,
} from "../vite/project.ts";
import { registerDevelopmentCss } from "./development.ts";

export type { IntegrationConfig } from "../vite/project.ts";

/** Virtual server module imported by Astro middleware to obtain document stylesheet markup. */
const ID = "virtual:stillwind/astro-styles";

/** Virtual client module that updates inline SSR styles when development CSS changes. */
const CLIENT_ID = ID + "/client";

/** External server module that connects SSR rendering to the active development compiler. */
const BRIDGE_ID = "@stillwind/integration/internal/astro-development";

/**
 * JavaScript string literal replaced with stylesheet links after the production graph is rendered.
 */
const MARKER = '"__STILLWIND_ASTRO_STYLES_574f6dfce7c69466ab9516fb7fc6199a__"';

/**
 * Creates an Astro integration that applies Stillwind CSS to server-rendered documents and client
 * islands.
 *
 * Development SSR reads current CSS through a server bridge and embeds it in HTML. Production SSR
 * inserts links from the stylesheet manifest.
 *
 * @param config - File discovery, project selection, compiler overrides, and Vite CSS emission.
 * Omission discovers stillwind.config.* from Astro's application root when Vite is configured.
 * @returns An Astro integration that installs Stillwind Vite plugins, middleware, and document
 * stylesheet modules.
 */
export function stillwind(config: IntegrationConfig = {}): AstroIntegration {
  let applicationRoot: string | undefined;

  const project = createViteProject(config, () => applicationRoot);
  const plugins = createVitePlugins(project);
  const bridgeIds = new WeakMap<ResolvedConfig, string>();
  const bridges = new WeakMap<EnvironmentOwner, () => void>();
  const bridgeId = (configuration: ResolvedConfig) => {
    let id = bridgeIds.get(configuration);
    if (!id) {
      id = randomUUID();
      bridgeIds.set(configuration, id);
    }
    return id;
  };

  return {
    name: "@stillwind/integration",
    hooks: {
      /**
       * Captures Astro's root and installs middleware plus Stillwind's Vite and document plugins.
       * @param options - Astro setup operations and application configuration.
       */
      "astro:config:setup"({ config: astroConfig, updateConfig, addMiddleware }) {
        applicationRoot = fileURLToPath(astroConfig.root);
        addMiddleware({
          entrypoint: fileURLToPath(new URL("./middleware.js", import.meta.url)).replaceAll(
            "\\",
            "/",
          ),
          order: "pre",
        });
        updateConfig({
          vite: {
            plugins: [
              plugins,
              {
                name: "stillwind:astro-document",
                /**
                 * Registers a current-CSS bridge for each server environment and ties cleanup to server closure.
                 * @param server - Vite server whose environments resolve SSR dependencies and register watches.
                 * @throws If no SSR environment is available.
                 */
                configureServer(server) {
                  if (!server.environments.ssr) {
                    throw new Error("Stillwind Astro requires an SSR environment");
                  }
                  const generation = project.forConfig(server.config);
                  const id = bridgeId(server.config);
                  return () => {
                    const disposers = Object.entries(server.environments)
                      .filter(([name]) => name !== "client")
                      .map(([name, environment]) => {
                        const dispose = registerDevelopmentCss(`${id}:${name}`, async () => {
                          const plan = await generation.documentPlan(name, {
                            resolve: (specifier, importer, options) =>
                              environment.pluginContainer.resolveId(specifier, importer, options),
                            addWatchFile: (id) => {
                              server.watcher.add(id);
                            },
                            warn: (message) => {
                              server.config.logger.warn(message);
                            },
                          });
                          return plan.css;
                        });
                        bridges.set(environment, dispose);
                        return dispose;
                      });
                    /** Removes all bridge registrations owned by this server configuration. */
                    const dispose = () => {
                      for (const dispose of disposers) {
                        dispose();
                      }
                    };
                    server.httpServer?.once("close", dispose);
                  };
                },
                /** Removes the development bridge owned by the closing environment. */
                closeBundle() {
                  if (this.environment) {
                    bridges.get(this.environment)?.();
                  }
                },
                /**
                 * Resolves stylesheet virtual modules and keeps the development bridge external on the server.
                 * @param source - Authored module specifier.
                 * @returns A virtual identity, an external bridge, or undefined for unrelated requests.
                 * @throws If a client environment imports server-only styles or the CSS bridge.
                 */
                resolveId(source) {
                  if (source === BRIDGE_ID) {
                    if (this.environment?.name === "client") {
                      this.error("Stillwind Astro CSS bridge is server-only");
                    }
                    return {
                      id: source,
                      external: true,
                    };
                  }
                  if (source === ID || source === CLIENT_ID) {
                    if (source === ID && this.environment?.name === "client") {
                      this.error("Stillwind Astro styles are server-only");
                    }
                    return "\0" + source;
                  }
                  return undefined;
                },
                /**
                 * Generates SSR stylesheet markup or the client listener that refreshes inline SSR CSS.
                 * @param source - Resolved virtual module identity including its leading NUL.
                 * @returns Generated JavaScript, or undefined for unrelated modules. Production markup retains a manifest placeholder.
                 */
                load(source) {
                  const configuration = this.environment.getTopLevelConfig();
                  const generation = project.forConfig(configuration);
                  if (source === "\0" + CLIENT_ID) {
                    const endpoint =
                      generation.settings.publicBase + "__stillwind/style.css?environment=ssr";
                    return `// Stillwind SSR CSS (astro client)
if (import.meta.hot) {
  let revision = 0;
  import.meta.hot.on("stillwind:css", async () => {
    const requested = ++revision;
    const response = await fetch(${JSON.stringify(endpoint)});
    if (!response.ok) {
      throw new Error("Stillwind SSR CSS: " + response.status);
    }
    const css = await response.text();
    if (requested === revision) {
      for (const style of document.querySelectorAll("style[data-stillwind-ssr]")) {
        style.textContent = css;
      }
    }
  });
}
`;
                  }
                  if (source !== "\0" + ID) {
                    return;
                  }
                  if (generation.settings.watching) {
                    const script = `<script type="module" src="${generation.settings.publicBase}@id/__x00__${CLIENT_ID}"></script>`;
                    return `// Stillwind SSR CSS (astro with hot reload)
import { getDevelopmentCss } from ${JSON.stringify(BRIDGE_ID)};

export async function getStyles() {
  return "<style data-stillwind-ssr>" + escapeInlineCss(await getDevelopmentCss(${JSON.stringify(`${bridgeId(configuration)}:${this.environment.name}`)})) + "</style>" + ${JSON.stringify(script)};
}

function escapeInlineCss(css) {
  return css.replace(/<\\/style/gi, (match) => match.replace("/", "\\\\/"));
}
`;
                  }
                  return `// Stillwind SSR CSS (astro)
export async function getStyles() {
  return ${MARKER};
}
`;
                },
                /**
                 * Delivers client CSS and replaces the SSR marker with stylesheet links.
                 * @param code - Rendered server or client JavaScript.
                 * @param chunk - Chunk whose filename identifies the generated source map.
                 * @returns Changed JavaScript and its map, or nothing when no styles need delivery.
                 */
                renderChunk(code, chunk) {
                  const generation = project.forConfig(this.environment.getTopLevelConfig());

                  if (
                    this.environment.name === "client" &&
                    generation.config.emission !== "split"
                  ) {
                    return renderChunkStyles(generation, "client", this, code, chunk, true);
                  }

                  if (!code.includes(MARKER)) {
                    return;
                  }

                  const build = generation.buildOutput(this.environment.name);
                  const styles = (build?.manifest?.parts ?? [])
                    .map(
                      (part) =>
                        `<link rel="stylesheet" href="${part.href.replaceAll("&", "&amp;").replaceAll('"', "&quot;")}">`,
                    )
                    .join("");
                  const index = code.indexOf(MARKER);
                  if (index < 0) {
                    this.error("Cannot locate the Astro stylesheet placeholder");
                  }
                  const replacement = JSON.stringify(styles);
                  const output =
                    code.slice(0, index) + replacement + code.slice(index + MARKER.length);
                  return {
                    code: output,
                    map: JSON.stringify(
                      moduleSourceMap(
                        code,
                        output,
                        chunk.fileName,
                        [
                          {
                            originalStart: index,
                            originalEnd: index + MARKER.length,
                            generatedStart: index,
                            generatedEnd: index + replacement.length,
                          },
                        ],
                        [],
                        "",
                      ),
                    ),
                  };
                },
              },
            ],
          },
        });
      },
    },
  };
}
