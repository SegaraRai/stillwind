/** Vite+ task configuration shared by the Stillwind workspace. */
export default {
  run: {
    tasks: {
      fmt: {
        command: "vp fmt",
      },
      lint: {
        command: "vp lint",
      },
      check: {
        command: "vp lint",
        dependsOn: ["fmt"],
      },
    },
  },
};
