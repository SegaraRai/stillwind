import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";

import { moduleSourceMap } from "@stillwind/host";
import type { AstroIntegration } from "astro";

import { createVitePlugins } from "../vite/plugin.ts";
import { createViteProject, type StillwindOptions } from "../vite/project.ts";
import { registerDevelopmentCss } from "./development.ts";

export type { StillwindOptions } from "../vite/index.ts";

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
 * @param options - Compiler and Vite emission options shared by the client and server environments.
 * @returns An Astro integration that installs Stillwind Vite plugins, middleware, and document
 * stylesheet modules.
 */
export function stillwind(options: StillwindOptions): AstroIntegration {
  const project = createViteProject(options);
  const plugins = createVitePlugins(project);
  const bridgeId = randomUUID();

  let disposeBridge = () => {};

  return {
    name: "@stillwind/integration",
    hooks: {
      "astro:config:setup"({ updateConfig, addMiddleware }) {
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
                configureServer(server) {
                  disposeBridge();
                  if (!server.environments.ssr) {
                    throw new Error("Stillwind Astro requires an SSR environment");
                  }
                  const disposers = Object.entries(server.environments)
                    .filter(([name]) => name !== "client")
                    .map(([name, environment]) =>
                      registerDevelopmentCss(`${bridgeId}:${name}`, async () => {
                        const plan = await project.documentPlan(name, {
                          resolve: (specifier, importer) =>
                            environment.pluginContainer.resolveId(specifier, importer),
                          addWatchFile: (id) => {
                            server.watcher.add(id);
                          },
                        });
                        return plan.css;
                      }),
                    );
                  const dispose = () => {
                    for (const dispose of disposers) {
                      dispose();
                    }
                  };
                  disposeBridge = dispose;
                  server.httpServer?.once("close", dispose);
                },
                closeBundle() {
                  disposeBridge();
                },
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
                load(source) {
                  if (source === "\0" + CLIENT_ID) {
                    const endpoint =
                      project.settings.publicBase + "__stillwind/style.css?environment=ssr";
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
                  if (project.settings.watching) {
                    const script = `<script type="module" src="${project.settings.publicBase}@id/__x00__${CLIENT_ID}"></script>`;
                    return `// Stillwind SSR CSS (astro with hot reload)
import { getDevelopmentCss } from ${JSON.stringify(BRIDGE_ID)};

export async function getStyles() {
  return "<style data-stillwind-ssr>" + escapeInlineCss(await getDevelopmentCss(${JSON.stringify(`${bridgeId}:${this.environment?.name ?? "ssr"}`)})) + "</style>" + ${JSON.stringify(script)};
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
                renderChunk(code, chunk) {
                  if (!code.includes(MARKER)) {
                    return;
                  }

                  const build = project.buildOutput(this.environment?.name);
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
