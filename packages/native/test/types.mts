import {
  compileSource,
  compileProject,
  summarizeSource,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "../dist/index.js";
import type { NativeProjectSource, NativeStylesheetSelection } from "../dist/index.js";

// Checked by tsc; the generated declarations must describe actual JS values.
export function verifyNativeTypes() {
  const sources: NativeProjectSource[] = [
    {
      module_id: "x.ts",
      uri: "untitled:x.ts",
      source: "export {};",
      language: "ts",
      module_kind: "unambiguous",
      source_map_source: "untitled:x.ts",
    },
  ];
  const output = compileSource(
    {
      source: "export {};",
      module_id: "x.ts",
      uri: "untitled:x.ts",
      source_map_source: "untitled:x.ts",
      language: "ts",
      module_kind: "unambiguous",
      css: {
        content: "",
        name: "root.css",
        uri: "untitled:root.css",
        source_map_source: "untitled:root.css",
      },
    },
    {},
  );
  const code: string | null = output.code;
  const project = compileProject(sources, {
    content: "",
    name: "root.css",
    uri: "untitled:root.css",
    source_map_source: "untitled:root.css",
  });
  const moduleId: string = project.modules[0].module_id;
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
