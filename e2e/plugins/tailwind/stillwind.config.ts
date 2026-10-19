import { defineConfig } from "@stillwind/config";

export default defineConfig({
  projects: [
    "animate",
    "animated",
    "forms",
    "headless",
    "iconify",
    "motion",
    "radix",
    "react-aria",
    "safe-area",
    "scrollbar-hide",
    "scrollbar",
    "tw-animate",
    "typography",
  ].map((name) => ({
    name,
    root: `projects/${name}`,
    css: "theme.css",
    files: ["*.ts"],
    typeFacts: false,
  })),
});
