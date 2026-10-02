import { resolve } from "node:path";

import { stillwind } from "@stillwind/integration/nuxt";

export default defineNuxtConfig({
  devtools: { enabled: false },
  modules: [
    stillwind({
      css: resolve("src/styles.css"),
      types: false,
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
  ],
});
