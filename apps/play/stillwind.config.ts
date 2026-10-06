import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "styles/default-theme.css",
      files: ["src/**/*.{ts,tsx}"],
    },
  ],
});
