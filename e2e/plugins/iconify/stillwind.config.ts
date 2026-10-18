import { defineConfig } from "@stillwind/config";

export default defineConfig({
  projects: ["import-first", "plugin-first"].map((name) => ({
    name,
    root: `projects/${name}`,
    css: "theme.css",
    files: ["*.ts"],
    typeFacts: false,
  })),
});
