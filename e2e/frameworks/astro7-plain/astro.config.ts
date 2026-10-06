import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [
    stillwind({
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
  ],
  build: { inlineStylesheets: "never" },
});
