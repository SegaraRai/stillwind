/** Vite+ task configuration for the compatibility fixture toolchain. */
export default {
  run: {
    tasks: {
      "fixtures:write": {
        command: "node regenerate-tailwind-v4-fixtures.ts --write",
      },
      "fixtures:check": {
        command: "node regenerate-tailwind-v4-fixtures.ts",
      },
      "registry:static": {
        command: "node generate-static-utility-registry.ts",
      },
      "registry:static:write": {
        command: "node generate-static-utility-registry.ts --write",
      },
    },
  },
};
