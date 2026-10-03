import {
  compileSource,
  compileProject,
  summarizeSource,
  scanSource,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "../binding.js";
import type { NativePreparedSource, NativeStylesheetSelection } from "../binding.js";

// Checked by tsc; the generated declarations must describe actual JS values.
export function verifyNativeTypes() {
  const sources: NativePreparedSource[] = [
    {
      kind: "direct",
      moduleId: "x.ts",
      document: {
        uri: "untitled:x.ts",
        code: "export {};",
        format: "script",
        sourceMapSource: "untitled:x.ts",
      },
      syntax: {
        language: "ts",
        moduleKind: "unambiguous",
      },
    },
  ];
  const output = compileSource(
    {
      source: "export {};",
      moduleId: "x.ts",
      uri: "untitled:x.ts",
      sourceMapSource: "untitled:x.ts",
      language: "ts",
      moduleKind: "unambiguous",
      css: {
        content: "",
        name: "root.css",
        uri: "untitled:root.css",
        sourceMapSource: "untitled:root.css",
      },
    },
    {},
  );
  const code: string | null = output.code;
  const project = compileProject(sources, {
    content: "",
    name: "root.css",
    uri: "untitled:root.css",
    sourceMapSource: "untitled:root.css",
  });
  const moduleId: string = project.modules[0].moduleId;
  scanSource(sources[0]);
  const projected: NativePreparedSource = {
    kind: "projected",
    moduleId: "Component.vue",
    document: {
      uri: "untitled:Component.vue",
      code: "<script>export {};</script>",
      format: "vue",
    },
    projection: {
      code: "export {};",
      syntax: {
        language: "js",
        moduleKind: "module",
      },
      map: {
        kind: "volar",
        mappings: [
          {
            sourceOffsets: [8],
            generatedOffsets: [0],
            lengths: [10],
          },
        ],
      },
      converter: {
        name: "test",
        version: "1",
        optionsFingerprint: "test",
      },
      importRealms: [],
      editableRegions: [],
    },
  };
  scanSource(projected);
  const { map: _map, ...withoutMap } = projected.projection;
  // @ts-expect-error A projected source requires its generated-to-original map.
  scanSource({ ...projected, projection: withoutMap });
  const { converter: _converter, ...withoutConverter } = projected.projection;
  // @ts-expect-error A projected source requires the converter identity.
  scanSource({ ...projected, projection: withoutConverter });
  const { kind: _kind, ...unprepared } = sources[0];
  // @ts-expect-error The scanner accepts only explicitly prepared sources.
  scanSource(unprepared);
  const summary = summarizeSource("export {};", "x.ts", "ts", "unambiguous");
  const loader = new NativeDesignSystemLoader({ prefix: "--custom-" });
  const step = loader.provide(0, "root.css", "");
  if (step.kind === "needFile") {
    step.requests.map((request) => request.id);
  } else {
    step.sources.map((name) => name.length);
  }
  const session = new NativeProjectSession(loader, true);
  const selection: NativeStylesheetSelection = {
    modules: [],
    coverage: { kind: "observed" },
    placement: "document",
  };
  session.planStylesheet(selection).sourceMap.entries.map((entry) => entry.generatedStart);
  session.compile(sources).modules.map((module) => module.diagnostics);
  // @ts-expect-error Native compilation takes an array, not encoded JSON.
  session.compile("[]");
  // @ts-expect-error Compiler options are typed objects.
  new NativeDesignSystemLoader("{}");
  // @ts-expect-error The generated string enum rejects unknown placements.
  session.planStylesheet({ ...selection, placement: "unknown" });
  return { code, moduleId, summary };
}
