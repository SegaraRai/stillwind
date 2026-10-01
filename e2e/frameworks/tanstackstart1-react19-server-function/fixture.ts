export default {
  id: "tanstackstart1-react19-server-function",
  profiles: ["vite", "rsbuild"],
  script: "scripts/acceptance.ts",
  contracts: [
    "rendering",
    "production",
    "cold-build",
    "warm-build",
    "ssr",
    "hydration",
    "navigation",
    "css-edit",
    "macro-removal",
    "source-map",
  ],
} as const;
