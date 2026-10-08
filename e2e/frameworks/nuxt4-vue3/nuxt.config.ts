import { stillwind } from "@stillwind/integration/nuxt";

export default defineNuxtConfig({
  devtools: {
    enabled: false,
  },
  modules: [
    stillwind({
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
  ],
});
