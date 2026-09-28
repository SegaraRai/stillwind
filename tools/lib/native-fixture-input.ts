import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  compileSource,
  type NativeCompileCssInput,
  type NativeGeneratedClass,
  type NativeProjectSource,
} from "@stillwind/native";

export function fixtureProjectSource(source: string, moduleId: string): NativeProjectSource {
  const uri = pathToFileURL(path.resolve(moduleId)).href;
  return {
    module_id: moduleId,
    uri,
    source_map_source: uri,
    language: moduleId.endsWith(".tsx") ? "tsx" : "ts",
    module_kind: "unambiguous",
    source,
  };
}

export function fixtureCss(content: string): NativeCompileCssInput {
  const uri = pathToFileURL(path.resolve("stillwind.css")).href;
  return { content, name: "stillwind.css", uri, source_map_source: uri };
}

export function compileFixtureSource(source: string, moduleId: string, css: string) {
  return compileSource({ ...fixtureProjectSource(source, moduleId), css: fixtureCss(css) });
}

export function comparableFixtureClasses(classes: NativeGeneratedClass[]) {
  return classes.map(({ call_span, guard_path, ...generated }) => ({
    ...generated,
    call_span: { start: call_span.start, end: call_span.end },
    guard_path: guard_path.map(({ test, ...guard }) => ({
      ...guard,
      test: { start: test.start, end: test.end },
    })),
  }));
}
