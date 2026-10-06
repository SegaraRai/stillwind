import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/styles.css",
      files: ["src/**/*.{js,jsx,ts,tsx,vue,svelte,astro}"],
      types: false,
    },
  ],
});
