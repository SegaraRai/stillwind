import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "root.css",
      files: ["app/**/*.{js,jsx,ts,tsx}"],
    },
  ],
});
