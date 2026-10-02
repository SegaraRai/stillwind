export default {
  id: "rsbuild2-plain",
  support: { tier: "compatibility" },
  profiles: ["rsbuild"],
  contracts: [
    "rendering",
    "production",
    "cold-build",
    "warm-build",
    "css-source-map",
    "assets",
    "dependency-invalidation",
  ],
} as const;
