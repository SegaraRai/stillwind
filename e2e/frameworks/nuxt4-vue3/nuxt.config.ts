import { resolve } from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import type { ViteDevServer } from "vite-plus";

let ssr: ViteDevServer | undefined;

export default defineNuxtConfig({
  devtools: { enabled: false },
  hooks: {
    "vite:extendConfig"(config) {
      (config.plugins ??= []).unshift(
        stillwind({
          css: resolve("src/styles.css"),
          types: false,
          emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
        }),
      );
    },
    "vite:serverCreated"(server, { isClient }) {
      if (!isClient) {
        ssr = server;
        return;
      }
      server.middlewares.use("/e2e-ssr.css", (request, response, next) => {
        if (!ssr) {
          return next();
        }
        request.url = "/__stillwind/style.css?environment=ssr";
        ssr.middlewares(request, response, next);
      });
    },
  },
});
