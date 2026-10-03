const temporaryInputs = [
  { auto: true },
  { pattern: "!**/.test-*/**", base: "workspace" as const },
  { pattern: "!**/.review-fixtures/**", base: "workspace" as const },
  { pattern: "!**/node_modules/.vite-temp/**", base: "workspace" as const },
  { pattern: "!**/stillwind-*/**", base: "workspace" as const },
];

/** Tracks test inputs while excluding disposable files from cache restoration. */
export const testCache = {
  env: ["NODE_ENV", "NAPI_RS_*", "STILLWIND_KEEP_FRAMEWORK_FIXTURES"],
  input: temporaryInputs,
  output: [],
};

/** Tracks fixture inputs and keeps virtual source directories out of restored outputs. */
export const fixtureCache = {
  env: ["NAPI_RS_*"],
  input: temporaryInputs,
  output: [{ auto: true }, { pattern: "!**/.review-fixtures/**", base: "workspace" as const }],
};

/** Tracks Rust sources even when Cargo reuses compiled dependencies. */
export const rustCache = {
  env: [
    "CARGO_*",
    "RUST*",
    "CC",
    "CC_*",
    "CXX",
    "CXX_*",
    "AR",
    "AR_*",
    "CFLAGS*",
    "CXXFLAGS*",
    "LDFLAGS*",
    "WASI_*",
    "NAPI_RS_*",
  ],
  input: [
    { auto: true },
    { pattern: "crates/**", base: "workspace" as const },
    { pattern: "Cargo.toml", base: "workspace" as const },
    { pattern: "Cargo.lock", base: "workspace" as const },
    { pattern: "!target/**", base: "workspace" as const },
    { pattern: "!**/.*.napi-stage-*/**", base: "workspace" as const },
    { pattern: "!**/.napi-rs-*/**", base: "workspace" as const },
    { pattern: "!**/napi-rs-*/**", base: "workspace" as const },
    { pattern: "!**/*.tmp", base: "workspace" as const },
  ],
  output: [],
};
