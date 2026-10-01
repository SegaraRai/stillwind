export default {
  id: "rsbuild2-plain",
  profiles: ["rsbuild"],
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
