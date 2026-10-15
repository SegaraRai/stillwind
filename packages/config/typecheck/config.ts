import {
  defineConfig,
  lintPresets,
  type LintPreset,
  type SourceProvider,
  type ProjectionProvider,
  type StillwindConfig,
  type StillwindConfigExport,
  type StillwindConfigFactory,
} from "../src/index.ts";

defineConfig({
  projects: [
    {
      css: "theme.css",
      files: ["**/*.tsx"],
      tsconfig: "tsconfig.app.json",
      sort: false,
      rootFontSize: 20,
      target: ["chrome111", "firefox113", "safari17"],
      preview: {
        themeValues: false,
        colorEquivalents: "exact",
      },
      sources: {
        kind: "filesystem",
      },
      lint: {
        extends: [lintPresets.recommended, lintPresets.stylistic],
        rules: {
          "prefer-canonical-utilities": {
            pxToRem: false,
          },
        },
      },
    },
  ],
});

defineConfig({
  projects: [
    {
      css: "theme.css",
      files: ["**/*.tsx"],
      lint: false,
      sort: false,
    },
  ],
});

const team = {
  name: "team/policy",
  extends: [lintPresets.strict],
  rules: {
    "no-restricted-length-units": {
      rules: [
        {
          properties: ["padding*"],
          disallow: ["px"],
        },
      ],
    },
  },
} as const satisfies LintPreset;
void team;

const synchronous = defineConfig(() => ({
  projects: [
    {
      css: "theme.css",
      files: ["*.tsx"],
      sources: {
        kind: "filesystem",
      },
    },
  ],
}));
const synchronousSettings: StillwindConfig = synchronous();

const asynchronous = defineConfig(async () => ({
  projects: [
    {
      css: "theme.css",
      files: ["*.tsx"],
      typeFacts: false,
    },
  ],
}));
const asynchronousSettings: Promise<StillwindConfig> = asynchronous();

const promised: Promise<StillwindConfig> = defineConfig(
  Promise.resolve({
    projects: [
      {
        css: "theme.css",
        files: ["*.tsx"],
      },
    ],
  }),
);

export function preserveExport(config: StillwindConfigExport): StillwindConfigExport {
  return defineConfig(config);
}

export function preserveFactory(config: StillwindConfigFactory): StillwindConfigFactory {
  return defineConfig(config);
}

// @ts-expect-error Configuration factories must return project settings.
void defineConfig(() => "invalid");
// @ts-expect-error Asynchronous configuration factories must resolve to project settings.
void defineConfig(async () => "invalid");
// @ts-expect-error Configuration promises must resolve to project settings.
void defineConfig(Promise.resolve("invalid"));

void synchronousSettings;
void asynchronousSettings;
void promised;

const custom: SourceProvider = {
  kind: "custom",
  supports: () => true,
  load: async () => {
    throw new Error("Not used in type checks");
  },
};

const projection: ProjectionProvider = {
  kind: "custom",
  prepare: async () => {
    throw new Error("Not used in type checks");
  },
};

void custom;
void projection;

// @ts-expect-error Custom source providers require a load operation.
const missingLoad: SourceProvider = { kind: "custom", supports: () => true };
// @ts-expect-error Custom projection providers require a prepare operation.
const missingProjection: ProjectionProvider = { kind: "custom" };
void missingLoad;
void missingProjection;
export function sourceLoad(provider: SourceProvider) {
  if (provider.kind === "custom") {
    return provider.load.bind(provider);
  }
  // @ts-expect-error Filesystem providers do not carry custom source operations.
  return provider.load;
}
