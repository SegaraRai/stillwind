import { addPluginTemplate, defineNuxtModule } from "nuxt/kit";
import type { NuxtModule } from "nuxt/schema";
import type { UserConfig, ViteDevServer } from "vite";

import type { IntegrationConfig } from "../vite/index.ts";
import { createVitePlugins } from "../vite/plugin.ts";
import { createViteProject } from "../vite/project.ts";

export type { IntegrationConfig } from "../vite/index.ts";

/**
 * Adds the Vite compiler and development SSR stylesheet transport to a Nuxt application.
 *
 * @param config - File discovery, project selection, inline compiler overrides, and CSS emission.
 * Omission reads stillwind.config.ts relative to Nuxt's application root during Vite configuration.
 * @returns A Nuxt module that registers the Vite plugins and client stylesheet handling.
 */
export function stillwind(config: IntegrationConfig = {}): NuxtModule {
  return defineNuxtModule({
    meta: {
      name: "@stillwind/integration",
    },
    setup(_, nuxt) {
      let ssr: ViteDevServer | undefined;

      nuxt.hook("vite:extendConfig", (viteConfig) => {
        const vite = viteConfig as UserConfig;
        vite.plugins ??= [];
        vite.plugins.unshift(
          createVitePlugins(createViteProject(config, () => nuxt.options.rootDir)),
        );
      });

      nuxt.hook("vite:serverCreated", (server, { isClient }) => {
        if (!isClient) {
          ssr = server;
          return;
        }
        server.middlewares.use(
          nuxt.options.app.baseURL + "__stillwind/nuxt.css",
          (request, response, next) => {
            if (!ssr) {
              return next();
            }
            request.url = "/__stillwind/style.css?environment=ssr";
            ssr.middlewares(request, response, next);
          },
        );
      });

      if (nuxt.options.dev) {
        addPluginTemplate({
          filename: "stillwind-ssr.ts",
          getContents: () =>
            `
import { defineNuxtPlugin, useHead } from "#app";

export default defineNuxtPlugin((nuxt) => {
  const head = useHead({
    link: [
      {
        key: "stillwind-ssr",
        rel: "stylesheet",
        href: ${JSON.stringify(nuxt.options.app.baseURL + "__stillwind/nuxt.css")},
      },
    ],
  });

  nuxt.hook("app:mounted", () => {
    head.dispose();
  });
});
`.trimStart(),
        });
      }
    },
  });
}
