export default {
  id: "tanstackstart1-react19-server-function",
  support: {
    tier: "primary",
    profiles: {
      rsbuild: "compatibility",
    },
  },
  profiles: ["vite", "rsbuild"],
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
