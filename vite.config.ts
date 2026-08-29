/** Vite+ task configuration shared by the Stillwind workspace. */
export default {
  lint: {
    options: { typeAware: true },
  },
  fmt: {
    ignorePatterns: [],
  },
  run: {
    tasks: {
      fmt: {
        command: "vp fmt --check",
      },
      "fmt:write": {
        command: "vp fmt",
      },
      "webref:generate": {
        command: "node tools/generate-webref-css-domains.mts",
      },
      "webref:check": {
        command: "node tools/generate-webref-css-domains.mts --check",
      },
      lint: {
        command: "vp lint",
      },
      check: {
        command: "vp check",
      },
    },
  },
};
