export default {
  id: "rspack2-plain",
  profiles: ["rspack"],
  script: "scripts/acceptance.ts",
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
