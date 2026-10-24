import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/styles.css",
      files: ["src/**/*.{ts,tsx}", "routes/**/*.tsx", "islands/**/*.tsx"],
    },
  ],
});
