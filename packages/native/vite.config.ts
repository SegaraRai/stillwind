/** Vite+ task configuration for the local Node-API addon. */
export default {
  run: {
    tasks: {
      build: {
        command: "napi build --release --platform --no-js --manifest-path ../../crates/shorth-napi/Cargo.toml --output-dir . --package-json-path package.json",
      },
    },
  },
}
