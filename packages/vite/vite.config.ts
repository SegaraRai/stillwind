export default {
  run: {
    tasks: {
      test: {
        command: "node --test test/type-facts.test.mts",
        dependsOn: ["@stillwind/native#build"],
      },
    },
  },
};
