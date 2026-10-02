import { fileURLToPath } from "node:url";

import { moduleSourceMap } from "@stillwind/host";
import type { AstroIntegration } from "astro";

import { compilerPlugin } from "../vite/plugin.ts";
import { createViteProject, type StillwindOptions } from "../vite/project.ts";

export type { StillwindOptions } from "../vite/index.ts";

const ID = "virtual:stillwind/astro-styles";
const CLIENT_ID = ID + "/client";
const MARKER = '"__STILLWIND_ASTRO_STYLES_574f6dfce7c69466ab9516fb7fc6199a__"';

/**
 * Compiles Astro components and supplies their CSS to rendered documents and hydrated islands.
 */
export function stillwind(options: StillwindOptions): AstroIntegration {
  const project = createViteProject(options);
  const plugin = compilerPlugin(project);

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
              plugin,
              {
                name: "stillwind:astro-document",
                resolveId(source) {
                  if (source === ID || source === CLIENT_ID) {
                    return "\0" + source;
                  }
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
                    const endpoint =
                      project.settings.publicBase + "__stillwind/style.css?environment=ssr";
                    const script = `<script type="module" src="${project.settings.publicBase}@id/__x00__${CLIENT_ID}"></script>`;
                    return `// Stillwind SSR CSS (astro with hot reload)
export async function getStyles(url) {
  const response = await fetch(new URL(${JSON.stringify(endpoint)}, url));
  if (!response.ok) {
    throw new Error("Stillwind SSR CSS: " + response.status);
  }
  return "<style data-stillwind-ssr>" + escapeInlineCss(await response.text()) + "</style>" + ${JSON.stringify(script)};
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

                  const state = project.environmentState(this.environment?.name);
                  const styles = (state.manifest?.parts ?? [])
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
