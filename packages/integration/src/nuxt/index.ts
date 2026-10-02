import { addPluginTemplate, defineNuxtModule } from "nuxt/kit";
import type { NuxtModule } from "nuxt/schema";
import type { UserConfig, ViteDevServer } from "vite";

import { stillwind as vitePlugin, type StillwindOptions } from "../vite/index.ts";

export type { StillwindOptions } from "../vite/index.ts";

/**
 * Adds the Vite compiler and development SSR stylesheet transport to a Nuxt application.
 */
export function stillwind(options: StillwindOptions): NuxtModule {
  return defineNuxtModule({
    meta: {
      name: "@stillwind/integration",
    },
    setup(_, nuxt) {
      let ssr: ViteDevServer | undefined;

      nuxt.hook("vite:extendConfig", (config) => {
        const vite = config as UserConfig;
        (vite.plugins ??= []).unshift(vitePlugin(options));
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
