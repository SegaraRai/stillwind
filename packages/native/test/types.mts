import {
  compileReact,
  compileReactProject,
  summarizeReact,
  NativeDesignSystemLoader,
  NativeProjectSession,
} from "../index.js";
import type { NativeProjectSource, NativeStylesheetSelection } from "../index.js";

// Checked by tsc; the generated declarations must describe actual JS values.
export function verifyNativeTypes() {
  const sources: NativeProjectSource[] = [{ path: "x.ts", source: "export {};" }];
  const output = compileReact("export {};", "x.ts", "", { classIdentity: "module" });
  const code: string | null = output.code;
  const project = compileReactProject(sources, "");
  const modulePath: string = project.modules[0].path;
  const summary = summarizeReact("export {};", "x.ts");
  const loader = new NativeDesignSystemLoader({ prefix: "--custom-" });
  const step = loader.provide(0, "root.css", "");
  if (step.kind === "needFile") step.requests.map((request) => request.id);
  else step.sources.map((name) => name.length);
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
  return { code, modulePath, summary };
}
