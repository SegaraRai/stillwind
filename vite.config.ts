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
      lint: {
        command: "vp lint",
      },
      check: {
        command: "vp check",
      },
    },
  },
};
