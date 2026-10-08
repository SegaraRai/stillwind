export default {
  id: "rspack2-plain",
  support: {
    tier: "compatibility",
  },
  profiles: ["rspack"],
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
