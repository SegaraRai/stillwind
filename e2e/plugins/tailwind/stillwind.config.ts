import { defineConfig } from "@stillwind/config";

export default defineConfig({
  projects: [
    "animate",
    "animated",
    "daisyui",
    "forms",
    "headless",
    "iconify",
    "intersect",
    "motion",
    "preline",
    "radix",
    "react-aria",
    "safe-area",
    "scrollbar-hide",
    "scrollbar",
    "theme-prefix",
    "tw-animate",
    "typography",
  ].map((name) => ({
    name,
    root: `projects/${name}`,
    css: "theme.css",
    files: ["*.ts"],
    typeFacts: false,
    ...(name === "theme-prefix" ? { themePrefix: "--tw-" } : {}),
  })),
});
